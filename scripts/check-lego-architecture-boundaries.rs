#!/usr/bin/env -S nix develop -c cargo -q -Zscript
---cargo
[package]
edition = "2024"

[dependencies]
blake3 = "1"
serde_json = "1"
syn = { version = "2", features = ["full", "visit"] }
---

use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::fs;
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;
use std::process::ExitCode;

use serde_json::Value;
use serde_json::json;
use syn::visit::Visit;

const ERROR_EXIT: u8 = 1;
const ROOT_PACKAGE: &str = "clankers";
const AGENT_PACKAGE: &str = "clankers-agent";
const CONTROLLER_PACKAGE: &str = "clankers-controller";
const DEFAULT_OUTPUT: &str = "target/lego-architecture/dependency-ownership-inventory.json";
const BASELINE: &str = "policy/lego-architecture/dependency-ownership-baseline.json";
const CHANGE_TASKS_ACTIVE: &str = "cairn/changes/lego-decoupling-boundaries/tasks.md";
const CHANGE_SPEC_ACTIVE: &str = "cairn/changes/lego-decoupling-boundaries/specs/lego-architecture-boundaries/spec.md";
const CHANGE_TASKS_ARCHIVE: &str = "cairn/archive/2026-05-21-lego-decoupling-boundaries/tasks.md";
const CHANGE_SPEC_ARCHIVE: &str =
    "cairn/archive/2026-05-21-lego-decoupling-boundaries/specs/lego-architecture-boundaries/spec.md";
const ACCEPTED_SPEC: &str = "cairn/specs/lego-architecture-boundaries/spec.md";
const PROCESS_TOOL: &str = "src/tools/process.rs";
const PROCESS_TOOL_ADAPTER: &str = "src/tools/process/adapter.rs";
const AGENT_TURN_MOD: &str = "crates/clankers-agent/src/turn/mod.rs";
const AGENT_TURN_ADAPTERS: &str = "crates/clankers-agent/src/turn/adapters.rs";
const AGENT_TURN_PORTS: &str = "crates/clankers-agent/src/turn/ports.rs";
const CONTROLLER_CORE_EFFECTS: &str = "crates/clankers-controller/src/core_effects.rs";
const CONTROLLER_CONVERT: &str = "crates/clankers-controller/src/convert.rs";
const CONTROLLER_DOMAIN_EVENT: &str = "crates/clankers-controller/src/domain_event.rs";
const CONTROLLER_EFFECT_INTERPRETATION: &str = "crates/clankers-controller/src/effect_interpretation.rs";
const PROVIDER_ROUTER_BRIDGE: &str = "crates/clankers-provider/src/router_request_bridge.rs";
const PROVIDER_ROUTER_ADAPTER: &str = "crates/clankers-provider/src/router.rs";
const PROVIDER_RPC_ADAPTER: &str = "crates/clankers-provider/src/rpc_provider.rs";
const SESSION_COMMAND_POLICY: &str = "src/modes/session_command_policy.rs";
const ATTACH_COMMANDS: &str = "src/modes/attach/commands.rs";
const AGENT_TASK: &str = "src/modes/agent_task.rs";

const AGENT_CONCRETE_DEPS: &[&str] = &[
    "clankers-config",
    "clankers-db",
    "clankers-hooks",
    "clankers-model-selection",
    "clankers-procmon",
    "clankers-prompts",
    "clankers-provider",
    "clanker-router",
    "clankers-skills",
    "clanker-tui-types",
    "clankers-util",
];

const CONTROLLER_CONCRETE_DEPS: &[&str] = &[
    "clankers-agent",
    "clankers-config",
    "clankers-db",
    "clankers-hooks",
    "clankers-provider",
    "clankers-protocol",
    "clankers-session",
    "clanker-tui-types",
];

const DTO_PACKAGES: &[&str] = &["clanker-message", "clanker-tui-types", "clankers-protocol"];

fn main() -> ExitCode {
    match run() {
        Ok(path) => {
            println!("lego architecture dependency ownership inventory written to {}", path.display());
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("lego architecture dependency ownership check failed: {error}");
            ExitCode::from(ERROR_EXIT)
        }
    }
}

fn run() -> Result<PathBuf, String> {
    validate_change_contracts()?;
    let metadata = cargo_metadata()?;
    let workspace_packages = workspace_package_names(&metadata)?;
    let dependency_map = dependency_map(&metadata, &workspace_packages)?;
    let reverse_map = reverse_dependency_map(&dependency_map, &workspace_packages);

    let root_internal = internal_deps(ROOT_PACKAGE, &dependency_map)?;
    let agent_internal = internal_deps(AGENT_PACKAGE, &dependency_map)?;
    let controller_internal = internal_deps(CONTROLLER_PACKAGE, &dependency_map)?;
    let agent_concrete = matching_deps(&agent_internal, AGENT_CONCRETE_DEPS);
    let controller_concrete = matching_deps(&controller_internal, CONTROLLER_CONCRETE_DEPS);
    let shared_dtos = shared_dto_crates(&reverse_map);
    let process_tool_adapter = process_tool_adapter_signature()?;
    let agent_turn_ports = agent_turn_ports_signature()?;
    let controller_effect_interpretation = controller_effect_interpretation_signature()?;
    let provider_router_bridge = provider_router_bridge_signature()?;
    let controller_domain_event = controller_domain_event_signature()?;
    let session_command_policy = session_command_policy_signature()?;

    require_nonempty(&root_internal, "root internal dependency inventory")?;
    require_nonempty(&agent_concrete, "agent concrete dependency inventory")?;
    require_nonempty(&controller_concrete, "controller concrete dependency inventory")?;
    require_nonempty(&shared_dtos, "shared DTO crate inventory")?;

    let signature = json!({
        "root_crate": {
            "package": ROOT_PACKAGE,
            "internal_dependency_count": root_internal.len(),
            "internal_dependencies": root_internal,
            "owner": "product-shell wiring only; reusable behavior must move behind workspace brick APIs"
        },
        "agent_crate": {
            "package": AGENT_PACKAGE,
            "internal_dependency_count": agent_internal.len(),
            "concrete_dependency_count": agent_concrete.len(),
            "concrete_dependencies": agent_concrete,
            "owner": "turn orchestration shell; concrete systems should migrate behind model/tool/config/storage/hook/prompt/skill/cost ports"
        },
        "controller_crate": {
            "package": CONTROLLER_PACKAGE,
            "internal_dependency_count": controller_internal.len(),
            "concrete_dependency_count": controller_concrete.len(),
            "concrete_dependencies": controller_concrete,
            "owner": "session orchestration shell; translation, effect, continuation, persistence, and projection seams stay separately testable"
        },
        "most_shared_dto_crates": shared_dtos,
        "process_tool_adapter": process_tool_adapter,
        "agent_turn_ports": agent_turn_ports,
        "controller_effect_interpretation": controller_effect_interpretation,
        "provider_router_bridge": provider_router_bridge,
        "controller_domain_event": controller_domain_event,
        "session_command_policy": session_command_policy,
    });
    validate_baseline(&signature)?;

    let change_tasks = change_tasks_path()?;
    let change_spec = change_spec_path()?;
    let receipt = json!({
        "schema": "clankers.lego_architecture.dependency_ownership_inventory.v1",
        "change": "lego-decoupling-boundaries",
        "requirements": [
            "r[lego-architecture-boundaries.root-shell-thinness]",
            "r[lego-architecture-boundaries.process-tool-thin-adapter]",
            "r[lego-architecture-boundaries.agent-uses-ports-not-concrete-systems]",
            "r[lego-architecture-boundaries.controller-seams-are-single-purpose]",
            "r[lego-architecture-boundaries.display-and-protocol-types-do-not-leak-inward]",
            "r[lego-architecture-boundaries.attach-parity-uses-shared-policy-core]",
            "r[lego-architecture-boundaries.typed-architecture-rails]"
        ],
        "inventory_signature": signature,
        "rail_kind": "typed cargo metadata plus Rust AST boundary rails",
        "baseline": BASELINE,
        "source_artifacts": [change_tasks, change_spec, ACCEPTED_SPEC],
        "artifact_hashes": [
            hash_artifact(Path::new(BASELINE))?,
            hash_artifact(Path::new(change_tasks))?,
            hash_artifact(Path::new(change_spec))?,
            hash_artifact(Path::new(ACCEPTED_SPEC))?,
            hash_artifact(Path::new(PROCESS_TOOL))?,
            hash_artifact(Path::new(PROCESS_TOOL_ADAPTER))?,
            hash_artifact(Path::new(AGENT_TURN_MOD))?,
            hash_artifact(Path::new(AGENT_TURN_ADAPTERS))?,
            hash_artifact(Path::new(AGENT_TURN_PORTS))?,
            hash_artifact(Path::new(CONTROLLER_CORE_EFFECTS))?,
            hash_artifact(Path::new(CONTROLLER_CONVERT))?,
            hash_artifact(Path::new(CONTROLLER_DOMAIN_EVENT))?,
            hash_artifact(Path::new(CONTROLLER_EFFECT_INTERPRETATION))?,
            hash_artifact(Path::new(PROVIDER_ROUTER_BRIDGE))?,
            hash_artifact(Path::new(PROVIDER_ROUTER_ADAPTER))?,
            hash_artifact(Path::new(PROVIDER_RPC_ADAPTER))?,
            hash_artifact(Path::new(SESSION_COMMAND_POLICY))?,
            hash_artifact(Path::new(ATTACH_COMMANDS))?,
            hash_artifact(Path::new(AGENT_TASK))?
        ]
    });

    write_receipt(&receipt)
}

fn validate_change_contracts() -> Result<(), String> {
    let tasks_path = change_tasks_path()?;
    let spec_path = change_spec_path()?;
    let tasks = fs::read_to_string(tasks_path).map_err(|error| format!("failed to read {tasks_path}: {error}"))?;
    let spec = fs::read_to_string(spec_path).map_err(|error| format!("failed to read {spec_path}: {error}"))?;
    require_contains(&tasks, "dependency ownership inventory", "tasks I1/V1 dependency inventory")?;
    require_contains(&tasks, "r[lego-architecture-boundaries.root-shell-thinness]", "root shell task coverage")?;
    require_contains(&tasks, "r[lego-architecture-boundaries.typed-architecture-rails]", "typed rail task coverage")?;
    require_contains(&tasks, "process` tool JSON adapter boundary", "process tool adapter extraction task")?;
    require_contains(&tasks, "Define agent turn ports", "agent turn port extraction task")?;
    require_contains(&tasks, "Split controller seams", "controller seam split task")?;
    require_contains(&tasks, "Assign provider/router ownership", "provider/router ownership task")?;
    require_contains(&tasks, "neutral domain event and receipt DTOs", "neutral domain event task")?;
    require_contains(&tasks, "shared session command/effect/ack policy", "session command policy task")?;
    require_contains(&spec, "dependency owner", "dependency owner requirement text")?;
    require_contains(&spec, "source crate, target crate", "typed dependency diagnostic scenario")?;
    require_contains(
        &spec,
        "adapter MUST NOT construct or mutate persisted database DTOs",
        "process adapter storage DTO boundary",
    )?;
    require_contains(&spec, "Controller seams are single-purpose", "controller seam single-purpose requirement")?;
    require_contains(&spec, "Provider/router has one owner per concern", "provider/router single-owner requirement")?;
    require_contains(&spec, "Display and protocol types do not leak inward", "display/protocol edge requirement")?;
    require_contains(&spec, "Attach parity uses shared policy core", "attach parity shared policy requirement")?;
    Ok(())
}

fn change_tasks_path() -> Result<&'static str, String> {
    first_existing_path(CHANGE_TASKS_ACTIVE, CHANGE_TASKS_ARCHIVE, "lego decoupling tasks")
}

fn change_spec_path() -> Result<&'static str, String> {
    first_existing_path(CHANGE_SPEC_ACTIVE, CHANGE_SPEC_ARCHIVE, "lego decoupling spec")
}

fn first_existing_path(active: &'static str, archived: &'static str, label: &str) -> Result<&'static str, String> {
    if Path::new(active).exists() {
        return Ok(active);
    }
    if Path::new(archived).exists() {
        return Ok(archived);
    }
    Err(format!("missing {label}: checked {active} and {archived}"))
}

fn cargo_metadata() -> Result<Value, String> {
    let output = Command::new("cargo")
        .args(["metadata", "--no-deps", "--format-version", "1"])
        .output()
        .map_err(|error| format!("failed to run cargo metadata: {error}"))?;
    if !output.status.success() {
        return Err(format!("cargo metadata failed: {}", String::from_utf8_lossy(&output.stderr)));
    }
    serde_json::from_slice(&output.stdout).map_err(|error| format!("failed to parse cargo metadata JSON: {error}"))
}

fn workspace_package_names(metadata: &Value) -> Result<BTreeSet<String>, String> {
    let packages = metadata
        .get("packages")
        .and_then(Value::as_array)
        .ok_or_else(|| "cargo metadata missing packages".to_string())?;
    let members = metadata
        .get("workspace_members")
        .and_then(Value::as_array)
        .ok_or_else(|| "cargo metadata missing workspace_members".to_string())?;
    let member_ids: BTreeSet<&str> = members.iter().filter_map(Value::as_str).collect();
    let mut names = BTreeSet::new();
    for package in packages {
        let id = required_str(package, "id")?;
        if member_ids.contains(id) {
            names.insert(required_str(package, "name")?.to_string());
        }
    }
    Ok(names)
}

fn dependency_map(
    metadata: &Value,
    workspace_packages: &BTreeSet<String>,
) -> Result<BTreeMap<String, BTreeSet<String>>, String> {
    let packages = metadata
        .get("packages")
        .and_then(Value::as_array)
        .ok_or_else(|| "cargo metadata missing packages".to_string())?;
    let mut map = BTreeMap::new();
    for package in packages {
        let name = required_str(package, "name")?.to_string();
        if !workspace_packages.contains(&name) {
            continue;
        }
        let deps = package
            .get("dependencies")
            .and_then(Value::as_array)
            .ok_or_else(|| format!("package {name} missing dependencies"))?;
        let mut internal = BTreeSet::new();
        for dep in deps {
            let dep_name = required_str(dep, "name")?;
            if workspace_packages.contains(dep_name) && dep_name != name {
                internal.insert(dep_name.to_string());
            }
        }
        map.insert(name, internal);
    }
    Ok(map)
}

fn reverse_dependency_map(
    dependency_map: &BTreeMap<String, BTreeSet<String>>,
    workspace_packages: &BTreeSet<String>,
) -> BTreeMap<String, BTreeSet<String>> {
    let mut reverse: BTreeMap<String, BTreeSet<String>> =
        workspace_packages.iter().map(|name| (name.clone(), BTreeSet::new())).collect();
    for (source, deps) in dependency_map {
        for dep in deps {
            if let Some(users) = reverse.get_mut(dep) {
                users.insert(source.clone());
            }
        }
    }
    reverse
}

fn internal_deps(package: &str, dependency_map: &BTreeMap<String, BTreeSet<String>>) -> Result<Vec<String>, String> {
    let deps = dependency_map.get(package).ok_or_else(|| format!("missing workspace package `{package}`"))?;
    Ok(deps.iter().cloned().collect())
}

fn matching_deps(actual: &[String], expected: &[&str]) -> Vec<String> {
    actual.iter().filter(|dep| expected.contains(&dep.as_str())).cloned().collect()
}

fn shared_dto_crates(reverse_map: &BTreeMap<String, BTreeSet<String>>) -> Vec<Value> {
    let mut dtos: Vec<Value> =
        DTO_PACKAGES
            .iter()
            .filter_map(|name| {
                reverse_map.get(*name).map(|users| json!({
                "package": name,
                "dependent_count": users.len(),
                "dependents": users.iter().cloned().collect::<Vec<_>>(),
                "owner": "shared DTO/display/protocol type; inward dependencies require explicit edge justification"
            }))
            })
            .collect();
    dtos.sort_by(|left, right| {
        let left_count = left.get("dependent_count").and_then(Value::as_u64).unwrap_or(0);
        let right_count = right.get("dependent_count").and_then(Value::as_u64).unwrap_or(0);
        right_count.cmp(&left_count)
    });
    dtos
}

fn process_tool_adapter_signature() -> Result<Value, String> {
    let tool_file = read_rust_file(PROCESS_TOOL)?;
    let adapter_file = read_rust_file(PROCESS_TOOL_ADAPTER)?;
    let tool = &tool_file.source;
    let adapter = &adapter_file.source;
    require_rust_mod(&tool_file, "adapter", "process tool adapter module")?;
    require_contains(&tool, "mod adapter;", "process tool adapter module")?;
    require_contains(
        &tool,
        "ProcessToolJsonAdapter::process_job_tool_request(params)",
        "process tool request parser delegation",
    )?;
    forbid_rust_path(&adapter_file, "clankers_db", "process adapter storage DTO import")?;
    forbid_rust_path(&adapter_file, "StoredProcessJob", "process adapter persisted DTO reference")?;
    forbid_contains(&adapter, "clankers_db::", "process adapter storage DTO import")?;
    forbid_contains(&adapter, "StoredProcessJob", "process adapter persisted DTO reference")?;
    require_rust_method(&adapter_file, "process_job_tool_request", "process adapter typed request parser entrypoint")?;
    require_contains(
        &adapter,
        "pub(super) fn process_job_tool_request",
        "process adapter typed request parser entrypoint",
    )?;
    require_rust_path(&adapter_file, "ProcessJobToolRequest::Start", "process adapter typed start projection")?;
    require_contains(&adapter, "ProcessJobToolRequest::Start", "process adapter typed start projection")?;
    require_contains(&adapter, "Unknown process action", "process adapter fail-closed unsupported action")?;
    Ok(json!({
        "adapter_module": PROCESS_TOOL_ADAPTER,
        "tool_module": PROCESS_TOOL,
        "storage_dto_imports": 0,
        "storage_dto_references": 0,
        "request_parser_owner": "ProcessToolJsonAdapter",
        "fail_closed_negative_path": "unsupported action returns ToolResult error before backend dispatch",
        "typed_rail_kind": "Rust AST module, method, path, and forbidden dependency checks"
    }))
}

fn agent_turn_ports_signature() -> Result<Value, String> {
    let turn_mod_file = read_rust_file(AGENT_TURN_MOD)?;
    let adapters_file = read_rust_file(AGENT_TURN_ADAPTERS)?;
    let ports_file = read_rust_file(AGENT_TURN_PORTS)?;
    let turn_mod = &turn_mod_file.source;
    let adapters = &adapters_file.source;
    let ports = &ports_file.source;

    require_rust_mod(&turn_mod_file, "ports", "agent turn ports module")?;
    require_contains(&turn_mod, "mod ports;", "agent turn ports module")?;
    require_contains(&turn_mod, "ProviderModelPort::new(ctx.provider)", "provider adapter construction")?;
    require_contains(&turn_mod, "ControllerToolPort", "tool adapter construction")?;
    require_struct_field_type_path(&adapters_file, "AgentModelHost", "model_port", "AgentModelPort", "agent model host port field")?;
    require_struct_field_type_path(&adapters_file, "AgentToolHost", "tool_port", "AgentToolPort", "agent tool host port field")?;
    require_contains(&adapters, "model_port: &'a dyn AgentModelPort", "agent model host port field")?;
    require_contains(&adapters, "tool_port: &'a dyn AgentToolPort", "agent tool host port field")?;
    forbid_struct_field_type_path(&adapters_file, "AgentModelHost", "provider", "Provider", "agent model host concrete provider field")?;
    forbid_struct_field_type_path(&adapters_file, "AgentToolHost", "controller_tools", "HashMap", "agent tool host concrete tool map field")?;
    forbid_contains(&adapters, "provider: &'a dyn Provider", "agent model host concrete provider field")?;
    forbid_contains(&adapters, "controller_tools: &'a HashMap", "agent tool host concrete tool map field")?;
    require_rust_trait(&ports_file, "AgentModelPort", "agent model port trait")?;
    require_rust_trait(&ports_file, "AgentToolPort", "agent tool port trait")?;
    require_rust_impl(&ports_file, "AgentModelPort", "ProviderModelPort", "provider model port adapter")?;
    require_rust_impl(&ports_file, "AgentToolPort", "ControllerToolPort", "controller tool port adapter")?;
    require_contains(&ports, "trait AgentModelPort", "agent model port trait")?;
    require_contains(&ports, "trait AgentToolPort", "agent tool port trait")?;
    require_contains(&ports, "impl AgentModelPort for ProviderModelPort", "provider model port adapter")?;
    require_contains(&ports, "impl AgentToolPort for ControllerToolPort", "controller tool port adapter")?;

    Ok(json!({
        "ports_module": AGENT_TURN_PORTS,
        "model_port_trait": "AgentModelPort",
        "tool_port_trait": "AgentToolPort",
        "model_host_concrete_provider_fields": 0,
        "tool_host_concrete_tool_map_fields": 0,
        "provider_adapter": "ProviderModelPort",
        "tool_adapter": "ControllerToolPort",
        "typed_rail_kind": "Rust AST module, trait, impl, and struct-field checks"
    }))
}

fn controller_effect_interpretation_signature() -> Result<Value, String> {
    let core_effects_file = read_rust_file(CONTROLLER_CORE_EFFECTS)?;
    let interpretation_file = read_rust_file(CONTROLLER_EFFECT_INTERPRETATION)?;
    let core_effects = &core_effects_file.source;
    let interpretation = &interpretation_file.source;

    require_rust_path(&core_effects_file, "effect_interpretation::interpret_prompt_request", "controller prompt effect interpretation seam")?;
    require_rust_path(&core_effects_file, "effect_interpretation::interpret_thinking_change", "controller thinking effect interpretation seam")?;
    require_rust_path(&core_effects_file, "effect_interpretation::interpret_tool_filter_application", "controller tool filter effect interpretation seam")?;
    require_contains(
        &core_effects,
        "effect_interpretation::interpret_prompt_request",
        "controller prompt effect interpretation seam",
    )?;
    require_contains(
        &core_effects,
        "effect_interpretation::interpret_thinking_change",
        "controller thinking effect interpretation seam",
    )?;
    require_contains(
        &core_effects,
        "effect_interpretation::interpret_tool_filter_application",
        "controller tool filter effect interpretation seam",
    )?;
    require_contains(
        &interpretation,
        "Pure interpretation seam for clankers-core effects",
        "controller effect interpretation module purpose",
    )?;
    require_rust_struct(&interpretation_file, "ToolFilterApplication", "typed tool-filter effect projection")?;
    require_rust_fn(&interpretation_file, "interpret_prompt_request", "typed prompt effect projection")?;
    require_rust_fn(&interpretation_file, "interpret_thinking_change", "typed thinking effect projection")?;
    require_rust_fn(&interpretation_file, "disabled_tools_changed", "typed disabled-tools event projection")?;
    require_contains(&interpretation, "struct ToolFilterApplication", "typed tool-filter effect projection")?;
    require_contains(&interpretation, "interpret_prompt_request", "typed prompt effect projection")?;
    require_contains(&interpretation, "interpret_thinking_change", "typed thinking effect projection")?;
    require_contains(&interpretation, "disabled_tools_changed", "typed disabled-tools event projection")?;
    forbid_rust_path(&interpretation_file, "SessionController", "pure effect interpretation controller mutation")?;
    forbid_rust_path(&interpretation_file, "DaemonEvent", "pure effect interpretation protocol projection")?;
    forbid_rust_path(&interpretation_file, "clankers_agent", "pure effect interpretation agent runtime dependency")?;
    forbid_contains(&interpretation, "SessionController", "pure effect interpretation controller mutation")?;
    forbid_contains(&interpretation, "DaemonEvent", "pure effect interpretation protocol projection")?;
    forbid_contains(&interpretation, "clankers_agent", "pure effect interpretation agent runtime dependency")?;

    Ok(json!({
        "interpretation_module": CONTROLLER_EFFECT_INTERPRETATION,
        "shell_module": CONTROLLER_CORE_EFFECTS,
        "prompt_projection": "interpret_prompt_request",
        "thinking_projection": "interpret_thinking_change",
        "tool_filter_projection": "interpret_tool_filter_application",
        "protocol_projection_references": 0,
        "agent_runtime_references": 0,
        "typed_rail_kind": "Rust AST function, struct, path, and forbidden dependency checks"
    }))
}

fn provider_router_bridge_signature() -> Result<Value, String> {
    let bridge_file = read_rust_file(PROVIDER_ROUTER_BRIDGE)?;
    let router_adapter_file = read_rust_file(PROVIDER_ROUTER_ADAPTER)?;
    let rpc_adapter_file = read_rust_file(PROVIDER_RPC_ADAPTER)?;
    let bridge = &bridge_file.source;
    let router_adapter = &router_adapter_file.source;
    let rpc_adapter = &rpc_adapter_file.source;

    require_rust_fn(&bridge_file, "build_router_request", "provider/router bridge entrypoint")?;
    require_rust_fn(&bridge_file, "messages_to_router_json", "provider/router message projection owner")?;
    require_rust_path(&router_adapter_file, "crate::router_request_bridge::build_router_request", "local router adapter delegates request projection")?;
    require_rust_path(&rpc_adapter_file, "crate::router_request_bridge::build_router_request", "rpc router adapter delegates request projection")?;
    require_contains(
        &bridge,
        "Single clankers-provider owned bridge into `clanker_router::CompletionRequest`",
        "provider/router bridge ownership doc",
    )?;
    require_contains(&bridge, "pub(crate) fn build_router_request", "provider/router bridge entrypoint")?;
    require_contains(&bridge, "messages_to_router_json", "provider/router message projection owner")?;
    require_contains(&bridge, "Branch summary", "branch summary preservation fixture")?;
    require_contains(&bridge, "Compaction summary", "compaction summary preservation fixture")?;
    require_contains(
        &router_adapter,
        "crate::router_request_bridge::build_router_request(request)",
        "local router adapter delegates request projection",
    )?;
    require_contains(
        &rpc_adapter,
        "crate::router_request_bridge::build_router_request(request)",
        "rpc router adapter delegates request projection",
    )?;
    forbid_rust_fn(&router_adapter_file, "messages_to_router_json", "local router adapter duplicate message projection")?;
    forbid_rust_fn(&rpc_adapter_file, "convert_messages_to_api", "rpc router adapter duplicate message projection")?;
    forbid_rust_fn(&rpc_adapter_file, "content_to_json", "rpc router adapter duplicate content projection")?;
    forbid_contains(
        &router_adapter,
        "fn messages_to_router_json",
        "local router adapter duplicate message projection",
    )?;
    forbid_contains(&rpc_adapter, "fn convert_messages_to_api", "rpc router adapter duplicate message projection")?;
    forbid_contains(&rpc_adapter, "fn content_to_json", "rpc router adapter duplicate content projection")?;

    Ok(json!({
        "bridge_module": PROVIDER_ROUTER_BRIDGE,
        "local_adapter_module": PROVIDER_ROUTER_ADAPTER,
        "rpc_adapter_module": PROVIDER_RPC_ADAPTER,
        "request_projection_owner": "router_request_bridge::build_router_request",
        "local_adapter_duplicate_message_projection": 0,
        "rpc_adapter_duplicate_message_projection": 0,
        "summary_context_preserved": true,
        "typed_rail_kind": "Rust AST function ownership and call-path checks"
    }))
}

fn controller_domain_event_signature() -> Result<Value, String> {
    let domain_event_file = read_rust_file(CONTROLLER_DOMAIN_EVENT)?;
    let convert_file = read_rust_file(CONTROLLER_CONVERT)?;
    let domain_event = &domain_event_file.source;
    let convert = &convert_file.source;

    require_rust_enum(&domain_event_file, "ControllerDomainEvent", "neutral controller event enum")?;
    require_rust_struct(&domain_event_file, "DomainImage", "neutral image receipt DTO")?;
    require_rust_fn(&domain_event_file, "agent_event_to_domain_event", "agent/runtime event to neutral domain event projection")?;
    require_rust_fn(&domain_event_file, "tool_content_to_domain_parts", "neutral tool receipt projection")?;
    forbid_rust_path(&domain_event_file, "DaemonEvent", "domain event protocol DTO leakage")?;
    forbid_rust_path(&domain_event_file, "TuiEvent", "domain event TUI DTO leakage")?;
    forbid_rust_path(&domain_event_file, "clankers_protocol", "domain event protocol crate dependency")?;
    forbid_rust_path(&domain_event_file, "clanker_tui_types", "domain event TUI crate dependency")?;
    require_rust_path(&convert_file, "agent_event_to_domain_event", "protocol projection delegates through neutral domain event seam")?;
    require_rust_path(&convert_file, "domain_event_to_daemon_event", "protocol projection delegates through neutral domain event seam")?;

    require_contains(
        &domain_event,
        "Neutral controller domain events projected from agent/runtime events",
        "controller domain event seam purpose",
    )?;
    require_contains(&domain_event, "enum ControllerDomainEvent", "neutral controller event enum")?;
    require_contains(&domain_event, "struct DomainImage", "neutral image receipt DTO")?;
    require_contains(
        &domain_event,
        "agent_event_to_domain_event",
        "agent/runtime event to neutral domain event projection",
    )?;
    require_contains(&domain_event, "tool_content_to_domain_parts", "neutral tool receipt projection")?;
    require_contains(
        &domain_event,
        "projects_agent_streaming_without_protocol_or_tui_types",
        "neutral streaming projection fixture",
    )?;
    require_contains(
        &domain_event,
        "projects_tool_receipts_to_neutral_text_and_images",
        "neutral receipt projection fixture",
    )?;
    forbid_contains(&domain_event, "DaemonEvent", "domain event protocol DTO leakage")?;
    forbid_contains(&domain_event, "TuiEvent", "domain event TUI DTO leakage")?;
    forbid_contains(&domain_event, "clankers_protocol", "domain event protocol crate dependency")?;
    forbid_contains(&domain_event, "clanker_tui_types", "domain event TUI crate dependency")?;
    require_contains(
        &convert,
        "agent_event_to_domain_event(event).map(domain_event_to_daemon_event)",
        "protocol projection delegates through neutral domain event seam",
    )?;

    Ok(json!({
        "domain_event_module": CONTROLLER_DOMAIN_EVENT,
        "protocol_projection_module": CONTROLLER_CONVERT,
        "neutral_event_enum": "ControllerDomainEvent",
        "neutral_receipt_dto": "DomainImage",
        "protocol_references_in_domain_module": 0,
        "tui_references_in_domain_module": 0,
        "protocol_projection_owner": "convert::domain_event_to_daemon_event",
        "typed_rail_kind": "Rust AST enum, struct, function, path, and forbidden edge-dependency checks"
    }))
}

fn session_command_policy_signature() -> Result<Value, String> {
    let policy_file = read_rust_file(SESSION_COMMAND_POLICY)?;
    let attach_file = read_rust_file(ATTACH_COMMANDS)?;
    let agent_task_file = read_rust_file(AGENT_TASK)?;
    let policy = &policy_file.source;
    let attach = &attach_file.source;
    let agent_task = &agent_task_file.source;

    require_rust_enum(&policy_file, "LocalSessionEffect", "typed local session effect DTO")?;
    require_rust_enum(&policy_file, "SessionAckPolicy", "typed session ack policy DTO")?;
    require_rust_struct(&policy_file, "SessionCommandEffect", "typed session command effect DTO")?;
    for function in ["set_thinking_level_effect", "cycle_thinking_level_effect", "disabled_tools_effect", "manual_compaction_effect", "ack_matches"] {
        require_rust_fn(&policy_file, function, "shared session command policy function")?;
    }
    require_rust_path(&attach_file, "session_command_policy::cycle_thinking_level_effect", "attach cycle thinking delegates to shared policy")?;
    require_rust_path(&attach_file, "session_command_policy::ack_matches", "attach ack suppression delegates to shared policy")?;
    require_rust_path(&agent_task_file, "session_command_policy::thinking_level_message", "standalone thinking message delegates to shared policy")?;

    require_contains(
        &policy,
        "Shared session command/effect/ack policy",
        "session command policy module purpose",
    )?;
    require_contains(&policy, "enum LocalSessionEffect", "typed local session effect DTO")?;
    require_contains(&policy, "enum SessionAckPolicy", "typed session ack policy DTO")?;
    require_contains(&policy, "struct SessionCommandEffect", "typed session command effect DTO")?;
    require_contains(&policy, "set_thinking_level_effect", "shared thinking set effect")?;
    require_contains(&policy, "cycle_thinking_level_effect", "shared thinking cycle effect")?;
    require_contains(&policy, "disabled_tools_effect", "shared disabled-tools effect")?;
    require_contains(&policy, "manual_compaction_effect", "shared compaction effect")?;
    require_contains(&policy, "ack_matches", "shared ack matcher")?;
    require_contains(
        &policy,
        "thinking_effect_projects_local_message_command_and_ack_policy",
        "positive shared policy fixture",
    )?;
    require_contains(&policy, "ack_policy_matches_only_expected_daemon_ack_shape", "negative ack fixture")?;
    require_contains(
        &attach,
        "dispatch_session_command_effect",
        "attach command shell delegates through shared effect dispatcher",
    )?;
    require_contains(
        &attach,
        "session_command_policy::cycle_thinking_level_effect(app.thinking_level)",
        "attach cycle thinking delegates to shared policy",
    )?;
    require_contains(
        &attach,
        "session_command_policy::ack_matches(SessionAckPolicy::ThinkingLevel, event)",
        "attach ack suppression delegates to shared policy",
    )?;
    require_contains(
        &agent_task,
        "session_command_policy::thinking_level_message(level)",
        "standalone thinking message delegates to shared policy",
    )?;

    Ok(json!({
        "policy_module": SESSION_COMMAND_POLICY,
        "attach_adapter_module": ATTACH_COMMANDS,
        "standalone_agent_task_module": AGENT_TASK,
        "local_effect_dto": "LocalSessionEffect",
        "ack_policy_dto": "SessionAckPolicy",
        "effect_dto": "SessionCommandEffect",
        "shared_effects": [
            "set_thinking_level_effect",
            "cycle_thinking_level_effect",
            "disabled_tools_effect",
            "manual_compaction_effect"
        ],
        "ack_matcher": "ack_matches",
        "positive_fixture": "thinking_effect_projects_local_message_command_and_ack_policy",
        "negative_fixture": "ack_policy_matches_only_expected_daemon_ack_shape",
        "typed_rail_kind": "Rust AST enum, struct, function, and call-path checks"
    }))
}


struct RustFile {
    source: String,
    ast: syn::File,
}

fn read_rust_file(path: &str) -> Result<RustFile, String> {
    let source = fs::read_to_string(path).map_err(|error| format!("failed to read {path}: {error}"))?;
    let ast = syn::parse_file(&source).map_err(|error| format!("failed to parse Rust AST for {path}: {error}"))?;
    Ok(RustFile { source, ast })
}

fn require_rust_mod(file: &RustFile, name: &str, label: &str) -> Result<(), String> {
    if file.ast.items.iter().any(|item| matches!(item, syn::Item::Mod(item) if item.ident == name)) {
        return Ok(());
    }
    Err(format!("missing {label}: Rust module `{name}`"))
}

fn require_rust_fn(file: &RustFile, name: &str, label: &str) -> Result<(), String> {
    if file.ast.items.iter().any(|item| matches!(item, syn::Item::Fn(item) if item.sig.ident == name)) {
        return Ok(());
    }
    Err(format!("missing {label}: Rust function `{name}`"))
}

fn forbid_rust_fn(file: &RustFile, name: &str, label: &str) -> Result<(), String> {
    if file.ast.items.iter().any(|item| matches!(item, syn::Item::Fn(item) if item.sig.ident == name)) {
        return Err(format!("forbidden {label}: Rust function `{name}`"));
    }
    Ok(())
}

fn require_rust_method(file: &RustFile, name: &str, label: &str) -> Result<(), String> {
    for item in &file.ast.items {
        let syn::Item::Impl(item_impl) = item else { continue };
        if item_impl.items.iter().any(|item| matches!(item, syn::ImplItem::Fn(function) if function.sig.ident == name)) {
            return Ok(());
        }
    }
    Err(format!("missing {label}: Rust method `{name}`"))
}

fn require_rust_struct(file: &RustFile, name: &str, label: &str) -> Result<(), String> {
    if file.ast.items.iter().any(|item| matches!(item, syn::Item::Struct(item) if item.ident == name)) {
        return Ok(());
    }
    Err(format!("missing {label}: Rust struct `{name}`"))
}

fn require_rust_enum(file: &RustFile, name: &str, label: &str) -> Result<(), String> {
    if file.ast.items.iter().any(|item| matches!(item, syn::Item::Enum(item) if item.ident == name)) {
        return Ok(());
    }
    Err(format!("missing {label}: Rust enum `{name}`"))
}

fn require_rust_trait(file: &RustFile, name: &str, label: &str) -> Result<(), String> {
    if file.ast.items.iter().any(|item| matches!(item, syn::Item::Trait(item) if item.ident == name)) {
        return Ok(());
    }
    Err(format!("missing {label}: Rust trait `{name}`"))
}

fn require_rust_impl(file: &RustFile, trait_name: &str, type_name: &str, label: &str) -> Result<(), String> {
    for item in &file.ast.items {
        let syn::Item::Impl(item_impl) = item else { continue };
        let Some((_, trait_path, _)) = &item_impl.trait_ else { continue };
        if path_ends_with(trait_path, trait_name) && type_mentions_path(&item_impl.self_ty, type_name) {
            return Ok(());
        }
    }
    Err(format!("missing {label}: Rust impl `{trait_name}` for `{type_name}`"))
}

fn require_struct_field_type_path(
    file: &RustFile,
    struct_name: &str,
    field_name: &str,
    type_path: &str,
    label: &str,
) -> Result<(), String> {
    match struct_field_type_mentions(file, struct_name, field_name, type_path) {
        Some(true) => Ok(()),
        Some(false) => Err(format!("missing {label}: field `{struct_name}.{field_name}` does not reference `{type_path}`")),
        None => Err(format!("missing {label}: field `{struct_name}.{field_name}`")),
    }
}

fn forbid_struct_field_type_path(
    file: &RustFile,
    struct_name: &str,
    field_name: &str,
    type_path: &str,
    label: &str,
) -> Result<(), String> {
    if struct_field_type_mentions(file, struct_name, field_name, type_path) == Some(true) {
        return Err(format!("forbidden {label}: field `{struct_name}.{field_name}` references `{type_path}`"));
    }
    Ok(())
}

fn struct_field_type_mentions(file: &RustFile, struct_name: &str, field_name: &str, type_path: &str) -> Option<bool> {
    for item in &file.ast.items {
        let syn::Item::Struct(item_struct) = item else { continue };
        if item_struct.ident != struct_name {
            continue;
        }
        let syn::Fields::Named(fields) = &item_struct.fields else { return None };
        for field in &fields.named {
            if field.ident.as_ref().is_some_and(|ident| ident == field_name) {
                return Some(type_mentions_path(&field.ty, type_path));
            }
        }
    }
    None
}

fn require_rust_path(file: &RustFile, path: &str, label: &str) -> Result<(), String> {
    if rust_paths(file).iter().any(|actual| path_matches(actual, path)) {
        return Ok(());
    }
    Err(format!("missing {label}: Rust path `{path}`"))
}

fn forbid_rust_path(file: &RustFile, path: &str, label: &str) -> Result<(), String> {
    if rust_paths(file).iter().any(|actual| path_matches(actual, path)) {
        return Err(format!("forbidden {label}: Rust path `{path}`"));
    }
    Ok(())
}

fn rust_paths(file: &RustFile) -> BTreeSet<String> {
    let mut collector = PathCollector::default();
    collector.visit_file(&file.ast);
    collector.paths
}

#[derive(Default)]
struct PathCollector {
    paths: BTreeSet<String>,
}

impl<'ast> Visit<'ast> for PathCollector {
    fn visit_path(&mut self, path: &'ast syn::Path) {
        self.paths.insert(path_to_string(path));
        syn::visit::visit_path(self, path);
    }
}

fn type_mentions_path(ty: &syn::Type, expected: &str) -> bool {
    let mut collector = PathCollector::default();
    collector.visit_type(ty);
    collector.paths.iter().any(|actual| path_matches(actual, expected))
}

fn path_ends_with(path: &syn::Path, expected: &str) -> bool {
    path_matches(&path_to_string(path), expected)
}

fn path_to_string(path: &syn::Path) -> String {
    path.segments
        .iter()
        .map(|segment| segment.ident.to_string())
        .collect::<Vec<_>>()
        .join("::")
}

fn path_matches(actual: &str, expected: &str) -> bool {
    actual == expected || actual.ends_with(&format!("::{expected}"))
}

fn validate_baseline(signature: &Value) -> Result<(), String> {
    let text = fs::read_to_string(BASELINE).map_err(|error| format!("failed to read {BASELINE}: {error}; generate and commit the dependency ownership baseline before running the rail"))?;
    let baseline: Value =
        serde_json::from_str(&text).map_err(|error| format!("failed to parse {BASELINE}: {error}"))?;
    require_eq(&baseline, "schema", "clankers.lego_architecture.dependency_ownership_baseline.v1")?;
    let expected = baseline
        .get("inventory_signature")
        .ok_or_else(|| format!("{BASELINE} missing inventory_signature"))?;
    if expected != signature {
        return Err(format!(
            "dependency ownership inventory drifted from {BASELINE}; update the baseline intentionally or reduce the new coupling"
        ));
    }
    Ok(())
}

fn write_receipt(receipt: &Value) -> Result<PathBuf, String> {
    let output = PathBuf::from(DEFAULT_OUTPUT);
    let parent = output.parent().ok_or_else(|| format!("{} has no parent", output.display()))?;
    fs::create_dir_all(parent).map_err(|error| format!("failed to create {}: {error}", parent.display()))?;
    let bytes = serde_json::to_vec_pretty(receipt).map_err(|error| format!("failed to encode receipt: {error}"))?;
    fs::write(&output, [bytes.as_slice(), b"\n"].concat())
        .map_err(|error| format!("failed to write {}: {error}", output.display()))?;
    Ok(output)
}

fn require_nonempty<T>(items: &[T], label: &str) -> Result<(), String> {
    if items.is_empty() {
        return Err(format!("{label} must not be empty"));
    }
    Ok(())
}

fn require_contains(haystack: &str, needle: &str, label: &str) -> Result<(), String> {
    if !haystack.contains(needle) {
        return Err(format!("missing {label}: {needle}"));
    }
    Ok(())
}

fn forbid_contains(haystack: &str, needle: &str, label: &str) -> Result<(), String> {
    if haystack.contains(needle) {
        return Err(format!("forbidden {label}: {needle}"));
    }
    Ok(())
}

fn require_eq(value: &Value, key: &str, expected: &str) -> Result<(), String> {
    let actual = required_str(value, key)?;
    if actual != expected {
        return Err(format!("expected `{key}` to be `{expected}`, got `{actual}`"));
    }
    Ok(())
}

fn required_str<'a>(value: &'a Value, key: &str) -> Result<&'a str, String> {
    value.get(key).and_then(Value::as_str).ok_or_else(|| format!("missing string field `{key}`"))
}

fn hash_artifact(path: &Path) -> Result<Value, String> {
    let bytes = fs::read(path).map_err(|error| format!("failed to read {}: {error}", path.display()))?;
    Ok(json!({
        "path": path.display().to_string(),
        "blake3": blake3::hash(&bytes).to_hex().to_string()
    }))
}
