#!/usr/bin/env -S nix develop -c cargo -q -Zscript
---cargo
[package]
edition = "2024"
---

use std::fs;
use std::path::Path;
use std::process::ExitCode;

const ERROR_EXIT: u8 = 1;
const PROVIDER_SRC: &str = "crates/clankers-provider/src";
const PROVIDER_MANIFEST: &str = "crates/clankers-provider/Cargo.toml";
const RESPONSIBILITY: &str = "crates/clankers-provider/src/provider_router_responsibility.rs";
const BRIDGE: &str = "crates/clankers-provider/src/router_request_bridge.rs";
const ROUTER: &str = "crates/clankers-provider/src/router.rs";
const DISCOVERY: &str = "crates/clankers-provider/src/discovery.rs";
const RPC_PROVIDER: &str = "crates/clankers-provider/src/rpc_provider.rs";
const PROVIDER_ADAPTER_SOURCE: &str = "examples/embedded-provider-adapter/src/main.rs";
const PROVIDER_ADAPTER_MANIFEST: &str = "examples/embedded-provider-adapter/Cargo.toml";
const PROVIDER_ADAPTER_FIXTURE: &str = "examples/embedded-provider-adapter/fixtures/provider-adapter-fixtures.json";

const BRIDGE_MARKERS: &[&str] = &[
    "Single clankers-provider owned bridge into `clanker_router::CompletionRequest`",
    "pub(crate) fn build_router_request",
    "fn messages_to_router_json",
    "fn content_to_router_json",
    "Branch summary",
    "Compaction summary",
];

const ROUTER_MARKERS: &[&str] = &[
    "crate::router_request_bridge::build_router_request(request)",
    "struct RouterProvider",
    "fail_closed_prefixes",
    "FallbackConfig::with_defaults()",
    "fn resolve(&self, model: &str)",
    "self.fallbacks.chain_for",
    "db.rate_limits().is_healthy",
    "let is_retryable = e.is_retryable();",
];

fn main() -> ExitCode {
    match run() {
        Ok(()) => {
            println!("ok: provider/router boundary rail passed");
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("provider/router boundary rail failed: {error}");
            ExitCode::from(ERROR_EXIT)
        }
    }
}

fn run() -> Result<(), String> {
    let responsibility = read(RESPONSIBILITY)?;
    for marker in [
        "SDK_PROVIDER_EDGE_CONCERNS_REQUIREMENT",
        "SDK_PROVIDER_EDGE_NO_DISPLAY_DTOS_REQUIREMENT",
        "SDK_PROVIDER_EDGE_SDK_HOST_REQUIREMENT",
        "SDK_PROVIDER_EDGE_LITERAL_FIXTURE_REQUIREMENT",
        "SDK_PROVIDER_EDGE_DEPENDENCY_RAIL_REQUIREMENT",
        "DisplayDtoBoundary",
        "SdkProviderAdapter",
        "LiteralRequestFixtures",
    ] {
        require_contains(&responsibility, marker, RESPONSIBILITY)?;
    }

    let manifest = read(PROVIDER_MANIFEST)?;
    for forbidden in [
        "clanker-tui-types",
        "clanker_tui_types",
        "clankers-protocol",
        "clankers_protocol",
    ] {
        forbid_contains(&manifest, forbidden, PROVIDER_MANIFEST)?;
    }

    let bridge = read(BRIDGE)?;
    for marker in BRIDGE_MARKERS {
        require_contains(&bridge, marker, BRIDGE)?;
    }

    let router = read(ROUTER)?;
    for marker in ROUTER_MARKERS {
        require_contains(&router, marker, ROUTER)?;
    }

    let discovery = read(DISCOVERY)?;
    require_contains(&discovery, "RouterProvider::with_db", DISCOVERY)?;
    require_contains(&discovery, "RouterProvider::new", DISCOVERY)?;
    require_contains(&discovery, "RouterCompatAdapter::new", DISCOVERY)?;

    let rpc_provider = read(RPC_PROVIDER)?;
    require_contains(&rpc_provider, "crate::router_request_bridge::build_router_request(request)", RPC_PROVIDER)?;

    for path in rust_files(Path::new(PROVIDER_SRC))? {
        let text = read_path(&path)?;
        let runtime = text.split("#[cfg(test)]").next().unwrap_or(&text);
        let path_text = path.to_string_lossy();
        for forbidden in [
            "clanker_tui_types",
            "clankers_protocol",
            "DaemonEvent",
            "SessionCommand",
        ] {
            forbid_contains(runtime, forbidden, &path_text)?;
        }
        if runtime.contains("clanker_router::CompletionRequest {") && path_text != BRIDGE && path_text != RPC_PROVIDER {
            return Err(format!(
                "{path_text} constructs clanker_router::CompletionRequest directly; route through {BRIDGE}"
            ));
        }
        if runtime.contains("serde_json::to_value(AgentMessage") || runtime.contains("serde_json::to_value(message)") {
            return Err(format!(
                "{path_text} serializes AgentMessage directly for router requests; use provider-native bridge JSON"
            ));
        }
    }

    let provider_lib = read("crates/clankers-provider/src/lib.rs")?;
    require_contains(&provider_lib, "clanker_message::ThinkingLevel", "crates/clankers-provider/src/lib.rs")?;
    forbid_contains(&provider_lib, "clanker_tui_types::ThinkingLevel", "crates/clankers-provider/src/lib.rs")?;

    let adapter_source = read(PROVIDER_ADAPTER_SOURCE)?;
    let adapter_manifest = read(PROVIDER_ADAPTER_MANIFEST)?;
    let adapter_fixture = read(PROVIDER_ADAPTER_FIXTURE)?;
    for forbidden in [
        "clankers-provider",
        "clanker-router",
        "OAuth",
        "provider discovery",
        "live network credentials",
    ] {
        forbid_contains(&adapter_source, forbidden, PROVIDER_ADAPTER_SOURCE)?;
    }
    for forbidden in ["clankers-provider", "clanker-router", "clankers-config", "reqwest"] {
        forbid_contains(&adapter_manifest, forbidden, PROVIDER_ADAPTER_MANIFEST)?;
    }
    for marker in [
        "clankers.embedded_provider_adapter.fixtures.v1",
        "request_fixture",
        "model_capability_profile",
        "response_fixtures",
        "forbidden_dependencies",
    ] {
        require_contains(&adapter_fixture, marker, PROVIDER_ADAPTER_FIXTURE)?;
    }

    Ok(())
}

fn rust_files(root: &Path) -> Result<Vec<std::path::PathBuf>, String> {
    let mut files = Vec::new();
    collect_rust_files(root, &mut files)?;
    files.sort();
    Ok(files)
}

fn collect_rust_files(path: &Path, out: &mut Vec<std::path::PathBuf>) -> Result<(), String> {
    for entry in fs::read_dir(path).map_err(|error| format!("failed to read dir {}: {error}", path.display()))? {
        let entry = entry.map_err(|error| format!("failed to read dir entry under {}: {error}", path.display()))?;
        let path = entry.path();
        if path.is_dir() {
            collect_rust_files(&path, out)?;
        } else if path.extension().and_then(|ext| ext.to_str()) == Some("rs") {
            out.push(path);
        }
    }
    Ok(())
}

fn read(path: &str) -> Result<String, String> {
    fs::read_to_string(path).map_err(|error| format!("failed to read {path}: {error}"))
}

fn read_path(path: &Path) -> Result<String, String> {
    fs::read_to_string(path).map_err(|error| format!("failed to read {}: {error}", path.display()))
}

fn require_contains(text: &str, marker: &str, path: &str) -> Result<(), String> {
    if text.contains(marker) {
        Ok(())
    } else {
        Err(format!("{path} missing required marker `{marker}`"))
    }
}

fn forbid_contains(text: &str, marker: &str, path: &str) -> Result<(), String> {
    if text.contains(marker) {
        Err(format!("{path} contains forbidden provider-edge marker `{marker}`"))
    } else {
        Ok(())
    }
}
