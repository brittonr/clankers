#!/usr/bin/env -S nix develop -c cargo -q -Zscript
---cargo
[package]
edition = "2024"

[dependencies]
blake3 = "1"
serde_json = "1"
---

use std::collections::BTreeSet;
use std::fs;
use std::io::Read;
use std::path::Path;
use std::process::ExitCode;

use serde_json::{json, Value};

const ERROR_EXIT: u8 = 1;
const FIXTURE: &str = "examples/embedded-provider-adapter/fixtures/provider-adapter-fixtures.json";
const SOURCE: &str = "examples/embedded-provider-adapter/src/main.rs";
const CARGO_MANIFEST: &str = "examples/embedded-provider-adapter/Cargo.toml";
const SPEC: &str = "cairn/specs/embedded-composition-kits/spec.md";
const DOCS: &str = "docs/src/tutorials/embedded-agent-sdk.md";
const OUTPUT: &str = "target/embedded-sdk-release/provider-adapter-kit-receipt.json";
const FORBIDDEN: &[&str] = &["clankers-provider", "clanker-router", "OAuth", "provider discovery", "live network credentials"];

fn main() -> ExitCode {
    match run() {
        Ok(()) => {
            println!("provider-adapter-kit receipt written to {OUTPUT}");
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("provider-adapter-kit check failed: {error}");
            ExitCode::from(ERROR_EXIT)
        }
    }
}

fn run() -> Result<(), String> {
    let fixture = json_file(FIXTURE)?;
    validate_fixture(&fixture)?;
    validate_source()?;
    validate_boundary()?;
    validate_docs_and_spec()?;

    if let Some(parent) = Path::new(OUTPUT).parent() {
        fs::create_dir_all(parent).map_err(|error| format!("failed to create {}: {error}", parent.display()))?;
    }
    let receipt = json!({
        "schema": "clankers.embedded_provider_adapter.receipt.v1",
        "fixture": hash_artifact(Path::new(FIXTURE))?,
        "source": hash_artifact(Path::new(SOURCE))?,
        "cargo_manifest": hash_artifact(Path::new(CARGO_MANIFEST))?,
        "policy": "explicit fixtures are authored data; expected request/response shapes are not derived from ProductProviderAdapter implementation",
    });
    write_json(OUTPUT, &receipt)?;
    Ok(())
}

fn validate_fixture(fixture: &Value) -> Result<(), String> {
    require_eq(fixture, "schema", "clankers.embedded_provider_adapter.fixtures.v1")?;
    let request = fixture.get("request_fixture").ok_or_else(|| "missing request_fixture".to_string())?;
    require_eq(request, "model", "product-owned-model")?;
    require_eq(request, "session_id", "embedded-provider-session")?;
    require_eq(request, "prompt_text", "answer directly")?;

    let profile = fixture
        .get("model_capability_profile")
        .ok_or_else(|| "missing model_capability_profile".to_string())?;
    require_eq(profile, "owner", "product")?;
    if profile.get("live_credentials") != Some(&Value::Bool(false)) {
        return Err("model capability profile must disable live credentials".to_string());
    }
    if profile.get("network_required") != Some(&Value::Bool(false)) {
        return Err("model capability profile must not require network".to_string());
    }

    let names = fixture
        .get("response_fixtures")
        .and_then(Value::as_array)
        .ok_or_else(|| "missing response_fixtures".to_string())?
        .iter()
        .map(|value| required_str(value, "name").map(str::to_string))
        .collect::<Result<BTreeSet<_>, _>>()?;
    for required in ["completed", "retryable-failure", "terminal-failure", "usage-accounting"] {
        if !names.contains(required) {
            return Err(format!("provider fixture missing `{required}`"));
        }
    }
    Ok(())
}

fn validate_source() -> Result<(), String> {
    let source = fs::read_to_string(SOURCE).map_err(|error| format!("failed to read {SOURCE}: {error}"))?;
    for marker in [
        "ProductProviderRequest",
        "ProductProviderResponse::Completed",
        "ProductProviderResponse::RetryableFailure",
        "ProductProviderResponse::TerminalFailure",
        "ProductModelProfile",
        "product_model_profile",
        "CollectingUsageObserver",
    ] {
        if !source.contains(marker) {
            return Err(format!("provider adapter source missing `{marker}`"));
        }
    }
    for forbidden in FORBIDDEN {
        if source.contains(forbidden) {
            return Err(format!("provider adapter source contains forbidden token `{forbidden}`"));
        }
    }
    Ok(())
}

fn validate_boundary() -> Result<(), String> {
    let manifest = fs::read_to_string(CARGO_MANIFEST).map_err(|error| format!("failed to read {CARGO_MANIFEST}: {error}"))?;
    for required in [
        "clanker-message",
        "clankers-adapters",
        "clankers-engine",
        "clankers-engine-host",
    ] {
        if !manifest.contains(required) {
            return Err(format!("provider adapter Cargo.toml missing `{required}`"));
        }
    }
    for forbidden in ["clankers-provider", "clanker-router", "clankers-config", "reqwest"] {
        if manifest.contains(forbidden) {
            return Err(format!("provider adapter Cargo.toml contains forbidden dependency `{forbidden}`"));
        }
    }
    Ok(())
}

fn validate_docs_and_spec() -> Result<(), String> {
    let docs = fs::read_to_string(DOCS).map_err(|error| format!("failed to read {DOCS}: {error}"))?;
    for marker in ["provider-adapter kit", FIXTURE, "ProductModelProfile"] {
        if !docs.contains(marker) {
            return Err(format!("SDK docs missing provider adapter marker `{marker}`"));
        }
    }
    let spec = fs::read_to_string(SPEC).map_err(|error| format!("failed to read {SPEC}: {error}"))?;
    for marker in [
        "provider-adapter-template-is-fixture-backed",
        "model-capability-profile-remains-product-owned",
        "template-dependency-boundary-is-enforced",
    ] {
        if !spec.contains(marker) {
            return Err(format!("embedded-composition spec missing `{marker}`"));
        }
    }
    Ok(())
}

fn json_file(path: &str) -> Result<Value, String> {
    let text = fs::read_to_string(path).map_err(|error| format!("failed to read {path}: {error}"))?;
    serde_json::from_str(&text).map_err(|error| format!("failed to parse {path}: {error}"))
}

fn write_json(path: &str, value: &Value) -> Result<(), String> {
    let bytes = serde_json::to_vec_pretty(value).map_err(|error| format!("failed to encode {path}: {error}"))?;
    fs::write(path, [bytes.as_slice(), b"\n"].concat()).map_err(|error| format!("failed to write {path}: {error}"))
}

fn required_str<'a>(value: &'a Value, field: &str) -> Result<&'a str, String> {
    value.get(field).and_then(Value::as_str).ok_or_else(|| format!("missing string field `{field}`"))
}

fn require_eq(value: &Value, field: &str, expected: &str) -> Result<(), String> {
    let actual = required_str(value, field)?;
    if actual != expected {
        return Err(format!("field `{field}` expected `{expected}`, got `{actual}`"));
    }
    Ok(())
}

fn hash_artifact(path: &Path) -> Result<Value, String> {
    let mut file = fs::File::open(path).map_err(|error| format!("failed to open {}: {error}", path.display()))?;
    let mut hasher = blake3::Hasher::new();
    let mut buffer = [0u8; 16 * 1024];
    let mut bytes = 0u64;
    loop {
        let read = file.read(&mut buffer).map_err(|error| format!("failed to read {}: {error}", path.display()))?;
        if read == 0 {
            break;
        }
        bytes += read as u64;
        hasher.update(&buffer[..read]);
    }
    Ok(json!({
        "path": path.to_string_lossy(),
        "bytes": bytes,
        "blake3": hasher.finalize().to_hex().to_string(),
    }))
}
