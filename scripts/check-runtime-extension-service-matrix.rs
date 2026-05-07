#!/usr/bin/env -S nix develop -c cargo -q -Zscript
---cargo
[package]
edition = "2024"
---

use std::fs;

fn main() {
    let runtime = fs::read_to_string("crates/clankers-runtime/src/lib.rs").expect("read runtime source");
    let desktop = fs::read_to_string("src/runtime_services.rs").expect("read desktop runtime services");
    let joined = format!("{runtime}\n{desktop}");
    let required = [
        "runtime_extension_service_matrix_default_safe_fails_closed_independently",
        "runtime_extension_service_matrix_mixed_injected_absent_no_ambient_fallback",
        "runtime_extension_service_matrix_injected_error_receipts_are_redacted",
        "runtime_extension_service_matrix_safe_receipts_redact_success_denial_and_error",
        "desktop_runtime_mixed_injected_services_do_not_fall_back_to_ambient",
        "provider_router",
        "auth_store",
        "credential_pool",
        "runtime",
        "ExtensionRuntimeKind::Plugin",
        "ExtensionRuntimeKind::Mcp",
        "ExtensionRuntimeKind::Gateway",
        "disabled",
        "injected",
        "ExtensionStatus::Succeeded",
        "ExtensionStatus::Failed",
        "ExtensionStatus::Unavailable",
        "contains_secret_markers",
        "serde_json::to_string",
        "execute_calls",
        "publish_calls",
    ];
    let missing: Vec<_> = required.iter().copied().filter(|needle| !joined.contains(needle)).collect();
    if !missing.is_empty() {
        eprintln!("runtime extension service matrix freshness failed:");
        for item in missing { eprintln!("  - missing {item}"); }
        std::process::exit(1);
    }
}
