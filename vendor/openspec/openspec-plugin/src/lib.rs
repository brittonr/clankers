//! OpenSpec WASM plugin
//!
//! Exposes spec-driven development tools to LLM agents via the clankers
//! plugin SDK. All operations are pure — filesystem access is handled by
//! the host. Tool handlers receive file contents as JSON arguments and
//! return structured results.

use clanker_plugin_sdk::prelude::*;

mod tools;

#[plugin_fn]
pub fn describe(_input: String) -> FnResult<String> {
    let meta = PluginMeta::new(
        "openspec",
        "0.1.0",
        &[
            ("spec_list", "List project specs with domains, purposes, and requirement counts"),
            (
                "spec_parse",
                "Parse a spec markdown file into structured requirements with GIVEN/WHEN/THEN scenarios",
            ),
            ("change_list", "List active OpenSpec changes with task progress summaries"),
            ("change_verify", "Verify an OpenSpec change — check task completion and spec coverage"),
            ("artifact_status", "Show artifact dependency graph state for an OpenSpec change"),
        ],
        &[],
    );
    Ok(serde_json::to_string(&meta)?)
}

#[plugin_fn]
pub fn handle_tool_call(input: String) -> FnResult<String> {
    dispatch_tools(&input, &[
        ("spec_list", tools::handle_spec_list),
        ("spec_parse", tools::handle_spec_parse),
        ("change_list", tools::handle_change_list),
        ("change_verify", tools::handle_change_verify),
        ("artifact_status", tools::handle_artifact_status),
    ])
}

#[plugin_fn]
pub fn on_event(input: String) -> FnResult<String> {
    dispatch_events(&input, "openspec", &[("agent_start", |_| "OpenSpec plugin ready".to_string())])
}
