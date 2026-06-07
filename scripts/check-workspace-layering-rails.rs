#!/usr/bin/env -S nix develop -c cargo -q -Zscript
---cargo
[package]
edition = "2024"

[dependencies]
blake3 = "1"
serde = { version = "1", features = ["derive"] }
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

use serde::Deserialize;
use serde_json::Value;
use serde_json::json;
use syn::visit::Visit;

const ERROR_EXIT: u8 = 1;
const POLICY_PATH: &str = "policy/workspace-layering/layers.json";
const DEFAULT_OUTPUT: &str = "target/workspace-layering/workspace-layering-inventory.json";
const WORKSPACE_SPEC: &str = "cairn/specs/remaining-coupling-drain/spec.md";
const CHANGE_SPEC_ACTIVE: &str =
    "cairn/changes/enforce-workspace-layering-rails/specs/remaining-coupling-drain/spec.md";
const CHANGE_TASKS_ACTIVE: &str = "cairn/changes/enforce-workspace-layering-rails/tasks.md";
const CHANGE_SPEC_ARCHIVE: &str =
    "cairn/archive/2026-06-06-enforce-workspace-layering-rails/specs/remaining-coupling-drain/spec.md";
const CHANGE_TASKS_ARCHIVE: &str = "cairn/archive/2026-06-06-enforce-workspace-layering-rails/tasks.md";

#[derive(Debug, Deserialize)]
struct LayerPolicy {
    schema: String,
    layers: Vec<LayerSpec>,
    #[serde(default)]
    allowed_upward_edges: Vec<AllowedEdge>,
    ast_constructor_guard: AstConstructorGuard,
}

#[derive(Debug, Deserialize)]
struct LayerSpec {
    id: String,
    rank: u32,
    description: String,
    packages: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct AllowedEdge {
    from: String,
    to: String,
    reason: String,
}

#[derive(Debug, Deserialize)]
struct AstConstructorGuard {
    max_source_rank: u32,
    description: String,
}

#[derive(Debug, Clone)]
struct PackageLayer {
    package: String,
    layer: String,
    rank: u32,
}

#[derive(Debug, Clone)]
struct PackageInfo {
    name: String,
    manifest_path: PathBuf,
    dependencies: Vec<DependencyInfo>,
}

#[derive(Debug, Clone)]
struct DependencyInfo {
    name: String,
    kind: Option<String>,
}

#[derive(Debug, Clone)]
struct LayerViolation {
    source: String,
    source_layer: String,
    target: String,
    target_layer: String,
    message: String,
}

#[derive(Debug, Clone)]
struct AstViolation {
    source_package: String,
    source_layer: String,
    source_file: PathBuf,
    referenced_package: String,
    referenced_layer: String,
    path: String,
    message: String,
}

fn main() -> ExitCode {
    match run() {
        Ok(path) => {
            println!("workspace layering inventory written to {}", path.display());
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("workspace layering rail failed: {error}");
            ExitCode::from(ERROR_EXIT)
        }
    }
}

fn run() -> Result<PathBuf, String> {
    let policy = read_policy()?;
    validate_policy_contract(&policy)?;
    let package_layers = package_layers(&policy)?;
    let packages = cargo_workspace_packages()?;
    validate_workspace_coverage(&packages, &package_layers)?;

    let dependency_violations = dependency_layer_violations(&policy, &packages, &package_layers);
    let ast_violations = ast_constructor_violations(&policy, &packages, &package_layers)?;
    if !dependency_violations.is_empty() || !ast_violations.is_empty() {
        return Err(format_diagnostics(&dependency_violations, &ast_violations));
    }

    let output = write_inventory(&policy, &packages, &package_layers)?;
    Ok(output)
}

fn read_policy() -> Result<LayerPolicy, String> {
    let text = fs::read_to_string(POLICY_PATH).map_err(|error| format!("failed to read {POLICY_PATH}: {error}"))?;
    serde_json::from_str(&text).map_err(|error| format!("failed to parse {POLICY_PATH}: {error}"))
}

fn validate_policy_contract(policy: &LayerPolicy) -> Result<(), String> {
    if policy.schema != "clankers.workspace_layering.v1" {
        return Err(format!("unexpected workspace layering schema: {}", policy.schema));
    }
    let mut ranks = BTreeSet::new();
    let mut packages = BTreeSet::new();
    for layer in &policy.layers {
        if layer.id.is_empty() || layer.description.is_empty() {
            return Err("workspace layering policy layer must include id and description".to_string());
        }
        ranks.insert(layer.rank);
        for package in &layer.packages {
            if !packages.insert(package.clone()) {
                return Err(format!("workspace package `{package}` appears in multiple layers"));
            }
        }
    }
    if ranks.len() != policy.layers.len() {
        return Err("workspace layering policy ranks must be unique".to_string());
    }
    if policy.ast_constructor_guard.description.is_empty() {
        return Err("workspace layering AST guard must describe its replacement path".to_string());
    }
    Ok(())
}

fn package_layers(policy: &LayerPolicy) -> Result<BTreeMap<String, PackageLayer>, String> {
    let mut layers = BTreeMap::new();
    for layer in &policy.layers {
        for package in &layer.packages {
            layers.insert(package.clone(), PackageLayer {
                package: package.clone(),
                layer: layer.id.clone(),
                rank: layer.rank,
            });
        }
    }
    Ok(layers)
}

fn cargo_workspace_packages() -> Result<BTreeMap<String, PackageInfo>, String> {
    let output = Command::new("cargo")
        .args(["metadata", "--format-version", "1", "--no-deps"])
        .output()
        .map_err(|error| format!("failed to run cargo metadata: {error}"))?;
    if !output.status.success() {
        return Err(format!(
            "cargo metadata failed with status {:?}\nstdout:\n{}\nstderr:\n{}",
            output.status.code(),
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    let metadata: Value = serde_json::from_slice(&output.stdout)
        .map_err(|error| format!("failed to parse cargo metadata JSON: {error}"))?;
    let packages = metadata
        .get("packages")
        .and_then(Value::as_array)
        .ok_or_else(|| "cargo metadata missing packages array".to_string())?;
    let workspace_members = metadata
        .get("workspace_members")
        .and_then(Value::as_array)
        .ok_or_else(|| "cargo metadata missing workspace_members array".to_string())?
        .iter()
        .filter_map(Value::as_str)
        .map(ToOwned::to_owned)
        .collect::<BTreeSet<_>>();
    let mut package_names = BTreeSet::new();
    for package in packages {
        let id = package
            .get("id")
            .and_then(Value::as_str)
            .ok_or_else(|| "cargo metadata package missing id".to_string())?;
        if workspace_members.contains(id) {
            let name = package
                .get("name")
                .and_then(Value::as_str)
                .ok_or_else(|| "cargo metadata package missing name".to_string())?;
            package_names.insert(name.to_string());
        }
    }

    let mut result = BTreeMap::new();
    for package in packages {
        let id = package
            .get("id")
            .and_then(Value::as_str)
            .ok_or_else(|| "cargo metadata package missing id".to_string())?;
        if !workspace_members.contains(id) {
            continue;
        }
        let name = package
            .get("name")
            .and_then(Value::as_str)
            .ok_or_else(|| "cargo metadata package missing name".to_string())?
            .to_string();
        let manifest_path = PathBuf::from(
            package
                .get("manifest_path")
                .and_then(Value::as_str)
                .ok_or_else(|| format!("package {name} missing manifest_path"))?,
        );
        let dependencies = package
            .get("dependencies")
            .and_then(Value::as_array)
            .ok_or_else(|| format!("package {name} missing dependencies array"))?
            .iter()
            .filter_map(|dependency| dependency_info(dependency, &package_names))
            .collect::<Vec<_>>();
        result.insert(name.clone(), PackageInfo {
            name,
            manifest_path,
            dependencies,
        });
    }
    Ok(result)
}

fn dependency_info(dependency: &Value, package_names: &BTreeSet<String>) -> Option<DependencyInfo> {
    let name = dependency.get("name")?.as_str()?.to_string();
    if !package_names.contains(&name) {
        return None;
    }
    let kind = dependency.get("kind").and_then(Value::as_str).map(ToOwned::to_owned);
    Some(DependencyInfo { name, kind })
}

fn validate_workspace_coverage(
    packages: &BTreeMap<String, PackageInfo>,
    package_layers: &BTreeMap<String, PackageLayer>,
) -> Result<(), String> {
    let missing = packages
        .keys()
        .filter(|package| !package_layers.contains_key(*package))
        .cloned()
        .collect::<Vec<_>>();
    if !missing.is_empty() {
        return Err(format!("workspace layering policy missing packages: {}", missing.join(", ")));
    }
    let stale = package_layers
        .keys()
        .filter(|package| !packages.contains_key(*package))
        .cloned()
        .collect::<Vec<_>>();
    if !stale.is_empty() {
        return Err(format!("workspace layering policy names non-workspace packages: {}", stale.join(", ")));
    }
    Ok(())
}

fn dependency_layer_violations(
    policy: &LayerPolicy,
    packages: &BTreeMap<String, PackageInfo>,
    package_layers: &BTreeMap<String, PackageLayer>,
) -> Vec<LayerViolation> {
    let allowed = policy
        .allowed_upward_edges
        .iter()
        .map(|edge| (edge.from.as_str(), edge.to.as_str()))
        .collect::<BTreeSet<_>>();
    let mut violations = Vec::new();
    for package in packages.values() {
        let source_layer = package_layers.get(&package.name).expect("coverage checked");
        for dependency in &package.dependencies {
            if dependency.kind.as_deref() == Some("dev") {
                continue;
            }
            let target_layer = package_layers.get(&dependency.name).expect("coverage checked");
            if source_layer.rank < target_layer.rank
                && !allowed.contains(&(package.name.as_str(), dependency.name.as_str()))
            {
                let allowed_hint = allowed_edge_hint(policy, &package.name, &dependency.name);
                violations.push(LayerViolation {
                    source: package.name.clone(),
                    source_layer: source_layer.layer.clone(),
                    target: dependency.name.clone(),
                    target_layer: target_layer.layer.clone(),
                    message: format!(
                        "{} ({}) depends upward on {} ({}); move dependency behind a lower-layer adapter or add a reviewed policy exception{allowed_hint}",
                        package.name, source_layer.layer, dependency.name, target_layer.layer
                    ),
                });
            }
        }
    }
    violations
}

fn allowed_edge_hint(policy: &LayerPolicy, from: &str, to: &str) -> String {
    let same_from = policy
        .allowed_upward_edges
        .iter()
        .filter(|edge| edge.from == from)
        .map(|edge| format!("{} ({})", edge.to, edge.reason))
        .collect::<Vec<_>>();
    if same_from.is_empty() {
        format!(" for {from}->{to}")
    } else {
        format!(" for {from}->{to}; existing exceptions for {from}: {}", same_from.join(", "))
    }
}

fn ast_constructor_violations(
    policy: &LayerPolicy,
    packages: &BTreeMap<String, PackageInfo>,
    package_layers: &BTreeMap<String, PackageLayer>,
) -> Result<Vec<AstViolation>, String> {
    let ident_to_package = package_layers
        .keys()
        .map(|package| (package.replace('-', "_"), package.clone()))
        .collect::<BTreeMap<_, _>>();
    let mut violations = Vec::new();
    for package in packages.values() {
        let source_layer = package_layers.get(&package.name).expect("coverage checked");
        if source_layer.rank > policy.ast_constructor_guard.max_source_rank {
            continue;
        }
        let Some(root) = package.manifest_path.parent() else {
            continue;
        };
        let src_dir = root.join("src");
        if !src_dir.is_dir() {
            continue;
        }
        for file in rust_files(&src_dir)? {
            if is_test_file(&file) {
                continue;
            }
            let text =
                fs::read_to_string(&file).map_err(|error| format!("failed to read {}: {error}", file.display()))?;
            let syntax =
                syn::parse_file(&text).map_err(|error| format!("failed to parse {}: {error}", file.display()))?;
            let mut collector = PathCollector::default();
            collector.visit_file(&syntax);
            for path in collector.paths {
                let Some(first) = path.split("::").next() else {
                    continue;
                };
                let Some(target_package) = ident_to_package.get(first) else {
                    continue;
                };
                if target_package == &package.name {
                    continue;
                }
                let target_layer = package_layers.get(target_package).expect("coverage checked");
                if source_layer.rank < target_layer.rank {
                    violations.push(AstViolation {
                        source_package: package.name.clone(),
                        source_layer: source_layer.layer.clone(),
                        source_file: relative_path(&file),
                        referenced_package: target_package.clone(),
                        referenced_layer: target_layer.layer.clone(),
                        path: path.clone(),
                        message: format!(
                            "{} ({}) references higher-layer path `{}` from {} ({}); {}",
                            package.name,
                            source_layer.layer,
                            path,
                            target_package,
                            target_layer.layer,
                            policy.ast_constructor_guard.description
                        ),
                    });
                }
            }
        }
    }
    Ok(violations)
}

#[derive(Default)]
struct PathCollector {
    paths: BTreeSet<String>,
}

impl<'ast> Visit<'ast> for PathCollector {
    fn visit_item_mod(&mut self, node: &'ast syn::ItemMod) {
        if is_cfg_test(&node.attrs) {
            return;
        }
        syn::visit::visit_item_mod(self, node);
    }

    fn visit_item_fn(&mut self, node: &'ast syn::ItemFn) {
        if is_cfg_test(&node.attrs) {
            return;
        }
        syn::visit::visit_item_fn(self, node);
    }

    fn visit_item_impl(&mut self, node: &'ast syn::ItemImpl) {
        if is_cfg_test(&node.attrs) {
            return;
        }
        syn::visit::visit_item_impl(self, node);
    }

    fn visit_item_struct(&mut self, node: &'ast syn::ItemStruct) {
        if is_cfg_test(&node.attrs) {
            return;
        }
        syn::visit::visit_item_struct(self, node);
    }

    fn visit_item_enum(&mut self, node: &'ast syn::ItemEnum) {
        if is_cfg_test(&node.attrs) {
            return;
        }
        syn::visit::visit_item_enum(self, node);
    }

    fn visit_use_tree(&mut self, node: &'ast syn::UseTree) {
        collect_use_tree_paths(node, String::new(), &mut self.paths);
        syn::visit::visit_use_tree(self, node);
    }

    fn visit_expr_struct(&mut self, node: &'ast syn::ExprStruct) {
        self.paths.insert(path_to_string(&node.path));
        syn::visit::visit_expr_struct(self, node);
    }

    fn visit_expr_call(&mut self, node: &'ast syn::ExprCall) {
        if let syn::Expr::Path(path) = node.func.as_ref() {
            self.paths.insert(path_to_string(&path.path));
        }
        syn::visit::visit_expr_call(self, node);
    }

    fn visit_path(&mut self, node: &'ast syn::Path) {
        self.paths.insert(path_to_string(node));
        syn::visit::visit_path(self, node);
    }
}

fn is_cfg_test(attrs: &[syn::Attribute]) -> bool {
    attrs.iter().any(|attr| {
        if !attr.path().is_ident("cfg") {
            return false;
        }
        let tokens = attr.meta.require_list().map(|list| list.tokens.to_string()).unwrap_or_default();
        tokens.contains("test")
    })
}

fn collect_use_tree_paths(tree: &syn::UseTree, prefix: String, paths: &mut BTreeSet<String>) {
    match tree {
        syn::UseTree::Path(path) => {
            let next = append_segment(prefix, path.ident.to_string());
            collect_use_tree_paths(&path.tree, next, paths);
        }
        syn::UseTree::Name(name) => {
            paths.insert(append_segment(prefix, name.ident.to_string()));
        }
        syn::UseTree::Rename(rename) => {
            paths.insert(append_segment(prefix, rename.ident.to_string()));
        }
        syn::UseTree::Glob(_) => {
            paths.insert(format!("{prefix}::*"));
        }
        syn::UseTree::Group(group) => {
            for item in &group.items {
                collect_use_tree_paths(item, prefix.clone(), paths);
            }
        }
    }
}

fn append_segment(prefix: String, segment: String) -> String {
    if prefix.is_empty() {
        segment
    } else {
        format!("{prefix}::{segment}")
    }
}

fn path_to_string(path: &syn::Path) -> String {
    path.segments.iter().map(|segment| segment.ident.to_string()).collect::<Vec<_>>().join("::")
}

fn rust_files(dir: &Path) -> Result<Vec<PathBuf>, String> {
    let mut files = Vec::new();
    collect_rust_files(dir, &mut files)?;
    files.sort();
    Ok(files)
}

fn collect_rust_files(dir: &Path, files: &mut Vec<PathBuf>) -> Result<(), String> {
    let entries = fs::read_dir(dir).map_err(|error| format!("failed to read {}: {error}", dir.display()))?;
    for entry in entries {
        let entry = entry.map_err(|error| format!("failed to read directory entry in {}: {error}", dir.display()))?;
        let path = entry.path();
        if path.is_dir() {
            collect_rust_files(&path, files)?;
        } else if path.extension().is_some_and(|extension| extension == "rs") {
            files.push(path);
        }
    }
    Ok(())
}

fn is_test_file(path: &Path) -> bool {
    let mut components = path.components().filter_map(|component| component.as_os_str().to_str());
    if components.any(|component| component == "tests") {
        return true;
    }
    path.file_name().and_then(|name| name.to_str()).is_some_and(|name| name.ends_with("_tests.rs"))
}

fn format_diagnostics(dependency_violations: &[LayerViolation], ast_violations: &[AstViolation]) -> String {
    let mut lines = Vec::new();
    for violation in dependency_violations {
        lines.push(format!(
            "dependency violation: {} [{}] -> {} [{}]: {}",
            violation.source, violation.source_layer, violation.target, violation.target_layer, violation.message
        ));
    }
    for violation in ast_violations {
        lines.push(format!(
            "AST violation: {} [{}] {} references {} [{}] via `{}`: {}",
            violation.source_package,
            violation.source_layer,
            violation.source_file.display(),
            violation.referenced_package,
            violation.referenced_layer,
            violation.path,
            violation.message
        ));
    }
    lines.join("\n")
}

fn write_inventory(
    policy: &LayerPolicy,
    packages: &BTreeMap<String, PackageInfo>,
    package_layers: &BTreeMap<String, PackageLayer>,
) -> Result<PathBuf, String> {
    let dependency_edges = packages
        .values()
        .flat_map(|package| {
            let package_layers = package_layers;
            package.dependencies.iter().filter_map(move |dependency| {
                if dependency.kind.as_deref() == Some("dev") {
                    return None;
                }
                let source = package_layers.get(&package.name)?;
                let target = package_layers.get(&dependency.name)?;
                Some(json!({
                    "from": package.name,
                    "from_layer": source.layer,
                    "to": dependency.name,
                    "to_layer": target.layer,
                    "direction": if source.rank < target.rank { "up" } else if source.rank > target.rank { "down" } else { "same" }
                }))
            })
        })
        .collect::<Vec<_>>();
    let packages_json = package_layers
        .values()
        .map(|layer| {
            json!({
                "package": layer.package,
                "layer": layer.layer,
                "rank": layer.rank,
            })
        })
        .collect::<Vec<_>>();
    let receipt = json!({
        "schema": "clankers.workspace_layering.inventory.v1",
        "policy": POLICY_PATH,
        "policy_hash": hash_artifact(Path::new(POLICY_PATH))?,
        "requirements": ["r[remaining-coupling-drain.architecture-rail-hardening.workspace-layer-map]"],
        "workspace_package_count": packages.len(),
        "packages": packages_json,
        "dependency_edges": dependency_edges,
        "allowed_upward_edges": policy.allowed_upward_edges.iter().map(|edge| json!({
            "from": edge.from,
            "to": edge.to,
            "reason": edge.reason,
        })).collect::<Vec<_>>(),
        "ast_constructor_guard": {
            "max_source_rank": policy.ast_constructor_guard.max_source_rank,
            "description": policy.ast_constructor_guard.description,
        },
        "source_artifacts": source_artifacts(),
        "source_hashes": source_artifacts().iter().map(|artifact| hash_artifact(Path::new(artifact))).collect::<Result<Vec<_>, _>>()?,
    });
    let output = PathBuf::from(DEFAULT_OUTPUT);
    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent).map_err(|error| format!("failed to create {}: {error}", parent.display()))?;
    }
    let text =
        serde_json::to_string_pretty(&receipt).map_err(|error| format!("failed to serialize inventory: {error}"))?;
    fs::write(&output, format!("{text}\n"))
        .map_err(|error| format!("failed to write {}: {error}", output.display()))?;
    Ok(output)
}

fn source_artifacts() -> Vec<&'static str> {
    let mut artifacts = vec![POLICY_PATH, WORKSPACE_SPEC];
    if Path::new(CHANGE_TASKS_ACTIVE).is_file() {
        artifacts.push(CHANGE_TASKS_ACTIVE);
    } else if Path::new(CHANGE_TASKS_ARCHIVE).is_file() {
        artifacts.push(CHANGE_TASKS_ARCHIVE);
    }
    if Path::new(CHANGE_SPEC_ACTIVE).is_file() {
        artifacts.push(CHANGE_SPEC_ACTIVE);
    } else if Path::new(CHANGE_SPEC_ARCHIVE).is_file() {
        artifacts.push(CHANGE_SPEC_ARCHIVE);
    }
    artifacts
}

fn hash_artifact(path: &Path) -> Result<Value, String> {
    let bytes = fs::read(path).map_err(|error| format!("failed to hash {}: {error}", path.display()))?;
    Ok(json!({
        "path": path,
        "blake3": blake3_hex(&bytes),
    }))
}

fn blake3_hex(bytes: &[u8]) -> String {
    let hash = blake3::hash(bytes);
    hash.to_hex().to_string()
}

fn relative_path(path: &Path) -> PathBuf {
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    path.strip_prefix(&cwd).unwrap_or(path).to_path_buf()
}
