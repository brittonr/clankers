#!/usr/bin/env -S nix develop -c cargo -q -Zscript
---cargo
[package]
edition = "2024"

[dependencies]
blake3 = "1"
quote = "1"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
syn = { version = "2", features = ["full", "parsing"] }
---

use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::fs;
use std::path::Path;
use std::path::PathBuf;
use std::process::ExitCode;

use quote::ToTokens;
use serde::Deserialize;
use syn::Fields;
use syn::File;
use syn::ImplItem;
use syn::Item;
use syn::TraitItem;
use syn::UseTree;
use syn::Visibility;

const ERROR_EXIT: u8 = 1;
const CRATE_NAME: &str = "clankers-runtime";
const SOURCE_ROOT: &str = "crates/clankers-runtime/src";
const MANIFEST_PATH: &str = "crates/clankers-runtime/Cargo.toml";
const POLICY_PATH: &str = "policy/embedded-lego/runtime-facade-boundary.json";
const INVENTORY_PATH: &str = "docs/src/generated/runtime-facade-api.md";
const WRITE_INVENTORY_ARG: &str = "--write-inventory";
const GENERATED_HEADER: &str = "# Runtime Facade API Boundary";

#[derive(Debug, Deserialize)]
struct Policy {
    schema: String,
    crate_name: String,
    facade_classification: String,
    selected_green_subsets: Vec<String>,
    groups: Vec<ApiGroup>,
    dependency_allowlist: Vec<DependencyRule>,
    forbidden_dependency_fragments: Vec<String>,
    forbidden_source_tokens: Vec<ForbiddenToken>,
}

#[derive(Debug, Deserialize)]
struct ApiGroup {
    id: String,
    owner: String,
    classification: String,
    stability: String,
    source_paths: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct DependencyRule {
    name: String,
    classification: String,
    reason: String,
}

#[derive(Debug, Deserialize)]
struct ForbiddenToken {
    token: String,
    reason: String,
}

#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd)]
struct PublicItemKey {
    entry: String,
    kind: String,
}

#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd)]
struct PublicItem {
    key: PublicItemKey,
    source: PathBuf,
}

#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd)]
struct InventoryRow {
    entry: String,
    kind: String,
    source: String,
    group_id: String,
    owner: String,
    classification: String,
    stability: String,
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => {
            println!("ok: runtime facade boundary inventories {CRATE_NAME} public API and dependency classifications");
            ExitCode::SUCCESS
        }
        Err(errors) => {
            for error in errors {
                eprintln!("runtime facade boundary error: {error}");
            }
            ExitCode::from(ERROR_EXIT)
        }
    }
}

fn run() -> Result<(), Vec<String>> {
    let write_inventory = std::env::args().skip(1).any(|arg| arg == WRITE_INVENTORY_ARG);
    let policy = read_policy()?;
    let mut errors = Vec::new();

    validate_policy_header(&policy, &mut errors);
    let groups_by_path = groups_by_source_path(&policy, &mut errors);
    let items = scan_public_items(Path::new(SOURCE_ROOT)).map_err(|error| vec![error])?;
    let rows = classify_items(items, &groups_by_path, &mut errors);
    validate_dependencies(&policy, &mut errors);
    validate_forbidden_source_tokens(&policy, &mut errors);

    let inventory = render_inventory(&policy, &rows);
    if write_inventory {
        fs::write(INVENTORY_PATH, inventory)
            .map_err(|error| vec![format!("failed to write {INVENTORY_PATH}: {error}")])?;
    } else {
        let current = fs::read_to_string(INVENTORY_PATH).map_err(|error| {
            vec![format!(
                "failed to read {INVENTORY_PATH}: {error}; run {POLICY_PATH} rail with {WRITE_INVENTORY_ARG}"
            )]
        })?;
        if current != inventory {
            errors.push(format!(
                "{INVENTORY_PATH} is stale; run scripts/check-runtime-facade-boundary.rs {WRITE_INVENTORY_ARG}"
            ));
        }
    }

    if errors.is_empty() { Ok(()) } else { Err(errors) }
}

fn read_policy() -> Result<Policy, Vec<String>> {
    let text =
        fs::read_to_string(POLICY_PATH).map_err(|error| vec![format!("failed to read {POLICY_PATH}: {error}")])?;
    serde_json::from_str(&text).map_err(|error| vec![format!("failed to parse {POLICY_PATH}: {error}")])
}

fn validate_policy_header(policy: &Policy, errors: &mut Vec<String>) {
    if policy.schema != "clankers.runtime_facade_boundary.v1" {
        errors.push(format!("{POLICY_PATH} has unsupported schema `{}`", policy.schema));
    }
    if policy.crate_name != CRATE_NAME {
        errors.push(format!("{POLICY_PATH} crate_name must be `{CRATE_NAME}`, got `{}`", policy.crate_name));
    }
    if policy.facade_classification.trim().is_empty() {
        errors.push(format!("{POLICY_PATH} facade_classification must not be empty"));
    }
    if policy.selected_green_subsets.is_empty() {
        errors.push(format!("{POLICY_PATH} selected_green_subsets must name reviewed green subsets or `none`"));
    }
}

fn groups_by_source_path<'a>(policy: &'a Policy, errors: &mut Vec<String>) -> BTreeMap<String, &'a ApiGroup> {
    let mut groups = BTreeMap::new();
    let mut ids = BTreeSet::new();
    for group in &policy.groups {
        if !ids.insert(group.id.clone()) {
            errors.push(format!("duplicate runtime API group `{}`", group.id));
        }
        if group.owner.trim().is_empty() || group.classification.trim().is_empty() || group.stability.trim().is_empty()
        {
            errors.push(format!("runtime API group `{}` has an empty owner/classification/stability", group.id));
        }
        for source in &group.source_paths {
            if !Path::new(source).is_file() {
                errors.push(format!("runtime API group `{}` references missing source `{source}`", group.id));
            }
            if let Some(previous) = groups.insert(source.clone(), group) {
                errors.push(format!(
                    "runtime source `{source}` is assigned to both `{}` and `{}`",
                    previous.id, group.id
                ));
            }
        }
    }
    groups
}

fn scan_public_items(root: &Path) -> Result<Vec<PublicItem>, String> {
    let mut rows = BTreeSet::new();
    for source in collect_rust_files(root)? {
        let text =
            fs::read_to_string(&source).map_err(|error| format!("failed to read {}: {error}", source.display()))?;
        let parsed =
            syn::parse_file(&text).map_err(|error| format!("failed to parse {}: {error}", source.display()))?;
        for item in scan_parsed_file(&source, &parsed) {
            rows.insert((item.source, item.key.entry, item.key.kind));
        }
    }
    Ok(rows
        .into_iter()
        .map(|(source, entry, kind)| PublicItem {
            source,
            key: PublicItemKey { entry, kind },
        })
        .collect())
}

fn collect_rust_files(root: &Path) -> Result<Vec<PathBuf>, String> {
    let mut files = Vec::new();
    collect_rust_files_rec(root, &mut files)?;
    files.sort();
    Ok(files)
}

fn collect_rust_files_rec(path: &Path, files: &mut Vec<PathBuf>) -> Result<(), String> {
    if path.is_file() {
        if path.extension().and_then(|ext| ext.to_str()) == Some("rs") {
            files.push(path.to_path_buf());
        }
        return Ok(());
    }
    let entries = fs::read_dir(path).map_err(|error| format!("failed to read dir {}: {error}", path.display()))?;
    for entry in entries {
        let entry = entry.map_err(|error| format!("failed to read dir entry under {}: {error}", path.display()))?;
        collect_rust_files_rec(&entry.path(), files)?;
    }
    Ok(())
}

fn scan_parsed_file(source: &Path, parsed: &File) -> Vec<PublicItem> {
    let mut items = Vec::new();
    let public_types = public_type_names(parsed);
    for item in &parsed.items {
        if has_test_cfg(attrs_for_item(item)) {
            continue;
        }
        match item {
            Item::Const(item) if is_public(&item.vis) => push_item(&mut items, source, &item.ident, "constant"),
            Item::Enum(item) if is_public(&item.vis) => push_item(&mut items, source, &item.ident, "enum"),
            Item::Fn(item) if is_public(&item.vis) => push_item(&mut items, source, &item.sig.ident, "function"),
            Item::Mod(item) if is_public(&item.vis) => push_item(&mut items, source, &item.ident, "module"),
            Item::Struct(item) if is_public(&item.vis) => {
                let owner = item.ident.to_string();
                push_item(&mut items, source, &item.ident, "struct");
                collect_public_fields(&mut items, source, &owner, &item.fields);
            }
            Item::Trait(item) if is_public(&item.vis) => {
                let owner = item.ident.to_string();
                push_item(&mut items, source, &item.ident, "trait");
                collect_trait_methods(&mut items, source, &owner, item);
            }
            Item::Type(item) if is_public(&item.vis) => push_item(&mut items, source, &item.ident, "type"),
            Item::Use(item) if is_crate_root(source) && is_public(&item.vis) => {
                collect_reexports(&mut items, source, &item.tree)
            }
            Item::Impl(item) => collect_public_impl_methods(&mut items, source, &public_types, item),
            _ => {}
        }
    }
    items
}

fn attrs_for_item(item: &Item) -> &[syn::Attribute] {
    match item {
        Item::Const(item) => &item.attrs,
        Item::Enum(item) => &item.attrs,
        Item::Fn(item) => &item.attrs,
        Item::Impl(item) => &item.attrs,
        Item::Mod(item) => &item.attrs,
        Item::Struct(item) => &item.attrs,
        Item::Trait(item) => &item.attrs,
        Item::Type(item) => &item.attrs,
        Item::Use(item) => &item.attrs,
        _ => &[],
    }
}

fn public_type_names(parsed: &File) -> BTreeSet<String> {
    let mut names = BTreeSet::new();
    for item in &parsed.items {
        if has_test_cfg(attrs_for_item(item)) {
            continue;
        }
        match item {
            Item::Enum(item) if is_public(&item.vis) => {
                names.insert(item.ident.to_string());
            }
            Item::Struct(item) if is_public(&item.vis) => {
                names.insert(item.ident.to_string());
            }
            Item::Trait(item) if is_public(&item.vis) => {
                names.insert(item.ident.to_string());
            }
            _ => {}
        }
    }
    names
}

fn is_public(vis: &Visibility) -> bool {
    matches!(vis, Visibility::Public(_))
}

fn is_crate_root(path: &Path) -> bool {
    path.file_name().and_then(|name| name.to_str()) == Some("lib.rs")
}

fn has_test_cfg(attrs: &[syn::Attribute]) -> bool {
    attrs.iter().any(|attr| {
        let path = attr.path();
        (path.is_ident("cfg") || path.is_ident("cfg_attr")) && attr.meta.to_token_stream().to_string().contains("test")
    })
}

fn push_item(items: &mut Vec<PublicItem>, source: &Path, ident: &syn::Ident, kind: &str) {
    items.push(PublicItem {
        key: PublicItemKey {
            entry: ident.to_string(),
            kind: kind.to_string(),
        },
        source: source.to_path_buf(),
    });
}

fn push_qualified(items: &mut Vec<PublicItem>, source: &Path, owner: &str, name: &str, kind: &str) {
    items.push(PublicItem {
        key: PublicItemKey {
            entry: format!("{owner}::{name}"),
            kind: kind.to_string(),
        },
        source: source.to_path_buf(),
    });
}

fn collect_public_fields(items: &mut Vec<PublicItem>, source: &Path, owner: &str, fields: &Fields) {
    for field in fields {
        if is_public(&field.vis) {
            if let Some(ident) = &field.ident {
                push_qualified(items, source, owner, &ident.to_string(), "field");
            }
        }
    }
}

fn collect_trait_methods(items: &mut Vec<PublicItem>, source: &Path, owner: &str, item: &syn::ItemTrait) {
    for trait_item in &item.items {
        if let TraitItem::Fn(method) = trait_item {
            push_qualified(items, source, owner, &method.sig.ident.to_string(), "method");
        }
    }
}

fn collect_public_impl_methods(
    items: &mut Vec<PublicItem>,
    source: &Path,
    public_types: &BTreeSet<String>,
    item: &syn::ItemImpl,
) {
    if item.trait_.is_some() {
        return;
    }
    let Some(owner) = impl_owner_name(&item.self_ty) else {
        return;
    };
    if !public_types.contains(&owner) {
        return;
    }
    for impl_item in &item.items {
        if let ImplItem::Fn(method) = impl_item {
            if is_public(&method.vis) {
                push_qualified(items, source, &owner, &method.sig.ident.to_string(), "method");
            }
        }
    }
}

fn impl_owner_name(ty: &syn::Type) -> Option<String> {
    match ty {
        syn::Type::Path(path) => path.path.segments.last().map(|segment| segment.ident.to_string()),
        _ => None,
    }
}

fn collect_reexports(items: &mut Vec<PublicItem>, source: &Path, tree: &UseTree) {
    match tree {
        UseTree::Path(path) => collect_reexports(items, source, &path.tree),
        UseTree::Name(name) => push_item(items, source, &name.ident, "reexport"),
        UseTree::Rename(rename) => push_item(items, source, &rename.rename, "reexport"),
        UseTree::Group(group) => {
            for item in &group.items {
                collect_reexports(items, source, item);
            }
        }
        UseTree::Glob(_) => {
            items.push(PublicItem {
                key: PublicItemKey {
                    entry: "*".to_string(),
                    kind: "reexport".to_string(),
                },
                source: source.to_path_buf(),
            });
        }
    }
}

fn classify_items(
    items: Vec<PublicItem>,
    groups_by_path: &BTreeMap<String, &ApiGroup>,
    errors: &mut Vec<String>,
) -> Vec<InventoryRow> {
    let mut rows = Vec::new();
    for item in items {
        let source = normalize_path(&item.source);
        let Some(group) = groups_by_path.get(&source) else {
            errors.push(format!(
                "public runtime item `{}` ({}) in `{source}` has no API group",
                item.key.entry, item.key.kind
            ));
            continue;
        };
        rows.push(InventoryRow {
            entry: item.key.entry,
            kind: item.key.kind,
            source,
            group_id: group.id.clone(),
            owner: group.owner.clone(),
            classification: group.classification.clone(),
            stability: group.stability.clone(),
        });
    }
    rows.sort();
    rows
}

fn normalize_path(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

fn validate_dependencies(policy: &Policy, errors: &mut Vec<String>) {
    let manifest = match fs::read_to_string(MANIFEST_PATH) {
        Ok(text) => text,
        Err(error) => {
            errors.push(format!("failed to read {MANIFEST_PATH}: {error}"));
            return;
        }
    };
    for fragment in &policy.forbidden_dependency_fragments {
        if manifest.contains(fragment) {
            errors.push(format!("{MANIFEST_PATH} contains forbidden dependency fragment `{fragment}`"));
        }
    }
    let allowed = policy
        .dependency_allowlist
        .iter()
        .map(|rule| (rule.name.as_str(), rule))
        .collect::<BTreeMap<_, _>>();
    let deps = parse_dependencies(&manifest);
    for dep in deps {
        match allowed.get(dep.as_str()) {
            Some(rule) => {
                if rule.classification.trim().is_empty() || rule.reason.trim().is_empty() {
                    errors.push(format!("dependency allowlist entry `{dep}` must have classification and reason"));
                }
            }
            None => errors.push(format!("dependency `{dep}` is not classified in {POLICY_PATH}")),
        }
    }
}

fn parse_dependencies(manifest: &str) -> Vec<String> {
    let mut in_dependencies = false;
    let mut deps = BTreeSet::new();
    for line in manifest.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') {
            in_dependencies = trimmed == "[dependencies]";
            continue;
        }
        if !in_dependencies || trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let Some((name, _)) = trimmed.split_once('=') else {
            continue;
        };
        let dep = name.trim();
        if !dep.is_empty() {
            deps.insert(dep.to_string());
        }
    }
    deps.into_iter().collect()
}

fn validate_forbidden_source_tokens(policy: &Policy, errors: &mut Vec<String>) {
    let files = match collect_rust_files(Path::new(SOURCE_ROOT)) {
        Ok(files) => files,
        Err(error) => {
            errors.push(error);
            return;
        }
    };
    for file in files {
        let path = normalize_path(&file);
        let source = match fs::read_to_string(&file) {
            Ok(source) => source,
            Err(error) => {
                errors.push(format!("failed to read {path}: {error}"));
                continue;
            }
        };
        for token in &policy.forbidden_source_tokens {
            if source.contains(&token.token) {
                errors.push(format!(
                    "{path} contains forbidden runtime source token `{}` ({})",
                    token.token, token.reason
                ));
            }
        }
    }
}

fn render_inventory(policy: &Policy, rows: &[InventoryRow]) -> String {
    let mut out = String::new();
    out.push_str(GENERATED_HEADER);
    out.push_str("\n\n");
    out.push_str("Generated by `scripts/check-runtime-facade-boundary.rs --write-inventory`. Do not edit by hand.\n\n");
    out.push_str(&format!("- Crate: `{}`\n", policy.crate_name));
    out.push_str(&format!("- Facade classification: {}\n", policy.facade_classification));
    out.push_str(&format!("- Selected green subsets: {}\n", policy.selected_green_subsets.join(", ")));
    out.push_str(&format!("- Public inventory rows: {}\n", rows.len()));
    out.push_str(&format!("- Inventory hash: `{}`\n\n", inventory_hash(rows)));

    out.push_str("## API groups\n\n");
    out.push_str("| Group | Owner | Classification | Stability | Source paths | Public rows |\n");
    out.push_str("|---|---|---|---|---|---|\n");
    for group in &policy.groups {
        let count = rows.iter().filter(|row| row.group_id == group.id).count();
        out.push_str(&format!(
            "| `{}` | `{}` | {} | {} | {} | {} |\n",
            escape_md(&group.id),
            escape_md(&group.owner),
            escape_md(&group.classification),
            escape_md(&group.stability),
            group
                .source_paths
                .iter()
                .map(|path| format!("`{}`", escape_md(path)))
                .collect::<Vec<_>>()
                .join("<br>"),
            count
        ));
    }

    out.push_str("\n## Dependencies\n\n");
    out.push_str("| Dependency | Classification | Reason |\n");
    out.push_str("|---|---|---|\n");
    for dep in &policy.dependency_allowlist {
        out.push_str(&format!(
            "| `{}` | {} | {} |\n",
            escape_md(&dep.name),
            escape_md(&dep.classification),
            escape_md(&dep.reason)
        ));
    }

    out.push_str("\n## Public exports\n\n");
    out.push_str("| Item | Kind | Source | Group | Owner | Classification | Stability |\n");
    out.push_str("|---|---|---|---|---|---|---|\n");
    for row in rows {
        out.push_str(&format!(
            "| `{}` | {} | `{}` | `{}` | `{}` | {} | {} |\n",
            escape_md(&row.entry),
            escape_md(&row.kind),
            escape_md(&row.source),
            escape_md(&row.group_id),
            escape_md(&row.owner),
            escape_md(&row.classification),
            escape_md(&row.stability)
        ));
    }
    out
}

fn inventory_hash(rows: &[InventoryRow]) -> String {
    let mut hasher = blake3::Hasher::new();
    for row in rows {
        hasher.update(row.entry.as_bytes());
        hasher.update(b"\0");
        hasher.update(row.kind.as_bytes());
        hasher.update(b"\0");
        hasher.update(row.source.as_bytes());
        hasher.update(b"\0");
        hasher.update(row.group_id.as_bytes());
        hasher.update(b"\0");
        hasher.update(row.owner.as_bytes());
        hasher.update(b"\0");
        hasher.update(row.classification.as_bytes());
        hasher.update(b"\0");
        hasher.update(row.stability.as_bytes());
        hasher.update(b"\n");
    }
    hasher.finalize().to_hex().to_string()
}

fn escape_md(text: &str) -> String {
    text.replace('|', "\\|")
}
