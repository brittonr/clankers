use std::env;
use std::path::PathBuf;
use std::process::Command;
use std::sync::OnceLock;

use extism::Manifest;
use extism::Plugin;
use extism::Wasm;
use serde::Deserialize;
use serde_json::json;
use serde_json::Value;

const BUILD_PROFILE_DIR: &str = "debug";
const CARGO_BUILD_SUBCOMMAND: &str = "build";
const DESCRIBE_FUNCTION: &str = "describe";
const EXPECTED_TOOL_COUNT: usize = 5;
const HANDLE_TOOL_CALL_FUNCTION: &str = "handle_tool_call";
const ON_EVENT_FUNCTION: &str = "on_event";
const PLUGIN_NAME: &str = "openspec";
const RUSTC_WRAPPER_ENV: &str = "RUSTC_WRAPPER";
const TARGET_DIR_NAME: &str = "openspec-plugin-extism-tests";
const TOOL_STATUS_OK: &str = "ok";
const UNKNOWN_TOOL_STATUS: &str = "unknown_tool";
const WASM_FILENAME: &str = "openspec_plugin.wasm";
const WASM_TARGET_TRIPLE: &str = "wasm32-unknown-unknown";

const EXPECTED_TOOL_NAMES: [&str; EXPECTED_TOOL_COUNT] = [
    "spec_list",
    "spec_parse",
    "change_list",
    "change_verify",
    "artifact_status",
];

const SAMPLE_SPEC_CONTENT: &str = "## Purpose
Demo spec

## Requirement
The system MUST parse plugin tool requests.
Given a valid request
When the tool parses it
Then the response includes one requirement.";

static WASM_PATH: OnceLock<PathBuf> = OnceLock::new();

#[derive(Debug, Deserialize)]
struct PluginMetaEnvelope {
    name: String,
    tools: Vec<ToolMetaEnvelope>,
}

#[derive(Debug, Deserialize)]
struct ToolMetaEnvelope {
    name: String,
}

#[derive(Debug, Deserialize)]
struct ToolCallEnvelope {
    status: String,
    result: String,
}

#[derive(Debug, Deserialize)]
struct EventEnvelope {
    event: String,
    handled: bool,
    message: String,
}

fn plugin_crate_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn wasm_target_dir() -> PathBuf {
    env::temp_dir().join(TARGET_DIR_NAME)
}

fn cargo_binary() -> String {
    env::var("CARGO").unwrap_or_else(|_| String::from("cargo"))
}

fn build_plugin_wasm() -> Result<PathBuf, String> {
    let target_dir = wasm_target_dir();
    let output = Command::new(cargo_binary())
        .current_dir(plugin_crate_dir())
        .env(RUSTC_WRAPPER_ENV, "")
        .arg(CARGO_BUILD_SUBCOMMAND)
        .arg("--target")
        .arg(WASM_TARGET_TRIPLE)
        .arg("--target-dir")
        .arg(&target_dir)
        .arg("--locked")
        .output()
        .map_err(|error| format!("failed to spawn cargo build: {error}"))?;

    if !output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("cargo build --target {WASM_TARGET_TRIPLE} failed\nstdout:\n{stdout}\nstderr:\n{stderr}"));
    }

    let wasm_path = target_dir.join(WASM_TARGET_TRIPLE).join(BUILD_PROFILE_DIR).join(WASM_FILENAME);
    if !wasm_path.is_file() {
        return Err(format!("expected wasm artifact at {}", wasm_path.display()));
    }

    Ok(wasm_path)
}

fn wasm_path() -> &'static PathBuf {
    WASM_PATH.get_or_init(|| build_plugin_wasm().expect("failed to build openspec plugin wasm"))
}

fn load_plugin() -> Plugin {
    let manifest = Manifest::new([Wasm::file(wasm_path())]);
    Plugin::new(manifest, [], true).expect("failed to load openspec plugin via extism")
}

fn parse_json<T>(raw: &str, context: &str) -> T
where T: for<'de> Deserialize<'de> {
    serde_json::from_str(raw).unwrap_or_else(|error| panic!("{context}: {error}; raw={raw}"))
}

fn call_tool(plugin: &mut Plugin, tool: &str, args: Value) -> ToolCallEnvelope {
    let raw = plugin
        .call::<String, String>(HANDLE_TOOL_CALL_FUNCTION, json!({ "tool": tool, "args": args }).to_string())
        .unwrap_or_else(|error| panic!("tool {tool} call failed: {error}"));
    parse_json(&raw, "invalid tool response")
}

fn parse_ok_tool_json(envelope: ToolCallEnvelope) -> Value {
    assert_eq!(envelope.status, TOOL_STATUS_OK, "unexpected tool status");
    parse_json(&envelope.result, "invalid ok tool payload")
}

fn call_event(plugin: &mut Plugin, event: &str, data: Value) -> EventEnvelope {
    let raw = plugin
        .call::<String, String>(ON_EVENT_FUNCTION, json!({ "event": event, "data": data }).to_string())
        .unwrap_or_else(|error| panic!("event {event} call failed: {error}"));
    parse_json(&raw, "invalid event response")
}

#[test]
fn describe_and_agent_start_event_work_via_extism() {
    let mut plugin = load_plugin();

    let describe_raw = plugin.call::<&str, String>(DESCRIBE_FUNCTION, "null").expect("describe call failed");
    let describe: PluginMetaEnvelope = parse_json(&describe_raw, "invalid describe response");
    assert_eq!(describe.name, PLUGIN_NAME);

    let tool_names: Vec<&str> = describe.tools.iter().map(|tool| tool.name.as_str()).collect();
    assert_eq!(tool_names, EXPECTED_TOOL_NAMES);

    let event = call_event(&mut plugin, "agent_start", json!({}));
    assert_eq!(event.event, "agent_start");
    assert!(event.handled, "agent_start should be handled");
    assert_eq!(event.message, "OpenSpec plugin ready");
}

#[test]
fn all_tools_return_expected_payloads_via_extism() {
    let mut plugin = load_plugin();

    let spec_list = parse_ok_tool_json(call_tool(
        &mut plugin,
        "spec_list",
        json!({
            "entries": [{
                "domain": "demo",
                "content": SAMPLE_SPEC_CONTENT,
            }],
        }),
    ));
    let spec_entries = spec_list.as_array().expect("spec_list should return an array");
    assert_eq!(spec_entries.len(), 1);
    assert_eq!(spec_entries[0]["domain"], "demo");
    assert_eq!(spec_entries[0]["requirement_count"], 1);

    let spec_parse = parse_ok_tool_json(call_tool(
        &mut plugin,
        "spec_parse",
        json!({
            "domain": "demo",
            "content": SAMPLE_SPEC_CONTENT,
        }),
    ));
    assert_eq!(spec_parse["domain"], "demo");
    assert_eq!(spec_parse["requirements"][0]["heading"], "Requirement");
    assert_eq!(spec_parse["requirements"][0]["strength"], "Must");
    assert_eq!(spec_parse["requirements"][0]["scenarios"][0]["given"][0], "Given a valid request");

    let change_list = parse_ok_tool_json(call_tool(
        &mut plugin,
        "change_list",
        json!({
            "entries": [{
                "name": "demo-change",
                "meta_content": "schema: spec-driven\ncreated: 2026-04-24T00:00:00Z\n",
                "tasks_content": "- [x] done\n- [ ] todo\n",
            }],
        }),
    ));
    let change_entries = change_list.as_array().expect("change_list should return an array");
    assert_eq!(change_entries.len(), 1);
    assert_eq!(change_entries[0]["name"], "demo-change");
    assert_eq!(change_entries[0]["task_progress"]["done"], 1);
    assert_eq!(change_entries[0]["task_progress"]["todo"], 1);

    let change_verify = parse_ok_tool_json(call_tool(
        &mut plugin,
        "change_verify",
        json!({
            "tasks_content": "- [x] done\n",
            "has_specs_dir": true,
        }),
    ));
    assert_eq!(change_verify["summary"], "0 critical, 0 warnings, 0 suggestions");
    assert_eq!(change_verify["has_critical"], false);

    let artifact_status = parse_ok_tool_json(call_tool(
        &mut plugin,
        "artifact_status",
        json!({
            "schema_artifacts": [
                {
                    "id": "proposal",
                    "generates": "proposal.md",
                    "requires": [],
                },
                {
                    "id": "specs",
                    "generates": "specs/**/*.md",
                    "requires": ["proposal"],
                },
            ],
            "existing_files": ["proposal.md"],
        }),
    ));
    assert_eq!(artifact_status["next_ready"], "specs");
    assert_eq!(artifact_status["is_complete"], false);
}

#[test]
fn invalid_and_unknown_tool_calls_fail_cleanly_via_extism() {
    let mut plugin = load_plugin();

    let invalid_spec_parse = call_tool(&mut plugin, "spec_parse", json!({}));
    assert!(invalid_spec_parse.status.starts_with("error:"), "expected invalid args to return an error status");
    assert!(invalid_spec_parse.result.contains("missing 'content' string"), "expected missing content error");

    let unknown_tool = call_tool(&mut plugin, "missing_tool", json!({}));
    assert_eq!(unknown_tool.status, UNKNOWN_TOOL_STATUS);
    assert!(unknown_tool.result.is_empty(), "unknown tool should not return a payload");
}
