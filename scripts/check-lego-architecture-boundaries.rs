#!/usr/bin/env -S nix develop -c cargo -q -Zscript
---cargo
[package]
edition = "2024"

[dependencies]
blake3 = "1"
serde_json = "1"
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

const ERROR_EXIT: u8 = 1;
const ROOT_PACKAGE: &str = "clankers";
const AGENT_PACKAGE: &str = "clankers-agent";
const CONTROLLER_PACKAGE: &str = "clankers-controller";
const DEFAULT_OUTPUT: &str = "target/lego-architecture/dependency-ownership-inventory.json";
const BASELINE: &str = "policy/lego-architecture/dependency-ownership-baseline.json";
const CHANGE_TASKS: &str = "cairn/changes/lego-decoupling-boundaries/tasks.md";
const CHANGE_SPEC: &str = "cairn/changes/lego-decoupling-boundaries/specs/lego-architecture-boundaries/spec.md";
const PROCESS_TOOL: &str = "src/tools/process.rs";
const PROCESS_TOOL_ADAPTER: &str = "src/tools/process/adapter.rs";

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
    });
    validate_baseline(&signature)?;

    let receipt = json!({
        "schema": "clankers.lego_architecture.dependency_ownership_inventory.v1",
        "change": "lego-decoupling-boundaries",
        "requirements": [
            "r[lego-architecture-boundaries.root-shell-thinness]",
            "r[lego-architecture-boundaries.process-tool-thin-adapter]",
            "r[lego-architecture-boundaries.typed-architecture-rails]"
        ],
        "inventory_signature": signature,
        "rail_kind": "typed cargo metadata inventory",
        "baseline": BASELINE,
        "source_artifacts": [CHANGE_TASKS, CHANGE_SPEC],
        "artifact_hashes": [
            hash_artifact(Path::new(BASELINE))?,
            hash_artifact(Path::new(CHANGE_TASKS))?,
            hash_artifact(Path::new(CHANGE_SPEC))?,
            hash_artifact(Path::new(PROCESS_TOOL))?,
            hash_artifact(Path::new(PROCESS_TOOL_ADAPTER))?
        ]
    });

    write_receipt(&receipt)
}

fn validate_change_contracts() -> Result<(), String> {
    let tasks = fs::read_to_string(CHANGE_TASKS).map_err(|error| format!("failed to read {CHANGE_TASKS}: {error}"))?;
    let spec = fs::read_to_string(CHANGE_SPEC).map_err(|error| format!("failed to read {CHANGE_SPEC}: {error}"))?;
    require_contains(&tasks, "dependency ownership inventory", "tasks I1/V1 dependency inventory")?;
    require_contains(&tasks, "r[lego-architecture-boundaries.root-shell-thinness]", "root shell task coverage")?;
    require_contains(&tasks, "r[lego-architecture-boundaries.typed-architecture-rails]", "typed rail task coverage")?;
    require_contains(&tasks, "process` tool JSON adapter boundary", "process tool adapter extraction task")?;
    require_contains(&spec, "dependency owner", "dependency owner requirement text")?;
    require_contains(&spec, "source crate, target crate", "typed dependency diagnostic scenario")?;
    require_contains(
        &spec,
        "adapter MUST NOT construct or mutate persisted database DTOs",
        "process adapter storage DTO boundary",
    )?;
    Ok(())
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
    let tool = fs::read_to_string(PROCESS_TOOL).map_err(|error| format!("failed to read {PROCESS_TOOL}: {error}"))?;
    let adapter = fs::read_to_string(PROCESS_TOOL_ADAPTER)
        .map_err(|error| format!("failed to read {PROCESS_TOOL_ADAPTER}: {error}"))?;
    require_contains(&tool, "mod adapter;", "process tool adapter module")?;
    require_contains(
        &tool,
        "ProcessToolJsonAdapter::process_job_tool_request(params)",
        "process tool request parser delegation",
    )?;
    forbid_contains(&adapter, "clankers_db::", "process adapter storage DTO import")?;
    forbid_contains(&adapter, "StoredProcessJob", "process adapter persisted DTO reference")?;
    require_contains(
        &adapter,
        "pub(super) fn process_job_tool_request",
        "process adapter typed request parser entrypoint",
    )?;
    require_contains(&adapter, "ProcessJobToolRequest::Start", "process adapter typed start projection")?;
    require_contains(&adapter, "Unknown process action", "process adapter fail-closed unsupported action")?;
    Ok(json!({
        "adapter_module": PROCESS_TOOL_ADAPTER,
        "tool_module": PROCESS_TOOL,
        "storage_dto_imports": 0,
        "storage_dto_references": 0,
        "request_parser_owner": "ProcessToolJsonAdapter",
        "fail_closed_negative_path": "unsupported action returns ToolResult error before backend dispatch"
    }))
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
