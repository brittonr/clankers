#!/usr/bin/env -S nix develop -c cargo -q -Zscript
---cargo
[package]
edition = "2024"
---

use std::fs;

fn main() {
    let source = fs::read_to_string("crates/clankers-runtime/src/lib.rs").expect("read clankers-runtime source");
    let required = [
        "tool_catalog_capability_pack_matrix_does_not_expand_dangerous_packs",
        "tool_catalog_disabled_filter_overrides_packs_with_safe_omissions",
        "tool_catalog_custom_tools_apply_collision_policy_matrix",
        "tool_catalog_extension_descriptors_require_runtime_availability_without_execute",
        "tool_catalog_metadata_query_does_not_start_extension_runtimes",
        "CapabilityPack::ReadOnly",
        "CapabilityPack::WorkspaceMutation",
        "CapabilityPack::ShellCommands",
        "CapabilityPack::Network",
        "CapabilityPack::ExternalProcesses",
        "ToolCollisionPolicy::Reject",
        "ToolCollisionPolicy::KeepExisting",
        "ToolCollisionPolicy::HostOverrides",
        "disabled_by_host_filter",
        "omissions",
        "publishable_tools",
        "execute_calls",
        "ExtensionRuntimeKind::Plugin",
    ];
    let missing: Vec<_> = required.iter().copied().filter(|needle| !source.contains(needle)).collect();
    if !missing.is_empty() {
        eprintln!("tool catalog matrix freshness failed:");
        for item in missing { eprintln!("  - missing {item}"); }
        std::process::exit(1);
    }
}
