#!/usr/bin/env -S nix develop -c cargo -q -Zscript
---cargo
[package]
edition = "2024"
---

use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::fs;
use std::path::Path;
use std::path::PathBuf;

const GUIDE_PATH: &str = "docs/src/tutorials/embedded-agent-sdk.md";
const INVENTORY_PATH: &str = "docs/src/generated/embedded-sdk-api.md";
const INVENTORY_GUIDE_LINK: &str = "../generated/embedded-sdk-api.md";
const EXPECTED_COLUMN_COUNT: usize = 5;
const ENTRY_CELL_PREFIX: &str = "`";
const CODE_SPAN_DELIMITER: char = '`';
const TABLE_SEPARATOR_PREFIX: &str = "---";
const TEST_CFG_ATTR: &str = "#[cfg(test)]";
const TEST_MODULE_PREFIX: &str = "mod tests";
const PUBLIC_PREFIX: &str = "pub ";
const PUBLIC_ASYNC_FN_PREFIX: &str = "pub async fn ";
const PUBLIC_CONST_PREFIX: &str = "pub const ";
const PUBLIC_ENUM_PREFIX: &str = "pub enum ";
const PUBLIC_FN_PREFIX: &str = "pub fn ";
const PUBLIC_MOD_PREFIX: &str = "pub mod ";
const PUBLIC_STRUCT_PREFIX: &str = "pub struct ";
const PUBLIC_TRAIT_PREFIX: &str = "pub trait ";
const PUBLIC_TYPE_PREFIX: &str = "pub type ";
const PUBLIC_USE_PREFIX: &str = "pub use ";
const PRIVATE_PUBLIC_PREFIX: &str = "pub(";
const EXAMPLE_KIND: &str = "example";
const ERROR_EXIT: i32 = 1;

const SOURCE_ROOTS: &[&str] = &[
    "crates/clankers-engine/src",
    "crates/clankers-engine-host/src",
    "crates/clankers-tool-host/src",
    "crates/clanker-message/src",
];

const VALID_STABILITIES: &[&str] = &["supported", "optional-support", "experimental", "unsupported-internal"];

#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd)]
struct PublicItemKey {
    name: String,
    kind: String,
}

#[derive(Debug, Clone)]
struct PublicItem {
    key: PublicItemKey,
    source: PathBuf,
}

#[derive(Debug, Clone)]
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

        let cells: Vec<String> = trimmed.trim_matches('|').split('|').map(|cell| cell.trim().to_string()).collect();

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

        let Some(name) = extract_code_span(&cells[0]) else {
            continue;
        };
        let crate_name = strip_code_span(&cells[1]);
        let kind = cells[2].trim().to_string();
        let stability = cells[3].trim().to_string();
        let source = strip_code_span(&cells[4]);

        entries.push(InventoryEntry {
            key: PublicItemKey { name, kind },
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

fn runtime_lines(text: &str) -> Vec<String> {
    let mut lines = Vec::new();
    let mut pending_test_attr = false;
    let mut skipping_test_module = false;
    let mut skip_depth = 0i32;

    for line in text.lines() {
        if skipping_test_module {
            skip_depth += brace_delta(line);
            if skip_depth <= 0 {
                skipping_test_module = false;
                skip_depth = 0;
            }
            continue;
        }

        let trimmed = line.trim();
        if trimmed == TEST_CFG_ATTR {
            pending_test_attr = true;
            continue;
        }

        if pending_test_attr {
            if trimmed.starts_with(TEST_MODULE_PREFIX) {
                skipping_test_module = true;
                skip_depth = brace_delta(line);
                if skip_depth <= 0 {
                    skipping_test_module = false;
                    skip_depth = 0;
                }
                pending_test_attr = false;
                continue;
            }
            if trimmed.starts_with("#[") || trimmed.is_empty() {
                continue;
            }
            pending_test_attr = false;
        }

        lines.push(line.to_string());
    }

    lines
}

fn brace_delta(line: &str) -> i32 {
    let mut delta = 0i32;
    for ch in line.chars() {
        match ch {
            '{' => delta += 1,
            '}' => delta -= 1,
            _ => {}
        }
    }
    delta
}

fn scan_public_items(roots: &[&str]) -> Result<Vec<PublicItem>, String> {
    let mut items = Vec::new();
    for root in roots {
        let root_path = Path::new(root);
        for file in collect_rust_files(root_path)? {
            let text = read_text(&file)?;
            for line in runtime_lines(&text) {
                if let Some(key) = parse_public_item_line(&line) {
                    items.push(PublicItem {
                        key,
                        source: file.clone(),
                    });
                }
            }
        }
    }
    items.sort_by(|left, right| left.key.cmp(&right.key).then_with(|| left.source.cmp(&right.source)));
    Ok(items)
}

fn parse_public_item_line(line: &str) -> Option<PublicItemKey> {
    if !line.starts_with(PUBLIC_PREFIX)
        || line.starts_with(PRIVATE_PUBLIC_PREFIX)
        || line.starts_with(PUBLIC_USE_PREFIX)
    {
        return None;
    }

    if let Some(name) = extract_identifier_after(line, PUBLIC_ASYNC_FN_PREFIX) {
        return Some(key(name, "function"));
    }
    if let Some(name) = extract_identifier_after(line, PUBLIC_CONST_PREFIX) {
        return Some(key(name, "constant"));
    }
    if let Some(name) = extract_identifier_after(line, PUBLIC_ENUM_PREFIX) {
        return Some(key(name, "enum"));
    }
    if let Some(name) = extract_identifier_after(line, PUBLIC_FN_PREFIX) {
        return Some(key(name, "function"));
    }
    if let Some(name) = extract_identifier_after(line, PUBLIC_MOD_PREFIX) {
        return Some(key(name, "module"));
    }
    if let Some(name) = extract_identifier_after(line, PUBLIC_STRUCT_PREFIX) {
        return Some(key(name, "struct"));
    }
    if let Some(name) = extract_identifier_after(line, PUBLIC_TRAIT_PREFIX) {
        return Some(key(name, "trait"));
    }
    if let Some(name) = extract_identifier_after(line, PUBLIC_TYPE_PREFIX) {
        return Some(key(name, "type"));
    }

    None
}

fn key(name: String, kind: &str) -> PublicItemKey {
    PublicItemKey {
        name,
        kind: kind.to_string(),
    }
}

fn extract_identifier_after(line: &str, prefix: &str) -> Option<String> {
    let tail = line.strip_prefix(prefix)?;
    let name: String = tail.chars().take_while(|ch| ch.is_ascii_alphanumeric() || *ch == '_').collect();
    if name.is_empty() { None } else { Some(name) }
}

fn index_scanned(items: &[PublicItem]) -> BTreeMap<PublicItemKey, BTreeSet<PathBuf>> {
    let mut index: BTreeMap<PublicItemKey, BTreeSet<PathBuf>> = BTreeMap::new();
    for item in items {
        index.entry(item.key.clone()).or_default().insert(item.source.clone());
    }
    index
}

fn inventory_key_set(entries: &[InventoryEntry]) -> BTreeSet<PublicItemKey> {
    entries.iter().map(|entry| entry.key.clone()).collect()
}

fn validate_inventory(entries: &[InventoryEntry], scanned: &[PublicItem], guide: &str) -> CheckReport {
    let mut errors = Vec::new();
    let scanned_index = index_scanned(scanned);
    let inventory_keys = inventory_key_set(entries);

    if !guide.contains(INVENTORY_GUIDE_LINK) {
        errors
            .push(format!("{GUIDE_PATH} must link to {INVENTORY_GUIDE_LINK} so SDK entrypoints resolve to inventory"));
    }

    let valid_stabilities: BTreeSet<&str> = VALID_STABILITIES.iter().copied().collect();
    let mut seen_rows = BTreeSet::new();
    for entry in entries {
        let row_key = format!("{}|{}|{}|{}", entry.key.name, entry.key.kind, entry.crate_name, entry.source.display());
        if !seen_rows.insert(row_key) {
            errors.push(format!(
                "duplicate inventory row for `{}` ({}) from {}",
                entry.key.name,
                entry.key.kind,
                entry.source.display()
            ));
        }

        if !valid_stabilities.contains(entry.stability.as_str()) {
            errors.push(format!(
                "invalid stability `{}` for `{}`; expected one of {:?}",
                entry.stability, entry.key.name, VALID_STABILITIES
            ));
        }

        if !entry.source.exists() {
            errors.push(format!("source path for `{}` does not exist: {}", entry.key.name, entry.source.display()));
            continue;
        }

        if entry.key.kind == EXAMPLE_KIND {
            continue;
        }

        match scanned_index.get(&entry.key) {
            Some(paths) if paths.contains(&entry.source) => {}
            Some(paths) => errors.push(format!(
                "inventory maps `{}` ({}) to {}, but public item was found in {:?}",
                entry.key.name,
                entry.key.kind,
                entry.source.display(),
                paths
            )),
            None => errors.push(format!(
                "inventory entry `{}` ({}) does not match any scanned public item",
                entry.key.name, entry.key.kind
            )),
        }
    }

    for item in scanned {
        if !inventory_keys.contains(&item.key) {
            errors.push(format!(
                "public SDK item missing from inventory: `{}` ({}) in {}",
                item.key.name,
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

fn run() -> Result<CheckReport, Vec<String>> {
    let guide = read_text(Path::new(GUIDE_PATH)).map_err(|error| vec![error])?;
    let inventory_text = read_text(Path::new(INVENTORY_PATH)).map_err(|error| vec![error])?;
    let entries = parse_inventory(&inventory_text)?;
    let scanned = scan_public_items(SOURCE_ROOTS).map_err(|error| vec![error])?;
    Ok(validate_inventory(&entries, &scanned, &guide))
}

fn main() {
    match run() {
        Ok(report) if report.errors.is_empty() => {
            println!(
                "ok: embedded SDK API inventory covers {} public items ({} rows)",
                report.scanned_count, report.inventory_count
            );
        }
        Ok(report) => {
            for error in report.errors {
                eprintln!("{error}");
            }
            std::process::exit(ERROR_EXIT);
        }
        Err(errors) => {
            for error in errors {
                eprintln!("{error}");
            }
            std::process::exit(ERROR_EXIT);
        }
    }
}
