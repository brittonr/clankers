#!/usr/bin/env -S nix develop -c cargo -q -Zscript
---cargo
[package]
edition = "2024"

[dependencies]
quote = "1"
syn = { version = "2", features = ["full", "parsing"] }
---

use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::fs;
use std::path::Path;
use std::path::PathBuf;

use quote::ToTokens;
use syn::Fields;
use syn::File;
use syn::ImplItem;
use syn::Item;
use syn::TraitItem;
use syn::Type;
use syn::UseTree;
use syn::Visibility;

const GUIDE_PATH: &str = "docs/src/tutorials/embedded-agent-sdk.md";
const INVENTORY_PATH: &str = "docs/src/generated/embedded-sdk-api.md";
const INVENTORY_GUIDE_LINK: &str = "../generated/embedded-sdk-api.md";
const EXPECTED_COLUMN_COUNT: usize = 5;
const ENTRY_CELL_PREFIX: &str = "`";
const CODE_SPAN_DELIMITER: char = '`';
const TABLE_SEPARATOR_PREFIX: &str = "---";
const EXAMPLE_KIND: &str = "example";
const FIELD_KIND: &str = "field";
const METHOD_KIND: &str = "method";
const REEXPORT_KIND: &str = "reexport";
const WRITE_INVENTORY_ARG: &str = "--write-inventory";
const GENERATE_INVENTORY_ARG: &str = "--generate-inventory";
const ERROR_EXIT: i32 = 1;

const SOURCE_ROOTS: &[SourceRoot] = &[
    SourceRoot {
        crate_name: "clankers-engine",
        path: "crates/clankers-engine/src",
    },
    SourceRoot {
        crate_name: "clankers-engine-host",
        path: "crates/clankers-engine-host/src",
    },
    SourceRoot {
        crate_name: "clankers-tool-host",
        path: "crates/clankers-tool-host/src",
    },
    SourceRoot {
        crate_name: "clanker-message",
        path: "crates/clanker-message/src",
    },
    SourceRoot {
        crate_name: "clankers-adapters",
        path: "crates/clankers-adapters/src",
    },
];

const CRATE_ORDER: &[&str] = &[
    "clankers-engine",
    "example",
    "clankers-adapters",
    "clankers-engine-host",
    "clankers-tool-host",
    "clanker-message",
];

const KIND_ORDER: &[&str] = &[
    "constant",
    "module",
    REEXPORT_KIND,
    "type",
    "struct",
    "enum",
    "trait",
    "function",
    FIELD_KIND,
    METHOD_KIND,
    EXAMPLE_KIND,
];

const VALID_STABILITIES: &[&str] = &[
    "supported",
    "optional-support",
    "compatibility-alias",
    "experimental",
    "unsupported-internal",
];

#[derive(Debug, Clone, Copy)]
struct SourceRoot {
    crate_name: &'static str,
    path: &'static str,
}

#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd)]
struct PublicItemKey {
    entry: String,
    kind: String,
}

#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd)]
struct PublicItemRef {
    key: PublicItemKey,
    source: PathBuf,
}

#[derive(Debug, Clone, Eq, PartialEq)]
struct PublicItem {
    key: PublicItemKey,
    crate_name: String,
    source: PathBuf,
}

#[derive(Debug, Clone, Eq, PartialEq)]
struct InventoryEntry {
    key: PublicItemKey,
    crate_name: String,
    stability: String,
    source: PathBuf,
}

#[derive(Debug)]
struct CheckReport {
    errors: Vec<String>,
    inventory_count: usize,
    scanned_count: usize,
}

fn read_text(path: &Path) -> Result<String, String> {
    fs::read_to_string(path).map_err(|error| format!("failed to read {}: {error}", path.display()))
}

fn parse_inventory(text: &str) -> Result<Vec<InventoryEntry>, Vec<String>> {
    let mut entries = Vec::new();
    let mut errors = Vec::new();

    for (line_index, line) in text.lines().enumerate() {
        let trimmed = line.trim();
        if !trimmed.starts_with('|') || !trimmed.contains(ENTRY_CELL_PREFIX) {
            continue;
        }

        let cells: Vec<String> = trimmed
            .trim_matches('|')
            .split('|')
            .map(|cell| cell.trim().to_string())
            .collect();

        if cells.len() != EXPECTED_COLUMN_COUNT {
            errors.push(format!(
                "{}:{} expected {EXPECTED_COLUMN_COUNT} table columns, found {}",
                INVENTORY_PATH,
                line_index + 1,
                cells.len()
            ));
            continue;
        }

        if cells[0].starts_with(TABLE_SEPARATOR_PREFIX) {
            continue;
        }

        let Some(entry) = extract_code_span(&cells[0]) else {
            continue;
        };
        let crate_name = strip_code_span(&cells[1]);
        let kind = cells[2].trim().to_string();
        let stability = cells[3].trim().to_string();
        let source = strip_code_span(&cells[4]);

        entries.push(InventoryEntry {
            key: PublicItemKey { entry, kind },
            crate_name,
            stability,
            source: PathBuf::from(source),
        });
    }

    if entries.is_empty() {
        errors.push(format!("{INVENTORY_PATH} contains no inventory rows"));
    }

    if errors.is_empty() { Ok(entries) } else { Err(errors) }
}

fn extract_code_span(cell: &str) -> Option<String> {
    let start = cell.find(CODE_SPAN_DELIMITER)?;
    let end = cell[start + 1..].find(CODE_SPAN_DELIMITER)? + start + 1;
    Some(cell[start + 1..end].to_string())
}

fn strip_code_span(cell: &str) -> String {
    extract_code_span(cell).unwrap_or_else(|| cell.trim().to_string())
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

fn scan_public_items(roots: &[SourceRoot]) -> Result<Vec<PublicItem>, String> {
    let mut items = BTreeSet::new();
    for root in roots {
        let root_path = Path::new(root.path);
        for file in collect_rust_files(root_path)? {
            let text = read_text(&file)?;
            let parsed = syn::parse_file(&text).map_err(|error| format!("failed to parse {}: {error}", file.display()))?;
            for item in scan_parsed_file(&file, root.crate_name, &parsed) {
                items.insert((item.crate_name, item.source, item.key.entry, item.key.kind));
            }
        }
    }
    Ok(items
        .into_iter()
        .map(|(crate_name, source, entry, kind)| PublicItem {
            key: PublicItemKey { entry, kind },
            crate_name,
            source,
        })
        .collect())
}

fn scan_parsed_file(source: &Path, crate_name: &str, parsed: &File) -> Vec<PublicItem> {
    let mut items = Vec::new();
    let public_types = public_type_names(parsed);
    let source_path = source.to_path_buf();

    for item in &parsed.items {
        if has_test_cfg(attrs_for_item(item)) {
            continue;
        }
        match item {
            Item::Const(item) if is_public(&item.vis) => push_item(&mut items, crate_name, &source_path, &item.ident, "constant"),
            Item::Enum(item) if is_public(&item.vis) => push_item(&mut items, crate_name, &source_path, &item.ident, "enum"),
            Item::Fn(item) if is_public(&item.vis) => push_item(&mut items, crate_name, &source_path, &item.sig.ident, "function"),
            Item::Mod(item) if is_public(&item.vis) => push_item(&mut items, crate_name, &source_path, &item.ident, "module"),
            Item::Struct(item) if is_public(&item.vis) => {
                let owner = item.ident.to_string();
                push_item(&mut items, crate_name, &source_path, &item.ident, "struct");
                collect_public_fields(&mut items, crate_name, &source_path, &owner, &item.fields);
            }
            Item::Trait(item) if is_public(&item.vis) => {
                let owner = item.ident.to_string();
                push_item(&mut items, crate_name, &source_path, &item.ident, "trait");
                collect_trait_methods(&mut items, crate_name, &source_path, &owner, item);
            }
            Item::Type(item) if is_public(&item.vis) => push_item(&mut items, crate_name, &source_path, &item.ident, "type"),
            Item::Use(item) if is_crate_root(source) && is_public(&item.vis) => {
                collect_reexports(&mut items, crate_name, &source_path, &item.tree)
            }
            Item::Impl(item) => collect_public_impl_methods(
                &mut items,
                crate_name,
                &source_path,
                &public_types,
                item,
            ),
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

fn push_item(items: &mut Vec<PublicItem>, crate_name: &str, source: &Path, ident: &syn::Ident, kind: &str) {
    items.push(PublicItem {
        key: PublicItemKey {
            entry: ident.to_string(),
            kind: kind.to_string(),
        },
        crate_name: crate_name.to_string(),
        source: source.to_path_buf(),
    });
}

fn push_qualified(items: &mut Vec<PublicItem>, crate_name: &str, source: &Path, owner: &str, name: &str, kind: &str) {
    items.push(PublicItem {
        key: PublicItemKey {
            entry: format!("{owner}::{name}"),
            kind: kind.to_string(),
        },
        crate_name: crate_name.to_string(),
        source: source.to_path_buf(),
    });
}

fn collect_public_fields(items: &mut Vec<PublicItem>, crate_name: &str, source: &Path, owner: &str, fields: &Fields) {
    match fields {
        Fields::Named(fields) => {
            for field in &fields.named {
                if is_public(&field.vis)
                    && !has_test_cfg(&field.attrs)
                    && let Some(ident) = &field.ident
                {
                    push_qualified(items, crate_name, source, owner, &ident.to_string(), FIELD_KIND);
                }
            }
        }
        Fields::Unnamed(fields) => {
            for (index, field) in fields.unnamed.iter().enumerate() {
                if is_public(&field.vis) && !has_test_cfg(&field.attrs) {
                    push_qualified(items, crate_name, source, owner, &index.to_string(), FIELD_KIND);
                }
            }
        }
        Fields::Unit => {}
    }
}

fn collect_trait_methods(
    items: &mut Vec<PublicItem>,
    crate_name: &str,
    source: &Path,
    owner: &str,
    item: &syn::ItemTrait,
) {
    for trait_item in &item.items {
        if let TraitItem::Fn(method) = trait_item
            && !has_test_cfg(&method.attrs)
        {
            push_qualified(items, crate_name, source, owner, &method.sig.ident.to_string(), METHOD_KIND);
        }
    }
}

fn collect_public_impl_methods(
    items: &mut Vec<PublicItem>,
    crate_name: &str,
    source: &Path,
    public_types: &BTreeSet<String>,
    item: &syn::ItemImpl,
) {
    if has_test_cfg(&item.attrs) || item.trait_.is_some() {
        return;
    }
    let Some(owner) = impl_owner_name(&item.self_ty) else {
        return;
    };
    if !public_types.contains(&owner) {
        return;
    }
    for impl_item in &item.items {
        if let ImplItem::Fn(method) = impl_item
            && is_public(&method.vis)
            && !has_test_cfg(&method.attrs)
        {
            push_qualified(items, crate_name, source, &owner, &method.sig.ident.to_string(), METHOD_KIND);
        }
    }
}

fn impl_owner_name(self_ty: &Type) -> Option<String> {
    match self_ty {
        Type::Path(path) => path.path.segments.last().map(|segment| segment.ident.to_string()),
        _ => None,
    }
}

fn collect_reexports(items: &mut Vec<PublicItem>, crate_name: &str, source: &Path, tree: &UseTree) {
    for name in reexport_leaf_names(tree) {
        items.push(PublicItem {
            key: PublicItemKey {
                entry: name,
                kind: REEXPORT_KIND.to_string(),
            },
            crate_name: crate_name.to_string(),
            source: source.to_path_buf(),
        });
    }
}

fn reexport_leaf_names(tree: &UseTree) -> Vec<String> {
    match tree {
        UseTree::Name(name) => vec![name.ident.to_string()],
        UseTree::Rename(rename) => vec![rename.rename.to_string()],
        UseTree::Path(path) => reexport_leaf_names(&path.tree),
        UseTree::Group(group) => group.items.iter().flat_map(reexport_leaf_names).collect(),
        UseTree::Glob(_) => vec!["*".to_string()],
    }
}

fn item_ref(item: &PublicItem) -> PublicItemRef {
    PublicItemRef {
        key: item.key.clone(),
        source: item.source.clone(),
    }
}

fn inventory_ref(entry: &InventoryEntry) -> PublicItemRef {
    PublicItemRef {
        key: entry.key.clone(),
        source: entry.source.clone(),
    }
}

fn scanned_ref_set(items: &[PublicItem]) -> BTreeSet<PublicItemRef> {
    items.iter().map(item_ref).collect()
}

fn inventory_ref_set(entries: &[InventoryEntry]) -> BTreeSet<PublicItemRef> {
    entries.iter().map(inventory_ref).collect()
}

fn scanned_crate_by_ref(items: &[PublicItem]) -> BTreeMap<PublicItemRef, String> {
    items
        .iter()
        .map(|item| (item_ref(item), item.crate_name.clone()))
        .collect()
}

fn validate_inventory(entries: &[InventoryEntry], scanned: &[PublicItem], guide: &str) -> CheckReport {
    let mut errors = Vec::new();
    let scanned_refs = scanned_ref_set(scanned);
    let inventory_refs = inventory_ref_set(entries);
    let crate_by_ref = scanned_crate_by_ref(scanned);

    if !guide.contains(INVENTORY_GUIDE_LINK) {
        errors.push(format!(
            "{GUIDE_PATH} must link to {INVENTORY_GUIDE_LINK} so SDK entrypoints resolve to inventory"
        ));
    }

    let valid_stabilities: BTreeSet<&str> = VALID_STABILITIES.iter().copied().collect();
    let mut seen_rows = BTreeSet::new();
    for entry in entries {
        let row_key = format!(
            "{}|{}|{}|{}",
            entry.key.entry,
            entry.key.kind,
            entry.crate_name,
            entry.source.display()
        );
        if !seen_rows.insert(row_key) {
            errors.push(format!(
                "duplicate inventory row for `{}` ({}) from {}",
                entry.key.entry,
                entry.key.kind,
                entry.source.display()
            ));
        }

        if !valid_stabilities.contains(entry.stability.as_str()) {
            errors.push(format!(
                "invalid stability `{}` for `{}`; expected one of {:?}",
                entry.stability, entry.key.entry, VALID_STABILITIES
            ));
        }

        if !entry.source.exists() {
            errors.push(format!(
                "source path for `{}` does not exist: {}",
                entry.key.entry,
                entry.source.display()
            ));
            continue;
        }

        if entry.key.kind == EXAMPLE_KIND {
            continue;
        }

        let reference = inventory_ref(entry);
        if !scanned_refs.contains(&reference) {
            errors.push(format!(
                "inventory entry `{}` ({}) does not match any scanned public item in {}; update the inventory owner/source row, hide the item, move it to an app-edge module, or update migration notes and policy if stability intentionally changed",
                entry.key.entry,
                entry.key.kind,
                entry.source.display()
            ));
            continue;
        }

        if let Some(scanned_crate) = crate_by_ref.get(&reference)
            && scanned_crate != &entry.crate_name
        {
            errors.push(format!(
                "inventory maps `{}` ({}) to crate `{}`, but scanner found crate `{}`; update the inventory owner row or move the item to the intended owner module",
                entry.key.entry, entry.key.kind, entry.crate_name, scanned_crate
            ));
        }
    }

    for item in scanned {
        if !inventory_refs.contains(&item_ref(item)) {
            errors.push(format!(
                "public SDK item missing from inventory: `{}` ({}) in {}; add an inventory row, hide the item, move it to an app-edge module, or update migration notes and policy if it becomes stable API",
                item.key.entry,
                item.key.kind,
                item.source.display()
            ));
        }
    }

    CheckReport {
        errors,
        inventory_count: entries.len(),
        scanned_count: scanned.len(),
    }
}

fn generate_inventory(existing: &[InventoryEntry], scanned: &[PublicItem]) -> Vec<InventoryEntry> {
    let existing_by_ref: BTreeMap<PublicItemRef, &InventoryEntry> = existing.iter().map(|entry| (inventory_ref(entry), entry)).collect();
    let owner_stability = owner_stability_index(existing);
    let examples = existing.iter().filter(|entry| entry.key.kind == EXAMPLE_KIND).cloned();
    let scanned_entries = scanned.iter().map(|item| {
        let reference = item_ref(item);
        if let Some(entry) = existing_by_ref.get(&reference) {
            return (*entry).clone();
        }
        InventoryEntry {
            key: item.key.clone(),
            crate_name: item.crate_name.clone(),
            stability: derived_stability(item, &owner_stability),
            source: item.source.clone(),
        }
    });

    let mut generated: Vec<InventoryEntry> = scanned_entries.chain(examples).collect();
    generated.sort_by(compare_inventory_entries);
    generated
}

fn owner_stability_index(existing: &[InventoryEntry]) -> BTreeMap<(String, String), String> {
    let mut index = BTreeMap::new();
    for entry in existing {
        if matches!(entry.key.kind.as_str(), "constant" | "enum" | "function" | "module" | "struct" | "trait" | "type") {
            index
                .entry((entry.crate_name.clone(), entry.key.entry.clone()))
                .or_insert_with(|| entry.stability.clone());
        }
    }
    index
}

fn derived_stability(item: &PublicItem, owner_stability: &BTreeMap<(String, String), String>) -> String {
    if item.key.kind == FIELD_KIND || item.key.kind == METHOD_KIND {
        if let Some(owner) = item.key.entry.split("::").next()
            && let Some(stability) = owner_stability.get(&(item.crate_name.clone(), owner.to_string()))
        {
            return stability.clone();
        }
    }

    if item.key.kind == REEXPORT_KIND
        && let Some(stability) = owner_stability.get(&(item.crate_name.clone(), item.key.entry.clone()))
    {
        return stability.clone();
    }

    "experimental".to_string()
}

fn compare_inventory_entries(left: &InventoryEntry, right: &InventoryEntry) -> std::cmp::Ordering {
    crate_rank(&left.crate_name)
        .cmp(&crate_rank(&right.crate_name))
        .then_with(|| left.source.cmp(&right.source))
        .then_with(|| kind_rank(&left.key.kind).cmp(&kind_rank(&right.key.kind)))
        .then_with(|| left.key.entry.cmp(&right.key.entry))
}

fn crate_rank(crate_name: &str) -> usize {
    CRATE_ORDER
        .iter()
        .position(|candidate| *candidate == crate_name)
        .unwrap_or(CRATE_ORDER.len())
}

fn kind_rank(kind: &str) -> usize {
    KIND_ORDER
        .iter()
        .position(|candidate| *candidate == kind)
        .unwrap_or(KIND_ORDER.len())
}

fn render_inventory(entries: &[InventoryEntry]) -> String {
    let mut output = String::new();
    output.push_str("<!-- This file is checked by `scripts/check-embedded-sdk-api.rs`. Keep support labels intentional. -->\n\n");
    output.push_str("<div class=\"generated-warning\">\n");
    output.push_str("⚡ Checked embedded SDK API inventory. Run <code>scripts/check-embedded-sdk-api.rs</code> to verify source mappings and support labels.\n");
    output.push_str("</div>\n\n");
    output.push_str("# Embedded SDK API Inventory\n\n");
    output.push_str("This inventory defines the documented embedded-agent SDK surface for `clankers-engine`, `clankers-engine-host`, `clankers-tool-host`, `clankers-adapters`, and the shared message contracts that those crates use.\n\n");
    output.push_str("Support labels:\n\n");
    output.push_str("- `supported` — stable embedding entrypoint for the current Clankers crate version line. Removing, renaming, or repurposing it requires an explicit migration note.\n");
    output.push_str("- `optional-support` — supported when a host intentionally opts into the companion concern, such as prompt lifecycle or provider-neutral streaming contracts.\n");
    output.push_str("- `compatibility-alias` — supported compatibility shim for an older brick name; it must name the canonical replacement in migration notes before removal.\n");
    output.push_str("- `experimental` — public but not yet promised as stable embedding API.\n");
    output.push_str("- `unsupported-internal` — public because of current crate layout or tests, but not an advertised stable embedded SDK entrypoint.\n\n");
    output.push_str("Inventory kinds additionally include `field`, `method`, and `reexport` rows so embedders can see typed public members and root convenience aliases, not just top-level declarations.\n\n");
    output.push_str("## Inventory\n\n");
    output.push_str("| Entry | Crate | Kind | Stability | Source |\n");
    output.push_str("|---|---|---|---|---|\n");
    for entry in entries {
        output.push_str(&format!(
            "| `{}` | `{}` | {} | {} | `{}` |\n",
            entry.key.entry,
            entry.crate_name,
            entry.key.kind,
            entry.stability,
            entry.source.display()
        ));
    }
    output
}

fn scanner_self_test() -> Result<(), String> {
    let fixture = r#"
        pub mod visible;
        #[cfg(feature = "extra")]
        pub struct FeatureType {
            pub value: u8,
            hidden: u8,
        }
        impl FeatureType {
            pub fn new() -> Self {
                Self { value: 0, hidden: 0 }
            }
            fn private() {}
        }
        pub trait Service {
            fn call(&self);
        }
        pub use visible::Thing as ReexportedThing;
        #[cfg(test)]
        pub struct TestOnly {
            pub field: u8,
        }
        #[cfg(test)]
        pub mod tests {
            pub struct Hidden;
        }
        pub struct RuntimeAfterTests {
            pub value: u8,
        }
        pub(crate) struct CrateOnly {
            pub field: u8,
        }
    "#;
    let parsed = syn::parse_file(fixture).map_err(|error| format!("scanner self-test fixture failed to parse: {error}"))?;
    let items = scan_parsed_file(Path::new("crates/fixture/src/lib.rs"), "fixture", &parsed);
    let keys: BTreeSet<(String, String)> = items
        .into_iter()
        .map(|item| (item.key.entry, item.key.kind))
        .collect();
    for expected in [
        ("visible", "module"),
        ("FeatureType", "struct"),
        ("FeatureType::value", FIELD_KIND),
        ("FeatureType::new", METHOD_KIND),
        ("Service", "trait"),
        ("Service::call", METHOD_KIND),
        ("ReexportedThing", REEXPORT_KIND),
        ("RuntimeAfterTests", "struct"),
        ("RuntimeAfterTests::value", FIELD_KIND),
    ] {
        if !keys.contains(&(expected.0.to_string(), expected.1.to_string())) {
            return Err(format!("scanner self-test missed `{}` ({})", expected.0, expected.1));
        }
    }
    for forbidden in [
        ("FeatureType::hidden", FIELD_KIND),
        ("FeatureType::private", METHOD_KIND),
        ("TestOnly", "struct"),
        ("TestOnly::field", FIELD_KIND),
        ("tests", "module"),
        ("Hidden", "struct"),
        ("CrateOnly", "struct"),
        ("CrateOnly::field", FIELD_KIND),
    ] {
        if keys.contains(&(forbidden.0.to_string(), forbidden.1.to_string())) {
            return Err(format!("scanner self-test included hidden/test item `{}` ({})", forbidden.0, forbidden.1));
        }
    }
    Ok(())
}

fn run() -> Result<CheckReport, Vec<String>> {
    scanner_self_test().map_err(|error| vec![error])?;
    let guide = read_text(Path::new(GUIDE_PATH)).map_err(|error| vec![error])?;
    let inventory_text = read_text(Path::new(INVENTORY_PATH)).map_err(|error| vec![error])?;
    let entries = parse_inventory(&inventory_text)?;
    let scanned = scan_public_items(SOURCE_ROOTS).map_err(|error| vec![error])?;
    Ok(validate_inventory(&entries, &scanned, &guide))
}

fn write_or_print_generated_inventory(write_file: bool) -> Result<usize, Vec<String>> {
    scanner_self_test().map_err(|error| vec![error])?;
    let inventory_text = read_text(Path::new(INVENTORY_PATH)).map_err(|error| vec![error])?;
    let existing = parse_inventory(&inventory_text)?;
    let scanned = scan_public_items(SOURCE_ROOTS).map_err(|error| vec![error])?;
    let generated = generate_inventory(&existing, &scanned);
    let rendered = render_inventory(&generated);
    if write_file {
        fs::write(INVENTORY_PATH, rendered)
            .map_err(|error| vec![format!("failed to write {INVENTORY_PATH}: {error}")])?;
    } else {
        print!("{rendered}");
    }
    Ok(generated.len())
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.iter().any(|arg| arg == WRITE_INVENTORY_ARG) {
        match write_or_print_generated_inventory(true) {
            Ok(count) => println!("wrote {INVENTORY_PATH} with {count} rows"),
            Err(errors) => exit_with_errors(errors),
        }
        return;
    }
    if args.iter().any(|arg| arg == GENERATE_INVENTORY_ARG) {
        match write_or_print_generated_inventory(false) {
            Ok(_) => {}
            Err(errors) => exit_with_errors(errors),
        }
        return;
    }

    match run() {
        Ok(report) if report.errors.is_empty() => {
            println!(
                "ok: embedded SDK API inventory covers {} public items ({} rows)",
                report.scanned_count, report.inventory_count
            );
        }
        Ok(report) => exit_with_errors(report.errors),
        Err(errors) => exit_with_errors(errors),
    }
}

fn exit_with_errors(errors: Vec<String>) {
    for error in errors {
        eprintln!("{error}");
    }
    std::process::exit(ERROR_EXIT);
}
