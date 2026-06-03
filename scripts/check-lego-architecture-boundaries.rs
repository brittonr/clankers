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
const PROCESS_TOOL_DURABLE: &str = "src/tools/process/durable.rs";
const PROCESS_TOOL_NATIVE: &str = "src/tools/process/native.rs";
const PROCESS_TOOL_PUEUE: &str = "src/tools/process/pueue.rs";
const PROCESS_TOOL_SYSTEMD: &str = "src/tools/process/systemd.rs";
const AGENT_LIB: &str = "crates/clankers-agent/src/lib.rs";
const AGENT_BUILDER: &str = "crates/clankers-agent/src/builder.rs";
const AGENT_COMPACTION: &str = "crates/clankers-agent/src/compaction.rs";
const AGENT_COMPACTION_TOOL_SUMMARIES: &str = "crates/clankers-agent/src/compaction/tool_summaries.rs";
const AGENT_EVENTS: &str = "crates/clankers-agent/src/events.rs";
const AGENT_TOOL: &str = "crates/clankers-agent/src/tool.rs";
const AGENT_TURN_MOD: &str = "crates/clankers-agent/src/turn/mod.rs";
const AGENT_TURN_ADAPTERS: &str = "crates/clankers-agent/src/turn/adapters.rs";
const AGENT_TURN_EXECUTION: &str = "crates/clankers-agent/src/turn/execution.rs";
const AGENT_TURN_MESSAGE: &str = "crates/clankers-agent/src/turn/message.rs";
const AGENT_TURN_POLICY: &str = "crates/clankers-agent/src/turn/policy.rs";
const AGENT_TURN_PORTS: &str = "crates/clankers-agent/src/turn/ports.rs";
const AGENT_TURN_STEEL_PLANNING: &str = "crates/clankers-agent/src/turn/steel_planning.rs";
const AGENT_TURN_STEEL_TOOL_SUBSTRATE: &str = "crates/clankers-agent/src/turn/steel_tool_substrate.rs";
const AGENT_TURN_TRANSCRIPT: &str = "crates/clankers-agent/src/turn/transcript.rs";
const AGENT_TURN_USAGE: &str = "crates/clankers-agent/src/turn/usage.rs";
const TOOL_HOST_LIB: &str = "crates/clankers-tool-host/src/lib.rs";
const CONTROLLER_COMMAND: &str = "crates/clankers-controller/src/command.rs";
const CONTROLLER_COMMAND_RESPONSIBILITY: &str = "crates/clankers-controller/src/command_responsibility.rs";
const CONTROLLER_COMMAND_THINKING: &str = "crates/clankers-controller/src/command_thinking.rs";
const CONTROLLER_AUTO_TEST: &str = "crates/clankers-controller/src/auto_test.rs";
const CONTROLLER_CORE_EFFECTS: &str = "crates/clankers-controller/src/core_effects.rs";
const CONTROLLER_CONVERT: &str = "crates/clankers-controller/src/convert.rs";
const CONTROLLER_DOMAIN_EVENT: &str = "crates/clankers-controller/src/domain_event.rs";
const CONTROLLER_EFFECT_INTERPRETATION: &str = "crates/clankers-controller/src/effect_interpretation.rs";
const PROVIDER_ROUTER_BRIDGE: &str = "crates/clankers-provider/src/router_request_bridge.rs";
const PROVIDER_ROUTER_RESPONSIBILITY: &str = "crates/clankers-provider/src/provider_router_responsibility.rs";
const PROVIDER_ROUTER_ADAPTER: &str = "crates/clankers-provider/src/router.rs";
const PROVIDER_RPC_ADAPTER: &str = "crates/clankers-provider/src/rpc_provider.rs";
const SESSION_COMMAND_POLICY: &str = "src/modes/session_command_policy.rs";
const ATTACH_COMMANDS: &str = "src/modes/attach/commands.rs";
const SLASH_EFFECTS: &str = "src/slash_commands/effects.rs";
const AGENT_TASK: &str = "src/modes/agent_task.rs";
const DAEMON_AGENT_PROCESS: &str = "src/modes/daemon/agent_process.rs";
const DAEMON_SESSION_BUILDER: &str = "src/modes/daemon/session_builder.rs";
const DAEMON_SESSION_PLUGINS: &str = "src/modes/daemon/session_plugins.rs";

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
    let root_owner_receipts = dependency_owner_receipts(ROOT_PACKAGE, &root_internal)?;
    let controller_owner_receipts = dependency_owner_receipts(CONTROLLER_PACKAGE, &controller_internal)?;
    let shared_dtos = shared_dto_crates(&reverse_map);
    let process_tool_adapter = process_tool_adapter_signature()?;
    let agent_turn_ports = agent_turn_ports_signature()?;
    let agent_provider_neutral_dtos = agent_provider_neutral_dto_signature()?;
    let tool_host_service_context = tool_host_service_context_signature()?;
    let controller_effect_interpretation = controller_effect_interpretation_signature()?;
    let provider_router_bridge = provider_router_bridge_signature()?;
    let controller_domain_event = controller_domain_event_signature()?;
    let controller_display_protocol_dtos = controller_display_protocol_dto_signature()?;
    let controller_command_responsibility = controller_command_responsibility_signature()?;
    let session_command_policy = session_command_policy_signature()?;
    let daemon_session_assembly = daemon_session_assembly_signature()?;

    require_nonempty(&root_internal, "root internal dependency inventory")?;
    require_nonempty(&agent_concrete, "agent concrete dependency inventory")?;
    require_nonempty(&controller_concrete, "controller concrete dependency inventory")?;
    require_nonempty(&shared_dtos, "shared DTO crate inventory")?;

    let signature = json!({
        "root_crate": {
            "package": ROOT_PACKAGE,
            "internal_dependency_count": root_internal.len(),
            "internal_dependencies": root_internal,
            "owner_receipts": root_owner_receipts,
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
            "owner_receipts": controller_owner_receipts,
            "owner": "session orchestration shell; translation, effect, continuation, persistence, and projection seams stay separately testable"
        },
        "most_shared_dto_crates": shared_dtos,
        "process_tool_adapter": process_tool_adapter,
        "agent_turn_ports": agent_turn_ports,
        "agent_provider_neutral_dtos": agent_provider_neutral_dtos,
        "tool_host_service_context": tool_host_service_context,
        "controller_effect_interpretation": controller_effect_interpretation,
        "provider_router_bridge": provider_router_bridge,
        "controller_domain_event": controller_domain_event,
        "controller_display_protocol_dtos": controller_display_protocol_dtos,
        "controller_command_responsibility": controller_command_responsibility,
        "session_command_policy": session_command_policy,
        "daemon_session_assembly": daemon_session_assembly,
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
            hash_artifact(Path::new(PROCESS_TOOL_DURABLE))?,
            hash_artifact(Path::new(PROCESS_TOOL_NATIVE))?,
            hash_artifact(Path::new(PROCESS_TOOL_PUEUE))?,
            hash_artifact(Path::new(PROCESS_TOOL_SYSTEMD))?,
            hash_artifact(Path::new(AGENT_LIB))?,
            hash_artifact(Path::new(AGENT_BUILDER))?,
            hash_artifact(Path::new(AGENT_COMPACTION))?,
            hash_artifact(Path::new(AGENT_COMPACTION_TOOL_SUMMARIES))?,
            hash_artifact(Path::new(AGENT_EVENTS))?,
            hash_artifact(Path::new(AGENT_TOOL))?,
            hash_artifact(Path::new(AGENT_TURN_MOD))?,
            hash_artifact(Path::new(AGENT_TURN_ADAPTERS))?,
            hash_artifact(Path::new(AGENT_TURN_EXECUTION))?,
            hash_artifact(Path::new(AGENT_TURN_MESSAGE))?,
            hash_artifact(Path::new(AGENT_TURN_POLICY))?,
            hash_artifact(Path::new(AGENT_TURN_PORTS))?,
            hash_artifact(Path::new(AGENT_TURN_STEEL_PLANNING))?,
            hash_artifact(Path::new(AGENT_TURN_STEEL_TOOL_SUBSTRATE))?,
            hash_artifact(Path::new(AGENT_TURN_TRANSCRIPT))?,
            hash_artifact(Path::new(AGENT_TURN_USAGE))?,
            hash_artifact(Path::new(TOOL_HOST_LIB))?,
            hash_artifact(Path::new(CONTROLLER_CORE_EFFECTS))?,
            hash_artifact(Path::new(CONTROLLER_CONVERT))?,
            hash_artifact(Path::new(CONTROLLER_DOMAIN_EVENT))?,
            hash_artifact(Path::new(CONTROLLER_EFFECT_INTERPRETATION))?,
            hash_artifact(Path::new(PROVIDER_ROUTER_BRIDGE))?,
            hash_artifact(Path::new(PROVIDER_ROUTER_RESPONSIBILITY))?,
            hash_artifact(Path::new(PROVIDER_ROUTER_ADAPTER))?,
            hash_artifact(Path::new(PROVIDER_RPC_ADAPTER))?,
            hash_artifact(Path::new(CONTROLLER_COMMAND))?,
            hash_artifact(Path::new(CONTROLLER_COMMAND_RESPONSIBILITY))?,
            hash_artifact(Path::new(CONTROLLER_COMMAND_THINKING))?,
            hash_artifact(Path::new(CONTROLLER_AUTO_TEST))?,
            hash_artifact(Path::new(SESSION_COMMAND_POLICY))?,
            hash_artifact(Path::new(ATTACH_COMMANDS))?,
            hash_artifact(Path::new(SLASH_EFFECTS))?,
            hash_artifact(Path::new(AGENT_TASK))?,
            hash_artifact(Path::new(DAEMON_AGENT_PROCESS))?,
            hash_artifact(Path::new(DAEMON_SESSION_BUILDER))?,
            hash_artifact(Path::new(DAEMON_SESSION_PLUGINS))?
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

fn dependency_owner_receipts(source_crate: &str, deps: &[String]) -> Result<Vec<Value>, String> {
    deps.iter().map(|target| dependency_owner_receipt(source_crate, target)).collect()
}

fn dependency_owner_receipt(source_crate: &str, target_crate: &str) -> Result<Value, String> {
    let (owner_category, adapter_module, convergence_condition) = match source_crate {
        ROOT_PACKAGE => root_dependency_owner(target_crate)?,
        CONTROLLER_PACKAGE => controller_dependency_owner(target_crate)?,
        other => return Err(format!("no dependency owner table for `{other}`")),
    };
    Ok(json!({
        "source_crate": source_crate,
        "target_crate": target_crate,
        "owner_category": owner_category,
        "adapter_module": adapter_module,
        "convergence_condition": convergence_condition,
    }))
}

fn root_dependency_owner(target_crate: &str) -> Result<(&'static str, &'static str, &'static str), String> {
    match target_crate {
        "clanker-message" => Ok(("shared message DTO", "src/runtime_services.rs", "remain shared DTO only; no root-owned message policy")),
        "clanker-router" => Ok(("desktop provider routing", "src/runtime_services.rs", "route through ProviderRouterService and provider bridge")),
        "clanker-tui-types" => Ok(("display-edge DTO", "src/modes/event_loop_runner", "keep rendering rules in TUI/display adapters")),
        "clankers-agent" => Ok(("desktop agent construction", "src/agent.rs", "agent execution policy migrates behind runtime/controller adapters")),
        "clankers-agent-defs" => Ok(("CLI agent profile loading", "src/cli.rs", "profile semantics stay in agent-defs crate")),
        "clankers-artifacts" => Ok(("artifact command shell", "src/commands", "artifact policy remains in artifacts crate")),
        "clankers-autoresearch" => Ok(("mode dispatch", "src/modes", "research policy remains in autoresearch crate")),
        "clankers-config" => Ok(("desktop config loading", "src/config", "neutral config stays in clankers-config core DTOs")),
        "clankers-controller" => Ok(("session orchestration adapter", "src/modes/event_loop_runner", "controller owns command lifecycle only")),
        "clankers-core" => Ok(("control-state bridge", "src/modes/session_command_policy.rs", "core reducer policy remains in clankers-core")),
        "clankers-db" => Ok(("desktop storage wiring", "src/runtime_services.rs", "storage access moves behind runtime/session stores")),
        "clankers-hooks" => Ok(("desktop hook wiring", "src/modes", "hook policy remains in hooks crate")),
        "clankers-matrix" => Ok(("optional Matrix mode", "src/commands/daemon.rs", "Matrix protocol remains in matrix crate")),
        "clankers-model-selection" => Ok(("desktop model selection", "src/runtime_services.rs", "model routing state moves behind provider/runtime service")),
        "clankers-nix" => Ok(("Nix tool/mode shell", "src/tools", "Nix semantics remain in clankers-nix crate")),
        "clankers-plugin" => Ok(("plugin host wiring", "src/plugin", "plugin runtime policy remains in plugin host facade")),
        "clankers-procmon" => Ok(("process-monitor tool shell", "src/tools/process", "process policy remains in procmon/tool adapter")),
        "clankers-prompts" => Ok(("desktop prompt source wiring", "src/runtime_prompt.rs", "prompt assembly stays in prompt/runtime services")),
        "clankers-protocol" => Ok(("daemon protocol edge", "src/modes/daemon", "protocol frames stay at transport edge")),
        "clankers-provider" => Ok(("desktop provider construction", "src/runtime_services.rs", "provider shaping stays in provider/router bridge")),
        "clankers-runtime" => Ok(("runtime facade composition", "src/runtime_services.rs", "runtime services remain explicit embeddable bricks")),
        "clankers-session" => Ok(("desktop session storage", "src/modes/session_setup.rs", "storage policy moves behind SessionStore/ledger DTOs")),
        "clankers-skills" => Ok(("desktop skill discovery", "src/runtime_services.rs", "skill discovery stays behind explicit SkillStore roots")),
        "clankers-tool-host" => Ok(("tool host DTO bridge", "src/tools", "tool execution policy stays in tool-host/adapters")),
        "clankers-tts" => Ok(("optional voice mode", "src/commands", "TTS policy stays in tts crate")),
        "clankers-tui" => Ok(("display shell", "src/modes/event_loop_runner", "rendering remains in TUI crate")),
        "clankers-ucan" => Ok(("capability auth shell", "src/capability", "authorization policy stays in UCAN/capability gate")),
        "clankers-util" => Ok(("desktop utility wiring", "src", "utilities must stay leaf helpers, not reusable policy owner")),
        "clankers-zellij" => Ok(("optional zellij mode", "src/commands", "zellij integration remains edge adapter")),
        other => Err(format!("unowned root dependency `{other}`")),
    }
}

fn controller_dependency_owner(target_crate: &str) -> Result<(&'static str, &'static str, &'static str), String> {
    match target_crate {
        "clanker-loop" => Ok(("loop state service", "crates/clankers-controller/src/loop_mode.rs", "loop policy remains in clanker-loop/core effects")),
        "clanker-message" => Ok(("shared semantic/message DTO", "crates/clankers-controller/src/convert.rs", "controller projects from SemanticEvent/message DTOs only")),
        "clanker-tui-types" => Ok(("display sync compatibility", "crates/clankers-controller/src/auto_test.rs", "display state sync remains edge-only until moved behind neutral DTO")),
        "clankers-agent" => Ok(("daemon agent runtime adapter", "crates/clankers-controller/src/runtime_adapter.rs", "prompt/control execution moves behind ControllerRuntimeAdapter")),
        "clankers-config" => Ok(("controller config DTO", "crates/clankers-controller/src/config.rs", "configuration remains construction input only")),
        "clankers-core" => Ok(("pure control reducer", "crates/clankers-controller/src/command.rs", "core policy remains in clankers-core and interpretation seams")),
        "clankers-db" => Ok(("optional search index shell", "crates/clankers-controller/src/persistence.rs", "storage/search moves behind session service interface")),
        "clankers-engine" => Ok(("engine composition fixture", "crates/clankers-controller/src/core_engine_composition.rs", "engine data remains adapter source only")),
        "clankers-hooks" => Ok(("lifecycle hook adapter", "crates/clankers-controller/src/event_processing.rs", "hook policy remains in hooks crate")),
        "clankers-protocol" => Ok(("transport protocol edge", "crates/clankers-controller/src/transport_convert.rs", "protocol construction stays in conversion modules")),
        "clankers-provider" => Ok(("provider thinking compatibility", "crates/clankers-controller/src/command.rs", "provider types disappear when agent runtime adapter owns execution")),
        "clankers-session" => Ok(("session persistence shell", "crates/clankers-controller/src/persistence.rs", "persistence migrates behind SessionStore/ledger service")),
        other => Err(format!("unowned controller dependency `{other}`")),
    }
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
    let durable_file = read_rust_file(PROCESS_TOOL_DURABLE)?;
    let native_file = read_rust_file(PROCESS_TOOL_NATIVE)?;
    let pueue_file = read_rust_file(PROCESS_TOOL_PUEUE)?;
    let systemd_file = read_rust_file(PROCESS_TOOL_SYSTEMD)?;
    let tool = &tool_file.source;
    let adapter = &adapter_file.source;
    let durable = &durable_file.source;
    let native = &native_file.source;
    let pueue = &pueue_file.source;
    let systemd = &systemd_file.source;
    require_rust_mod(&tool_file, "adapter", "process tool adapter module")?;
    require_rust_mod(&tool_file, "durable", "process durable policy module")?;
    require_rust_mod(&tool_file, "native", "process native backend adapter module")?;
    require_rust_mod(&tool_file, "pueue", "process pueue backend adapter module")?;
    require_rust_mod(&tool_file, "systemd", "process systemd backend adapter module")?;
    require_contains(&tool, "mod adapter;", "process tool adapter module")?;
    require_contains(&tool, "mod durable;", "process durable policy module")?;
    require_contains(&tool, "mod native;", "process native backend adapter module")?;
    require_contains(&tool, "mod pueue;", "process pueue backend adapter module")?;
    require_contains(&tool, "mod systemd;", "process systemd backend adapter module")?;
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
    require_rust_path(&native_file, "NativeProcessJobBackendAdapter", "process native backend adapter type")?;
    require_rust_path(&native_file, "NativeProcessJobService", "process native service owner")?;
    require_rust_path(&native_file, "ProcessEntry", "process native entry owner")?;
    require_rust_path(&native_file, "ProcessRegistry", "process native registry owner")?;
    require_contains(native, "pub(super) struct NativeProcessJobService", "process native service owner")?;
    require_contains(native, "pub(super) struct ProcessEntry", "process native entry owner")?;
    require_contains(native, "pub(super) struct ProcessRegistry", "process native registry owner")?;
    require_contains(native, "pub(super) fn reserve_native_start", "process native admission owner")?;
    require_contains(native, "pub(super) async fn restart_native_process_job", "process native restart owner")?;
    require_contains(native, "fn terminate_process_group", "process native termination owner")?;
    require_contains(tool, "NativeProcessJobBackendAdapter::for_invocation", "process native adapter dispatch")?;
    forbid_contains(tool, "struct NativeProcessJobService", "process root native service owner")?;
    forbid_contains(tool, "struct ProcessEntry", "process root native entry owner")?;
    forbid_contains(tool, "enum ProcessStatus", "process root native status owner")?;
    forbid_contains(tool, "struct ProcessRegistry", "process root native registry owner")?;
    forbid_contains(tool, "static REGISTRY", "process root native registry owner")?;
    forbid_contains(tool, "fn terminate_process_group", "process root native termination owner")?;
    require_rust_path(&pueue_file, "PueueProcessJobService", "process pueue backend service type")?;
    require_rust_path(&pueue_file, "PueueRunner", "process pueue fakeable runner trait")?;
    require_contains(pueue, "pub(super) trait PueueRunner", "process pueue fakeable runner trait")?;
    require_contains(pueue, "pub(super) struct PueueProcessJobService", "process pueue service owner")?;
    require_contains(pueue, "fn parse_pueue_tasks", "process pueue task parser owner")?;
    require_contains(pueue, "fn parse_pueue_log_text", "process pueue log parser owner")?;
    forbid_contains(tool, "struct PueueTaskProjection", "process root pueue projection owner")?;
    forbid_contains(tool, "fn parse_pueue_", "process root pueue parser owner")?;
    require_rust_path(&systemd_file, "SystemdProcessJobService", "process systemd backend service type")?;
    require_rust_path(&systemd_file, "SystemdRunner", "process systemd fakeable runner trait")?;
    require_contains(systemd, "pub(super) trait SystemdRunner", "process systemd fakeable runner trait")?;
    require_contains(systemd, "pub(super) struct SystemdProcessJobService", "process systemd service owner")?;
    require_contains(systemd, "fn parse_systemd_show", "process systemd show parser owner")?;
    require_contains(systemd, "fn parse_systemd_list_units", "process systemd list parser owner")?;
    forbid_contains(tool, "struct SystemdUnitProjection", "process root systemd projection owner")?;
    forbid_contains(tool, "fn parse_systemd_", "process root systemd parser owner")?;
    require_contains(durable, "pub(super) fn stored_record_from_entry", "process durable record owner")?;
    require_contains(durable, "pub(super) async fn apply_process_job_retention", "process durable retention owner")?;
    require_contains(durable, "pub(super) async fn evaluate_process_entry_notification", "process notification policy owner")?;
    forbid_contains(tool, "fn stored_record_from_entry", "process root durable record owner")?;
    forbid_contains(tool, "fn apply_process_job_retention", "process root retention owner")?;
    forbid_contains(tool, "DefaultProcessJobNotificationPolicyEngine", "process root notification policy owner")?;
    let backend_ownership = process_backend_ownership_signature(&tool_file, tool)?;
    Ok(json!({
        "adapter_module": PROCESS_TOOL_ADAPTER,
        "durable_module": PROCESS_TOOL_DURABLE,
        "native_module": PROCESS_TOOL_NATIVE,
        "pueue_module": PROCESS_TOOL_PUEUE,
        "systemd_module": PROCESS_TOOL_SYSTEMD,
        "tool_module": PROCESS_TOOL,
        "storage_dto_imports": 0,
        "storage_dto_references": 0,
        "request_parser_owner": "ProcessToolJsonAdapter",
        "fail_closed_negative_path": "unsupported action returns ToolResult error before backend dispatch",
        "backend_ownership": backend_ownership,
        "backend_owner_count": 7,
        "durable_record_owner": "stored_record_from_entry in src/tools/process/durable.rs",
        "durable_retention_owner": "apply_process_job_retention in src/tools/process/durable.rs",
        "native_backend_adapter": "NativeProcessJobBackendAdapter",
        "native_registry_owner": "ProcessRegistry in src/tools/process/native.rs",
        "pueue_backend_service": "PueueProcessJobService",
        "pueue_runner_owner": "PueueRunner in src/tools/process/pueue.rs",
        "systemd_backend_service": "SystemdProcessJobService",
        "systemd_runner_owner": "SystemdRunner in src/tools/process/systemd.rs",
        "notification_policy_owner": "evaluate_process_entry_notification in src/tools/process/durable.rs",
        "typed_rail_kind": "Rust AST module, method, path, forbidden dependency, process native adapter/registry, pueue runner/parser, systemd runner/parser, durable retention/notification, and backend ownership checks"
    }))
}

fn process_backend_ownership_signature(tool_file: &RustFile, tool: &str) -> Result<Value, String> {
    require_contains(tool, "PROCESS_JOB_BACKEND_ADAPTER_OWNERSHIP", "process backend ownership map")?;
    let expected = [
        (
            "RootProjection",
            "src/tools/process.rs::ProcessTool",
            "parse request, select backend service, project typed receipts",
        ),
        (
            "NativeBackend",
            "src/tools/process/native.rs::NativeProcessJobBackendAdapter",
            "select native backend and project typed ProcessJobReceipt",
        ),
        (
            "PueueBackend",
            "src/tools/process/pueue.rs::PueueProcessJobService",
            "select pueue backend and surface degraded unavailable receipts",
        ),
        (
            "SystemdBackend",
            "src/tools/process/systemd.rs::SystemdProcessJobService",
            "select systemd backend and surface degraded unsupported receipts",
        ),
        (
            "DurableStorage",
            "src/tools/process/durable.rs::ProcessJobDurableRecordPolicy",
            "wire optional durable storage service",
        ),
        (
            "RetentionGarbageCollection",
            "src/tools/process/durable.rs::ProcessJobRetentionPolicyService",
            "invoke retention service and project typed GC receipt",
        ),
        (
            "NotificationDelivery",
            "src/tools/process/durable.rs::ProcessJobNotificationPolicyService",
            "wire notification sink and project redacted observations",
        ),
    ];
    let mut owners = Vec::new();
    for (cluster, target_owner, root_accountability) in expected {
        require_rust_path(
            tool_file,
            &format!("ProcessJobPolicyCluster::{cluster}"),
            &format!("process backend ownership cluster {cluster}"),
        )?;
        require_contains(tool, target_owner, &format!("process backend target owner {cluster}"))?;
        require_contains(tool, root_accountability, &format!("process backend root accountability {cluster}"))?;
        owners.push(json!({
            "cluster": cluster,
            "target_owner": target_owner,
            "root_accountability": root_accountability,
        }));
    }
    Ok(Value::Array(owners))
}

fn agent_turn_ports_signature() -> Result<Value, String> {
    let agent_lib_file = read_rust_file(AGENT_LIB)?;
    let turn_mod_file = read_rust_file(AGENT_TURN_MOD)?;
    let adapters_file = read_rust_file(AGENT_TURN_ADAPTERS)?;
    let ports_file = read_rust_file(AGENT_TURN_PORTS)?;
    let steel_tool_substrate_file = read_rust_file(AGENT_TURN_STEEL_TOOL_SUBSTRATE)?;
    let tool_file = read_rust_file(AGENT_TOOL)?;
    let agent_lib = &agent_lib_file.source;
    let turn_mod = &turn_mod_file.source;
    let adapters = &adapters_file.source;
    let ports = &ports_file.source;
    let steel_tool_substrate = &steel_tool_substrate_file.source;

    require_rust_mod(&turn_mod_file, "ports", "agent turn ports module")?;
    require_contains(&turn_mod, "mod ports;", "agent turn ports module")?;
    require_contains(
        &agent_lib,
        "ProviderModelPort::new(self.provider.as_ref())",
        "provider adapter construction at agent shell edge",
    )?;
    require_contains(&agent_lib, "AgentRuntimeServices", "agent shell runtime service bundle construction")?;
    forbid_contains(&turn_mod, "ProviderModelPort::new(ctx.provider)", "turn loop concrete provider construction")?;
    require_contains(
        &turn_mod,
        "services.tools.tool_definitions()",
        "turn loop reads tool definitions through service bundle",
    )?;
    require_contains(&turn_mod, "services.cost", "turn loop reads cost service through service bundle")?;
    require_contains(
        &turn_mod,
        "services.cancellation",
        "turn loop reads cancellation service through service bundle",
    )?;
    require_struct_field_type_path(
        &adapters_file,
        "AgentModelHost",
        "model_port",
        "AgentModelPort",
        "agent model host port field",
    )?;
    require_struct_field_type_path(
        &adapters_file,
        "AgentToolHost",
        "tool_port",
        "AgentToolPort",
        "agent tool host port field",
    )?;
    require_struct_field_type_path(
        &adapters_file,
        "AgentUsageObserver",
        "cost",
        "AgentCostPort",
        "agent usage observer cost port field",
    )?;
    require_contains(&adapters, "model_port: &'a dyn AgentModelPort", "agent model host port field")?;
    require_contains(&adapters, "tool_port: &'a dyn AgentToolPort", "agent tool host port field")?;
    require_contains(&adapters, "cost: &'a dyn AgentCostPort", "agent usage observer cost port field")?;
    forbid_struct_field_type_path(
        &adapters_file,
        "AgentModelHost",
        "provider",
        "Provider",
        "agent model host concrete provider field",
    )?;
    forbid_struct_field_type_path(
        &adapters_file,
        "AgentToolHost",
        "controller_tools",
        "HashMap",
        "agent tool host concrete tool map field",
    )?;
    forbid_contains(&adapters, "provider: &'a dyn Provider", "agent model host concrete provider field")?;
    forbid_contains(&adapters, "controller_tools: &'a HashMap", "agent tool host concrete tool map field")?;
    require_rust_trait(&ports_file, "AgentModelPort", "agent model port trait")?;
    require_rust_trait(&ports_file, "AgentToolPort", "agent tool port trait")?;
    require_rust_trait(&ports_file, "AgentCostPort", "agent cost port trait")?;
    require_rust_trait(&ports_file, "AgentCancellationPort", "agent cancellation port trait")?;
    require_rust_impl(&ports_file, "AgentModelPort", "ProviderModelPort", "provider model port adapter")?;
    require_rust_impl(&ports_file, "AgentToolPort", "ControllerToolPort", "controller tool port adapter")?;
    require_rust_impl(&ports_file, "AgentCostPort", "CostTrackerPort", "cost tracker port adapter")?;
    require_rust_impl(
        &ports_file,
        "AgentCancellationPort",
        "TokenCancellationPort",
        "cancellation token port adapter",
    )?;
    require_contains(&ports, "trait AgentModelPort", "agent model port trait")?;
    require_contains(&ports, "trait AgentToolPort", "agent tool port trait")?;
    require_contains(&ports, "trait AgentCostPort", "agent cost port trait")?;
    require_contains(&ports, "trait AgentCancellationPort", "agent cancellation port trait")?;
    require_contains(&ports, "struct AgentRuntimeServices", "agent runtime service bundle")?;
    require_contains(&ports, "DESKTOP_AGENT_SERVICE_RECEIPTS", "agent runtime service owner receipts")?;
    for marker in [
        "ModelExecution",
        "ToolRegistry",
        "Storage",
        "PromptContext",
        "Hooks",
        "Skills",
        "Cost",
        "Cancellation",
    ] {
        require_contains(&ports, marker, "agent runtime service receipt kind")?;
    }
    require_contains(&ports, "impl AgentModelPort for ProviderModelPort", "provider model port adapter")?;
    require_contains(&ports, "impl AgentToolPort for ControllerToolPort", "controller tool port adapter")?;
    require_contains(
        &ports,
        "NEUTRAL_TOOL_SERVICE_CONTEXT_REQUIREMENT",
        "neutral tool service context requirement marker",
    )?;
    require_contains(
        &ports,
        "CONTROLLER_TOOL_PORT_SERVICE_INVENTORY",
        "controller tool port concrete service inventory",
    )?;
    require_contains(
        &ports,
        "LEGACY_TOOL_CONTEXT_SERVICE_INVENTORY",
        "legacy tool context concrete service inventory",
    )?;
    require_contains(
        &ports,
        "AGENT_CONCRETE_DEPENDENCY_DRAIN_REQUIREMENT",
        "agent concrete dependency drain requirement marker",
    )?;
    require_contains(
        &ports,
        "AGENT_CONCRETE_DEPENDENCY_BUDGET",
        "agent concrete dependency budget inventory",
    )?;
    for marker in [
        "AgentConcreteDependencyFamily::Provider",
        "AgentConcreteDependencyFamily::StorageSearch",
        "AgentConcreteDependencyFamily::Config",
        "AgentConcreteDependencyFamily::Procmon",
        "AgentConcreteDependencyFamily::DisplayProtocol",
        "AgentConcreteDependencyFamily::Router",
        "AgentToolSteelSubstrateSettings",
    ] {
        require_contains(&ports, marker, "agent concrete dependency budget family")?;
    }
    require_contains(
        steel_tool_substrate,
        "pub struct AgentToolSteelSubstrateSettings",
        "agent-owned Steel tool substrate settings DTO",
    )?;
    require_contains(
        agent_lib,
        "agent_tool_steel_substrate_settings_from_config",
        "Steel tool substrate app-edge settings adapter",
    )?;
    forbid_contains(
        steel_tool_substrate,
        "clankers_config::",
        "Steel tool substrate reusable policy concrete config import",
    )?;
    for (owner, field, type_path, label) in [
        ("ControllerToolPort", "controller_tools", "HashMap", "legacy tool registry service field"),
        (
            "ControllerToolPort",
            "services",
            "ControllerToolServices",
            "neutral controller tool service bundle field",
        ),
        ("ControllerToolServices", "events", "AgentToolEventSink", "progress/event adapter service field"),
        ("ControllerToolServices", "progress", "ToolProgressSink", "neutral progress service field"),
        (
            "ControllerToolServices",
            "cancellation",
            "ToolCancellationService",
            "neutral cancellation service field",
        ),
        ("ControllerToolServices", "storage", "ToolStorageService", "neutral storage service field"),
        ("ControllerToolServices", "search", "ToolSearchService", "neutral search service field"),
        ("ControllerToolServices", "hooks", "ToolHookService", "neutral hook service field"),
        ("ControllerToolServices", "capability", "ToolCapabilityService", "neutral capability service field"),
        (
            "ControllerToolServices",
            "legacy_runner",
            "LegacyToolRunner",
            "legacy storage/search compatibility runner field",
        ),
        (
            "ControllerToolServices",
            "steel_tool_substrate",
            "AgentToolSteelSubstrateConfig",
            "Steel substrate policy compatibility field",
        ),
    ] {
        require_struct_field_type_path(&ports_file, owner, field, type_path, label)?;
    }
    for (field, type_path, label) in [
        ("event_tx", "AgentEvent", "legacy progress/event sender field"),
        ("signal", "CancellationToken", "legacy cancellation field"),
        ("hook_pipeline", "HookPipeline", "legacy hook field"),
        ("db", "Db", "legacy storage field"),
        ("search_index", "SearchIndex", "legacy search-index field"),
    ] {
        require_struct_field_type_path(&tool_file, "ToolContext", field, type_path, label)?;
    }

    Ok(json!({
        "ports_module": AGENT_TURN_PORTS,
        "model_port_trait": "AgentModelPort",
        "tool_port_trait": "AgentToolPort",
        "cost_port_trait": "AgentCostPort",
        "cancellation_port_trait": "AgentCancellationPort",
        "runtime_service_bundle": "AgentRuntimeServices",
        "service_receipt_kinds": ["model", "tools", "storage", "prompt_context", "hooks", "skills", "cost", "cancellation"],
        "model_host_concrete_provider_fields": 0,
        "tool_host_concrete_tool_map_fields": 0,
        "provider_adapter": "ProviderModelPort",
        "tool_adapter": "ControllerToolPort",
        "tool_service_inventory": "CONTROLLER_TOOL_PORT_SERVICE_INVENTORY",
        "legacy_tool_context_inventory": "LEGACY_TOOL_CONTEXT_SERVICE_INVENTORY",
        "concrete_dependency_budget": "AGENT_CONCRETE_DEPENDENCY_BUDGET",
        "selected_config_slice": "Steel tool substrate settings now convert to AgentToolSteelSubstrateSettings at app edge",
        "tool_context_module": AGENT_TOOL,
        "cost_adapter": "CostTrackerPort",
        "cancellation_adapter": "TokenCancellationPort",
        "provider_adapter_owner": "crates/clankers-agent/src/lib.rs app-edge shell",
        "typed_rail_kind": "Rust AST module, trait, impl, struct-field, and service-receipt checks"
    }))
}

fn tool_host_service_context_signature() -> Result<Value, String> {
    let file = read_rust_file(TOOL_HOST_LIB)?;
    let source = &file.source;

    for (path, label) in [
        ("clankers_db", "concrete database service import"),
        ("SearchIndex", "concrete search-index service import"),
        ("clankers_hooks", "concrete hook pipeline import"),
        ("HookPipeline", "concrete hook pipeline type import"),
        ("AgentEvent", "agent progress/event import"),
        ("clanker_tui_types", "TUI DTO import"),
        ("DaemonEvent", "daemon protocol event import"),
        ("SessionCommand", "daemon protocol command import"),
        ("ToolContext", "legacy agent tool context import"),
    ] {
        forbid_rust_path(&file, path, &format!("neutral tool-host context shell leak: {label}"))?;
    }
    for (needle, label) in [
        ("clankers_db", "concrete database service path"),
        ("clankers_hooks", "concrete hook pipeline path"),
        ("clanker_tui_types", "TUI DTO path"),
        ("clankers_protocol", "daemon protocol DTO path"),
        ("crate::tools", "root tool state path"),
        ("src/tools", "root tool source path"),
    ] {
        forbid_contains(source, needle, &format!("neutral tool-host context shell leak: {label}"))?;
    }
    for (needle, label) in [
        ("ToolHostServices", "neutral service bundle"),
        ("ToolHostServiceKind", "neutral service kind DTO"),
        ("ToolHostFuture", "neutral async service future alias"),
        ("ToolStorageService", "neutral storage service trait"),
        ("ToolStorageReadRequest", "neutral storage read DTO"),
        ("ToolSearchService", "neutral search service trait"),
        ("ToolSearchRequest", "neutral search DTO"),
        ("ToolHookService", "neutral hook service trait"),
        ("ToolHookDecision", "neutral hook decision DTO"),
        ("ToolProgressSink", "neutral progress service"),
        ("ToolCapabilityService", "neutral capability service trait"),
        ("ToolCapabilityRequest", "neutral capability DTO"),
        ("ToolCancellationService", "neutral cancellation service trait"),
        ("ToolRuntimePolicyService", "neutral runtime policy service trait"),
        ("ToolRuntimePolicyDecision", "neutral runtime policy decision DTO"),
    ] {
        require_contains(source, needle, label)?;
    }

    Ok(json!({
        "module": TOOL_HOST_LIB,
        "neutral_service_bundle": "ToolHostServices",
        "neutral_service_kinds": "ToolHostServiceKind",
        "neutral_storage_service": "ToolStorageService",
        "neutral_search_service": "ToolSearchService",
        "neutral_hook_service": "ToolHookService",
        "neutral_progress_service": "ToolProgressSink",
        "neutral_capability_service": "ToolCapabilityService",
        "neutral_cancellation_service": "ToolCancellationService",
        "neutral_runtime_policy_service": "ToolRuntimePolicyService",
        "forbidden_shell_imports": [
            "clankers_db",
            "SearchIndex",
            "clankers_hooks",
            "HookPipeline",
            "AgentEvent",
            "clanker_tui_types",
            "DaemonEvent",
            "SessionCommand",
            "ToolContext",
            "crate::tools",
            "src/tools"
        ],
        "requirement": "r[neutral-tool-service-context.verification.boundary-rail]",
        "typed_rail_kind": "Rust AST/source import ownership check over reusable tool-host context module"
    }))
}

fn agent_provider_neutral_dto_signature() -> Result<Value, String> {
    let reusable_modules = [
        AGENT_COMPACTION,
        AGENT_COMPACTION_TOOL_SUMMARIES,
        AGENT_EVENTS,
        AGENT_TURN_EXECUTION,
        AGENT_TURN_MESSAGE,
        AGENT_TURN_MOD,
        AGENT_TURN_POLICY,
        AGENT_TURN_PORTS,
        AGENT_TURN_STEEL_PLANNING,
        AGENT_TURN_STEEL_TOOL_SUBSTRATE,
        AGENT_TURN_TRANSCRIPT,
        AGENT_TURN_USAGE,
    ];
    let forbidden_reexports = [
        ("clankers_provider::message", "neutral message DTOs must come from clanker-message"),
        ("clankers_provider::streaming", "neutral stream DTOs must come from clanker-message"),
        ("clankers_provider::Usage", "usage DTO must come from clanker-message"),
        ("clankers_provider::ThinkingConfig", "thinking config DTO must come from clanker-message"),
    ];

    for module in reusable_modules {
        let source = fs::read_to_string(module).map_err(|error| format!("failed to read {module}: {error}"))?;
        for (needle, reason) in forbidden_reexports {
            forbid_contains(
                &source,
                needle,
                &format!("agent provider neutral DTO import in {module}; {reason}"),
            )?;
        }
    }

    let ports = fs::read_to_string(AGENT_TURN_PORTS)
        .map_err(|error| format!("failed to read {AGENT_TURN_PORTS}: {error}"))?;
    require_contains(
        &ports,
        "convergence: \"replace concrete provider construction at app edge only\"",
        "agent provider dependency convergence receipt",
    )?;

    Ok(json!({
        "checked_modules": reusable_modules,
        "neutral_owner": "clanker-message",
        "model_adapter_modules": [AGENT_TURN_EXECUTION, AGENT_TURN_PORTS],
        "provider_native_allowed_in_model_adapters": ["CompletionRequest", "Provider"],
        "forbidden_provider_reexports": [
            "clankers_provider::message",
            "clankers_provider::streaming",
            "clankers_provider::Usage",
            "clankers_provider::ThinkingConfig"
        ],
        "provider_dependency_convergence": "replace concrete provider construction at app edge only",
        "typed_rail_kind": "source import ownership check over reusable agent policy modules"
    }))
}

fn controller_effect_interpretation_signature() -> Result<Value, String> {
    let core_effects_file = read_rust_file(CONTROLLER_CORE_EFFECTS)?;
    let interpretation_file = read_rust_file(CONTROLLER_EFFECT_INTERPRETATION)?;
    let core_effects = &core_effects_file.source;
    let interpretation = &interpretation_file.source;

    require_rust_path(
        &core_effects_file,
        "effect_interpretation::interpret_prompt_request",
        "controller prompt effect interpretation seam",
    )?;
    require_rust_path(
        &core_effects_file,
        "effect_interpretation::interpret_thinking_change",
        "controller thinking effect interpretation seam",
    )?;
    require_rust_path(
        &core_effects_file,
        "effect_interpretation::interpret_tool_filter_application",
        "controller tool filter effect interpretation seam",
    )?;
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
    let responsibility_file = read_rust_file(PROVIDER_ROUTER_RESPONSIBILITY)?;
    let router_adapter_file = read_rust_file(PROVIDER_ROUTER_ADAPTER)?;
    let rpc_adapter_file = read_rust_file(PROVIDER_RPC_ADAPTER)?;
    let bridge = &bridge_file.source;
    let responsibility = &responsibility_file.source;
    let router_adapter = &router_adapter_file.source;
    let rpc_adapter = &rpc_adapter_file.source;

    require_rust_fn(&bridge_file, "build_router_request", "provider/router bridge entrypoint")?;
    require_rust_fn(
        &bridge_file,
        "compute_router_cache_key_from_request_projection",
        "provider/router cache-key request projection owner",
    )?;
    require_rust_fn(&bridge_file, "messages_to_router_json", "provider/router message projection owner")?;
    require_rust_path(
        &router_adapter_file,
        "crate::router_request_bridge::build_router_request",
        "local router adapter delegates request projection",
    )?;
    require_rust_path(
        &rpc_adapter_file,
        "crate::router_request_bridge::build_router_request",
        "rpc router adapter delegates request projection",
    )?;
    require_rust_enum(
        &responsibility_file,
        "ProviderRouterConcern",
        "provider/router duplicate concern inventory enum",
    )?;
    require_rust_struct(
        &responsibility_file,
        "ProviderRouterConcernOwner",
        "provider/router duplicate concern inventory row",
    )?;
    require_contains(
        &bridge,
        "Single clankers-provider owned bridge into `clanker_router::CompletionRequest`",
        "provider/router bridge ownership doc",
    )?;
    require_contains(&bridge, "pub(crate) fn build_router_request", "provider/router bridge entrypoint")?;
    require_contains(
        &bridge,
        "compute_router_cache_key_from_request_projection",
        "provider/router cache-key request projection owner",
    )?;
    require_contains(&bridge, "messages_to_router_json", "provider/router message projection owner")?;
    require_contains(&bridge, "Branch summary", "branch summary preservation fixture")?;
    require_contains(&bridge, "Compaction summary", "compaction summary preservation fixture")?;
    require_contains(
        &router_adapter,
        "crate::router_request_bridge::build_router_request(request)",
        "local router adapter delegates request projection",
    )?;
    require_contains(
        &router_adapter,
        "crate::router_request_bridge::compute_router_cache_key_from_request_projection",
        "local router cache key delegates request projection",
    )?;
    require_contains(
        &rpc_adapter,
        "crate::router_request_bridge::build_router_request(request)",
        "rpc router adapter delegates request projection",
    )?;
    require_contains(
        &responsibility,
        "PROVIDER_ROUTER_CONCERN_INVENTORY",
        "provider/router duplicate concern inventory",
    )?;
    require_contains(
        &responsibility,
        "ProviderRouterConcern::CacheKeyRequestProjection",
        "selected provider/router cache key projection concern",
    )?;
    require_contains(
        &responsibility,
        "compute_router_cache_key_from_request_projection",
        "selected provider/router single policy owner",
    )?;
    require_contains(
        &responsibility,
        "inventory_names_owner_for_each_provider_router_concern",
        "provider/router duplicate concern inventory fixture",
    )?;
    require_contains(
        &responsibility,
        "selected_cache_key_projection_has_single_bridge_owner",
        "selected provider/router owner fixture",
    )?;
    forbid_rust_fn(
        &router_adapter_file,
        "messages_to_router_json",
        "local router adapter duplicate message projection",
    )?;
    forbid_rust_fn(&rpc_adapter_file, "convert_messages_to_api", "rpc router adapter duplicate message projection")?;
    forbid_rust_fn(&rpc_adapter_file, "content_to_json", "rpc router adapter duplicate content projection")?;
    forbid_contains(
        &router_adapter,
        "fn messages_to_router_json",
        "local router adapter duplicate message projection",
    )?;
    forbid_contains(
        &router_adapter,
        "serde_json::to_value(m)",
        "local router cache key duplicate AgentMessage serialization",
    )?;
    forbid_contains(&rpc_adapter, "fn convert_messages_to_api", "rpc router adapter duplicate message projection")?;
    forbid_contains(&rpc_adapter, "fn content_to_json", "rpc router adapter duplicate content projection")?;

    Ok(json!({
        "bridge_module": PROVIDER_ROUTER_BRIDGE,
        "concern_inventory_module": PROVIDER_ROUTER_RESPONSIBILITY,
        "local_adapter_module": PROVIDER_ROUTER_ADAPTER,
        "rpc_adapter_module": PROVIDER_RPC_ADAPTER,
        "request_projection_owner": "router_request_bridge::build_router_request",
        "cache_key_projection_owner": "router_request_bridge::compute_router_cache_key_from_request_projection",
        "selected_concern": "ProviderRouterConcern::CacheKeyRequestProjection",
        "local_adapter_duplicate_message_projection": 0,
        "local_adapter_duplicate_cache_key_message_projection": 0,
        "rpc_adapter_duplicate_message_projection": 0,
        "summary_context_preserved": true,
        "typed_rail_kind": "Rust AST function/enum/struct ownership and call-path checks"
    }))
}

fn controller_domain_event_signature() -> Result<Value, String> {
    let domain_event_file = read_rust_file(CONTROLLER_DOMAIN_EVENT)?;
    let convert_file = read_rust_file(CONTROLLER_CONVERT)?;
    let domain_event = &domain_event_file.source;
    let convert = &convert_file.source;

    require_rust_fn(
        &domain_event_file,
        "agent_event_to_domain_event",
        "agent/runtime event to shared semantic event projection",
    )?;
    require_rust_fn(&domain_event_file, "tool_content_to_domain_parts", "neutral tool receipt projection")?;
    forbid_rust_path(&domain_event_file, "DaemonEvent", "semantic event adapter protocol DTO leakage")?;
    forbid_rust_path(&domain_event_file, "TuiEvent", "semantic event adapter TUI DTO leakage")?;
    forbid_rust_path(&domain_event_file, "clankers_protocol", "semantic event adapter protocol crate dependency")?;
    forbid_rust_path(&domain_event_file, "clanker_tui_types", "semantic event adapter TUI crate dependency")?;
    require_rust_path(
        &domain_event_file,
        "SemanticEvent",
        "controller compatibility alias points at shared semantic event contract",
    )?;
    require_rust_path(
        &convert_file,
        "agent_event_to_domain_event",
        "protocol projection delegates through semantic event seam",
    )?;
    require_rust_path(
        &convert_file,
        "semantic_event_to_daemon_event",
        "daemon projection is owned by semantic event edge adapter",
    )?;
    require_rust_path(
        &convert_file,
        "semantic_event_to_tui_event",
        "TUI projection is owned by semantic event edge adapter",
    )?;

    require_contains(
        domain_event,
        "ControllerDomainEvent` is kept as the controller-local compatibility name",
        "controller domain event seam convergence note",
    )?;
    require_contains(
        domain_event,
        "pub(crate) type ControllerDomainEvent = SemanticEvent",
        "controller domain event compatibility alias",
    )?;
    require_contains(
        domain_event,
        "pub(crate) type DomainImage = SemanticImage",
        "neutral image receipt DTO compatibility alias",
    )?;
    require_contains(
        domain_event,
        "agent_event_to_domain_event",
        "agent/runtime event to semantic event projection",
    )?;
    require_contains(domain_event, "tool_content_to_domain_parts", "neutral tool receipt projection")?;
    require_contains(
        domain_event,
        "projects_agent_streaming_without_protocol_or_tui_types",
        "neutral streaming projection fixture",
    )?;
    require_contains(
        domain_event,
        "projects_tool_receipts_to_neutral_text_and_images",
        "neutral receipt projection fixture",
    )?;
    forbid_contains(domain_event, "DaemonEvent", "semantic event adapter protocol DTO leakage")?;
    forbid_contains(domain_event, "TuiEvent", "semantic event adapter TUI DTO leakage")?;
    forbid_contains(domain_event, "clankers_protocol", "semantic event adapter protocol crate dependency")?;
    forbid_contains(domain_event, "clanker_tui_types", "semantic event adapter TUI crate dependency")?;
    require_contains(
        convert,
        "agent_event_to_domain_event(event).and_then(|event| semantic_event_to_daemon_event(&event))",
        "protocol projection delegates through semantic event seam",
    )?;
    require_contains(
        convert,
        "semantic_event_projection_preserves_daemon_tui_and_json_shapes",
        "semantic edge projection parity fixture",
    )?;

    Ok(json!({
        "domain_event_module": CONTROLLER_DOMAIN_EVENT,
        "protocol_projection_module": CONTROLLER_CONVERT,
        "neutral_event_contract": "clanker_message::SemanticEvent",
        "compatibility_alias": "ControllerDomainEvent",
        "neutral_receipt_dto": "SemanticImage",
        "protocol_references_in_domain_module": 0,
        "tui_references_in_domain_module": 0,
        "protocol_projection_owner": "convert::semantic_event_to_daemon_event",
        "typed_rail_kind": "Rust AST function, path, alias, and forbidden edge-dependency checks"
    }))
}

fn controller_display_protocol_dto_signature() -> Result<Value, String> {
    let command_file = read_rust_file(CONTROLLER_COMMAND)?;
    let thinking_file = read_rust_file(CONTROLLER_COMMAND_THINKING)?;
    let auto_test_file = read_rust_file(CONTROLLER_AUTO_TEST)?;
    let convert_file = read_rust_file(CONTROLLER_CONVERT)?;
    let command = &command_file.source;
    let thinking = &thinking_file.source;
    let auto_test = &auto_test_file.source;
    let forbidden_display_dtos = [
        (
            "clanker_tui_types::ThinkingLevel",
            "controller-display-protocol-dto-drain.neutral-inputs.thinking",
            "TUI/attach projection edge; controller command policy must use CoreThinkingLevel",
        ),
        (
            "clanker_tui_types::LoopDisplayState",
            "controller-display-protocol-dto-drain.neutral-inputs.loop-state",
            "TUI event-loop projection edge; controller auto-test policy must use ControllerLoopStatus",
        ),
    ];

    for (dto, requirement, allowed_owner) in forbidden_display_dtos {
        forbid_rust_path(
            &command_file,
            dto,
            &format!("{requirement}: {dto} belongs to {allowed_owner}, not controller command policy"),
        )?;
        forbid_rust_path(
            &thinking_file,
            dto,
            &format!("{requirement}: {dto} belongs to {allowed_owner}, not controller thinking command policy"),
        )?;
        forbid_rust_path(
            &auto_test_file,
            dto,
            &format!("{requirement}: {dto} belongs to {allowed_owner}, not controller auto-test policy"),
        )?;
        forbid_contains(
            command,
            dto,
            &format!("{requirement}: {dto} belongs to {allowed_owner}, not controller command policy"),
        )?;
        forbid_contains(
            thinking,
            dto,
            &format!("{requirement}: {dto} belongs to {allowed_owner}, not controller thinking command policy"),
        )?;
        forbid_contains(
            auto_test,
            dto,
            &format!("{requirement}: {dto} belongs to {allowed_owner}, not controller auto-test policy"),
        )?;
    }

    require_rust_path(&thinking_file, "CoreThinkingLevel", "neutral controller thinking DTO")?;
    require_rust_path(&thinking_file, "CoreThinkingLevelInput", "neutral controller thinking input DTO")?;
    require_rust_struct(&auto_test_file, "ControllerLoopStatus", "neutral controller loop status DTO")?;
    require_rust_path(
        &thinking_file,
        "semantic_error_message_to_daemon_event",
        "command user-visible error semantic projection call",
    )?;
    require_rust_path(
        &thinking_file,
        "SemanticErrorClass::InvalidInput",
        "invalid thinking-level semantic error classification",
    )?;
    require_rust_fn(
        &convert_file,
        "semantic_error_message_to_daemon_event",
        "semantic error protocol projection owner",
    )?;
    require_rust_path(&convert_file, "SemanticEvent::Error", "semantic error protocol projection owner")?;
    require_rust_path(&convert_file, "DaemonEvent::SystemMessage", "daemon system-message projection owner")?;
    require_contains(
        thinking,
        "parser_uses_core_levels_without_tui_dto",
        "neutral thinking parser fixture",
    )?;
    require_contains(auto_test, "ControllerLoopStatus", "neutral loop status edge DTO")?;
    require_contains(
        &convert_file.source,
        "semantic_error_message_projects_through_daemon_system_message",
        "semantic error projection parity fixture",
    )?;

    Ok(json!({
        "command_policy_module": CONTROLLER_COMMAND,
        "auto_test_policy_module": CONTROLLER_AUTO_TEST,
        "projection_owner": CONTROLLER_CONVERT,
        "neutral_thinking_owner": "clankers-core::CoreThinkingLevel",
        "neutral_loop_status_owner": "clankers-controller::auto_test::ControllerLoopStatus",
        "forbidden_display_dtos": [
            {
                "dto": "clanker_tui_types::ThinkingLevel",
                "allowed_owner": "TUI/attach projection edge",
                "requirement": "controller-display-protocol-dto-drain.neutral-inputs.thinking"
            },
            {
                "dto": "clanker_tui_types::LoopDisplayState",
                "allowed_owner": "TUI event-loop projection edge",
                "requirement": "controller-display-protocol-dto-drain.neutral-inputs.loop-state"
            }
        ],
        "command_semantic_projection": "semantic_error_message_to_daemon_event",
        "protocol_projection_owner": "convert::semantic_error_message_to_daemon_event",
        "typed_rail_kind": "Rust AST path, struct, function, and owner-diagnostic checks for display/protocol DTO drains"
    }))
}

fn controller_command_responsibility_signature() -> Result<Value, String> {
    let inventory_file = read_rust_file(CONTROLLER_COMMAND_RESPONSIBILITY)?;
    let thinking_file = read_rust_file(CONTROLLER_COMMAND_THINKING)?;
    let command_file = read_rust_file(CONTROLLER_COMMAND)?;
    let inventory = &inventory_file.source;
    let thinking = &thinking_file.source;
    let command = &command_file.source;

    require_contains(
        inventory,
        "CONTROLLER_COMMAND_RESPONSIBILITY_DRAIN_REQUIREMENT",
        "controller command responsibility drain requirement marker",
    )?;
    require_contains(
        inventory,
        "COMMAND_RESPONSIBILITY_INVENTORY",
        "controller command responsibility inventory",
    )?;
    for marker in [
        "CommandResponsibilityKind::Translation",
        "CommandResponsibilityKind::Authorization",
        "CommandResponsibilityKind::CoreInputConstruction",
        "CommandResponsibilityKind::RuntimeDispatch",
        "CommandResponsibilityKind::Persistence",
        "CommandResponsibilityKind::Continuation",
        "CommandResponsibilityKind::Projection",
    ] {
        require_contains(inventory, marker, "controller command responsibility owner")?;
    }
    require_rust_path(
        &thinking_file,
        "CoreInput::SetThinkingLevel",
        "thinking command CoreInput owner",
    )?;
    require_rust_path(
        &thinking_file,
        "CoreInput::CycleThinkingLevel",
        "thinking command CoreInput owner",
    )?;
    require_contains(thinking, "pub(crate) fn handle_set_thinking_level", "thinking set command owner")?;
    require_contains(thinking, "pub(crate) fn handle_cycle_thinking_level", "thinking cycle command owner")?;
    forbid_contains(command, "fn parse_core_thinking_level", "command root thinking parser owner")?;
    require_contains(
        command,
        "self.handle_set_thinking_level(level)",
        "command dispatch delegates thinking set cluster",
    )?;
    require_contains(
        command,
        "self.handle_cycle_thinking_level()",
        "command dispatch delegates thinking cycle cluster",
    )?;

    Ok(json!({
        "inventory_module": CONTROLLER_COMMAND_RESPONSIBILITY,
        "thinking_command_module": CONTROLLER_COMMAND_THINKING,
        "root_command_module": CONTROLLER_COMMAND,
        "responsibility_kinds": ["translation", "authorization", "core_input", "runtime_dispatch", "persistence", "continuation", "projection"],
        "extracted_cluster": "thinking command parsing and CoreInput construction",
        "projection_owner": CONTROLLER_CONVERT,
        "typed_rail_kind": "Rust AST/source ownership rail for command responsibility drain"
    }))
}

fn daemon_session_assembly_signature() -> Result<Value, String> {
    let agent_process_file = read_rust_file(DAEMON_AGENT_PROCESS)?;
    let builder_file = read_rust_file(DAEMON_SESSION_BUILDER)?;
    let plugins_file = read_rust_file(DAEMON_SESSION_PLUGINS)?;
    let agent_process = &agent_process_file.source;
    let builder = &builder_file.source;
    let plugins = &plugins_file.source;
    let actor_forbidden = [
        ("AgentBuilder", "agent builder construction belongs to session_builder"),
        ("UcanCapabilityGate", "capability gate construction belongs to session_builder"),
        ("PublicUcanCapabilityGate", "public capability gate construction belongs to session_builder"),
        ("ScriptHookHandler", "hook pipeline construction belongs to session_builder"),
        ("GitHookHandler", "hook pipeline construction belongs to session_builder"),
        ("PluginHookHandler", "plugin hook attachment belongs to session_builder"),
        ("sync_tool_inventory", "tool-list tick policy belongs to DaemonSessionTickService"),
        ("drain_plugin_runtime_events", "plugin runtime drain policy belongs to DaemonSessionTickService"),
        ("drain_and_broadcast", "controller event drain policy belongs to DaemonSessionTickService"),
        ("merge_session_capabilities", "capability merge belongs to session_builder"),
        ("build_session_hook_pipeline", "hook assembly belongs to session_builder"),
        ("build_all_tiered_tools", "tool catalog projection belongs to session_plugins"),
        ("build_protocol_plugin_summaries", "plugin summary projection belongs to session_plugins"),
    ];

    for (path, reason) in actor_forbidden {
        forbid_rust_path(&agent_process_file, path, &format!("daemon actor loop assembly split: {reason}"))?;
        forbid_contains(agent_process, path, &format!("daemon actor loop assembly split: {reason}"))?;
    }

    require_rust_path(
        &agent_process_file,
        "assemble_session_runtime",
        "daemon actor consumes assembled session runtime",
    )?;
    require_rust_path(&agent_process_file, "DaemonSessionRuntimeRequest", "daemon actor runtime request DTO")?;
    require_contains(
        agent_process,
        "plan_ephemeral_child_session",
        "ephemeral child spawn path uses socketless builder plan",
    )?;
    require_contains(
        agent_process,
        "actor_tick_service.drain_background",
        "actor loop triggers periodic daemon projection through assembled tick service",
    )?;
    require_rust_struct(&builder_file, "DaemonSessionRuntime", "assembled daemon session runtime bundle")?;
    require_rust_struct(
        &builder_file,
        "DaemonSessionRuntimeRequest",
        "daemon session runtime request DTO",
    )?;
    require_rust_fn(&builder_file, "assemble_session_runtime", "socketless daemon runtime assembly entrypoint")?;
    require_rust_fn(&builder_file, "build_session_hook_pipeline", "builder-owned hook pipeline assembly")?;
    require_rust_fn(&builder_file, "merge_session_capabilities", "builder-owned capability merge")?;
    require_rust_method(&builder_file, "plan_ephemeral_child_session", "ephemeral child socketless spawn plan")?;
    require_contains(builder, "clankers_agent::builder::AgentBuilder::new", "builder-owned agent construction")?;
    require_rust_path(&builder_file, "DaemonSessionTickService", "builder assembles daemon tick service")?;
    require_rust_path(&builder_file, "tool_rebuilder_for_factory", "builder wires named tool rebuilder helper")?;
    require_rust_struct(&plugins_file, "DaemonPluginProjection", "daemon plugin protocol projection handle")?;
    require_rust_struct(&plugins_file, "DaemonSessionTickService", "daemon actor-loop tick service owner")?;
    require_rust_struct(&plugins_file, "DaemonToolRebuilder", "daemon tool rebuilder projection owner")?;
    require_rust_fn(&plugins_file, "sync_tool_inventory", "daemon tool-list refresh helper")?;
    require_rust_fn(&plugins_file, "drain_plugin_runtime_events", "daemon plugin runtime drain helper")?;
    require_rust_path(
        &plugins_file,
        "crate::plugin::build_protocol_plugin_summaries",
        "plugin summary projection owner",
    )?;
    require_rust_path(
        &plugins_file,
        "crate::modes::common::build_all_tiered_tools",
        "tool catalog projection owner",
    )?;
    require_contains(
        builder,
        "runtime_bundle_assembles_controller_channels_and_tick_service_without_actor_or_socket",
        "socketless runtime bundle fixture",
    )?;
    require_contains(
        builder,
        "ephemeral_plan_prepares_child_actor_inputs_without_socket",
        "ephemeral child socketless spawn fixture",
    )?;
    require_contains(
        agent_process,
        "shared_plugin_disconnect_and_reconnect_updates_all_sessions",
        "daemon actor plugin live refresh parity fixture",
    )?;
    require_contains(
        plugins,
        "Session actors trigger a tick service",
        "tool/plugin projection module purpose",
    )?;
    require_contains(
        plugins,
        "tick_service_refreshes_tool_inventory_without_socket",
        "daemon tick service socketless fixture",
    )?;

    Ok(json!({
        "actor_loop_module": DAEMON_AGENT_PROCESS,
        "runtime_builder_module": DAEMON_SESSION_BUILDER,
        "tool_plugin_projection_module": DAEMON_SESSION_PLUGINS,
        "runtime_bundle": "DaemonSessionRuntime",
        "runtime_request": "DaemonSessionRuntimeRequest",
        "actor_spawn_entrypoint": "assemble_session_runtime",
        "ephemeral_spawn_plan": "plan_ephemeral_child_session",
        "hook_pipeline_owner": "session_builder::build_session_hook_pipeline",
        "capability_merge_owner": "session_builder::merge_session_capabilities",
        "tool_rebuilder_owner": "session_plugins::DaemonToolRebuilder",
        "plugin_projection_owner": "session_plugins::DaemonPluginProjection",
        "tick_service_owner": "session_plugins::DaemonSessionTickService",
        "actor_forbidden_assembly_paths": [
            "AgentBuilder",
            "UcanCapabilityGate",
            "PublicUcanCapabilityGate",
            "ScriptHookHandler",
            "GitHookHandler",
            "PluginHookHandler",
            "sync_tool_inventory",
            "drain_plugin_runtime_events",
            "drain_and_broadcast",
            "merge_session_capabilities",
            "build_session_hook_pipeline",
            "build_all_tiered_tools",
            "build_protocol_plugin_summaries"
        ],
        "socketless_fixtures": [
            "create_plan_for_new_session_has_spawn_and_handle_data_without_socket",
            "create_plan_resolves_resume_messages_without_socket",
            "keyed_plans_prepare_new_and_recovered_actor_inputs_without_socket",
            "ephemeral_plan_prepares_child_actor_inputs_without_socket",
            "runtime_bundle_assembles_controller_channels_and_tick_service_without_actor_or_socket",
            "tick_service_refreshes_tool_inventory_without_socket",
            "tick_service_reports_tool_changes_after_controller_command_without_socket"
        ],
        "typed_rail_kind": "Rust AST path, struct, function, method, and source-owner checks for daemon session assembly"
    }))
}

fn session_command_policy_signature() -> Result<Value, String> {
    let policy_file = read_rust_file(SESSION_COMMAND_POLICY)?;
    let attach_file = read_rust_file(ATTACH_COMMANDS)?;
    let slash_effects_file = read_rust_file(SLASH_EFFECTS)?;
    let agent_task_file = read_rust_file(AGENT_TASK)?;
    let policy = &policy_file.source;
    let attach = &attach_file.source;
    let slash_effects = &slash_effects_file.source;
    let agent_task = &agent_task_file.source;

    require_rust_enum(&policy_file, "LocalSessionEffect", "typed local session effect DTO")?;
    require_rust_enum(&policy_file, "SessionAckPolicy", "typed session ack policy DTO")?;
    require_rust_struct(&policy_file, "SessionCommandEffect", "typed session command effect DTO")?;
    for function in [
        "set_thinking_level_effect",
        "cycle_thinking_level_effect",
        "disabled_tools_effect",
        "manual_compaction_effect",
        "ack_matches",
    ] {
        require_rust_fn(&policy_file, function, "shared session command policy function")?;
    }
    require_rust_path(
        &slash_effects_file,
        "session_command_policy::cycle_thinking_level_effect",
        "slash cycle thinking delegates to shared policy before attach dispatch",
    )?;
    require_rust_path(
        &attach_file,
        "session_command_policy::ack_matches",
        "attach ack suppression delegates to shared policy",
    )?;
    require_rust_path(
        &agent_task_file,
        "session_command_policy::thinking_level_message",
        "standalone thinking message delegates to shared policy",
    )?;

    require_contains(&policy, "Shared session command/effect/ack policy", "session command policy module purpose")?;
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
        &slash_effects,
        "session_command_policy::cycle_thinking_level_effect(current_thinking_level)",
        "slash cycle thinking delegates to shared policy before attach dispatch",
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
        "slash_effects_module": SLASH_EFFECTS,
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
        if item_impl
            .items
            .iter()
            .any(|item| matches!(item, syn::ImplItem::Fn(function) if function.sig.ident == name))
        {
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
        let Some((_, trait_path, _)) = &item_impl.trait_ else {
            continue;
        };
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
        Some(false) => {
            Err(format!("missing {label}: field `{struct_name}.{field_name}` does not reference `{type_path}`"))
        }
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
        let syn::Fields::Named(fields) = &item_struct.fields else {
            return None;
        };
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
    path.segments.iter().map(|segment| segment.ident.to_string()).collect::<Vec<_>>().join("::")
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
