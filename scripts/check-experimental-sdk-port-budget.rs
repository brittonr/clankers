#!/usr/bin/env -S nix develop -c cargo -q -Zscript
---cargo
[package]
edition = "2024"
[dependencies]
serde_json = "1"
---

use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::fs;
use std::process::ExitCode;

use serde_json::Value;

const POLICY_PATH: &str = "policy/embedded-lego/experimental-sdk-port-budget.json";
const ERROR_EXIT: u8 = 1;

#[derive(Debug, Clone)]
struct InventoryRow {
    entry: String,
    crate_name: String,
    kind: String,
    stability: String,
    source: String,
}

#[derive(Debug, Clone)]
struct BudgetGroup {
    id: String,
    crate_name: String,
    owner_module: String,
    disposition: String,
    expected_stability: String,
    use_site_status: String,
    validation: String,
    rationale: String,
    expected_rows: usize,
    entry_prefixes: Vec<String>,
}

fn main() -> ExitCode {
    match run() {
        Ok(summary) => {
            println!("ok: experimental SDK port budget covers {summary}");
            ExitCode::SUCCESS
        }
        Err(errors) => {
            for error in errors {
                eprintln!("experimental SDK port budget error: {error}");
            }
            ExitCode::from(ERROR_EXIT)
        }
    }
}

fn run() -> Result<String, Vec<String>> {
    let policy_text = read(POLICY_PATH)?;
    let policy: Value = serde_json::from_str(&policy_text)
        .map_err(|error| vec![format!("failed to parse {POLICY_PATH}: {error}")])?;
    let inventory_path = required_str(&policy, "inventory_path")?;
    let inventory_text = read(inventory_path)?;
    let rows = parse_inventory(&inventory_text)?;
    let groups = parse_groups(&policy)?;

    let mut errors = Vec::new();
    validate_groups(&groups, &mut errors);
    validate_rows(&rows, &groups, &policy, &mut errors);

    if errors.is_empty() {
        let experimental = rows.iter().filter(|row| row.stability == "experimental").count();
        let promoted = groups
            .iter()
            .filter(|group| group.disposition == "promote-with-evidence")
            .map(|group| group.expected_rows)
            .sum::<usize>();
        Ok(format!("{experimental} experimental rows; {promoted} promoted rows"))
    } else {
        Err(errors)
    }
}

fn read(path: &str) -> Result<String, Vec<String>> {
    fs::read_to_string(path).map_err(|error| vec![format!("failed to read {path}: {error}")])
}

fn required_str<'a>(value: &'a Value, key: &str) -> Result<&'a str, Vec<String>> {
    value
        .get(key)
        .and_then(Value::as_str)
        .filter(|text| !text.trim().is_empty())
        .ok_or_else(|| vec![format!("{POLICY_PATH} missing non-empty string field `{key}`")])
}

fn parse_inventory(text: &str) -> Result<Vec<InventoryRow>, Vec<String>> {
    let mut rows = Vec::new();
    let mut errors = Vec::new();
    for (index, line) in text.lines().enumerate() {
        let trimmed = line.trim();
        if !trimmed.starts_with("| `") {
            continue;
        }
        let cells = trimmed
            .trim_matches('|')
            .split('|')
            .map(|cell| cell.trim().to_string())
            .collect::<Vec<_>>();
        if cells.len() != 5 {
            errors.push(format!("inventory row {} has {} cells, expected 5", index + 1, cells.len()));
            continue;
        }
        rows.push(InventoryRow {
            entry: strip_code(&cells[0]),
            crate_name: strip_code(&cells[1]),
            kind: cells[2].clone(),
            stability: cells[3].clone(),
            source: strip_code(&cells[4]),
        });
    }
    if rows.is_empty() {
        errors.push("inventory has no rows".to_string());
    }
    if errors.is_empty() { Ok(rows) } else { Err(errors) }
}

fn strip_code(text: &str) -> String {
    text.trim().trim_matches('`').to_string()
}

fn parse_groups(policy: &Value) -> Result<Vec<BudgetGroup>, Vec<String>> {
    let Some(groups) = policy.get("groups").and_then(Value::as_array) else {
        return Err(vec![format!("{POLICY_PATH} missing groups array")]);
    };
    let mut parsed = Vec::new();
    let mut errors = Vec::new();
    for (index, group) in groups.iter().enumerate() {
        match parse_group(group) {
            Ok(group) => parsed.push(group),
            Err(mut new_errors) => {
                for error in &mut new_errors {
                    *error = format!("groups[{index}]: {error}");
                }
                errors.append(&mut new_errors);
            }
        }
    }
    if parsed.is_empty() {
        errors.push(format!("{POLICY_PATH} must define at least one budget group"));
    }
    if errors.is_empty() { Ok(parsed) } else { Err(errors) }
}

fn parse_group(group: &Value) -> Result<BudgetGroup, Vec<String>> {
    let mut errors = Vec::new();
    let id = group_str(group, "id", &mut errors);
    let crate_name = group_str(group, "crate", &mut errors);
    let owner_module = group_str(group, "owner_module", &mut errors);
    let disposition = group_str(group, "disposition", &mut errors);
    let expected_stability = group_str(group, "expected_stability", &mut errors);
    let use_site_status = group_str(group, "use_site_status", &mut errors);
    let validation = group_str(group, "validation", &mut errors);
    let rationale = group_str(group, "rationale", &mut errors);
    let expected_rows = match group.get("expected_rows").and_then(Value::as_u64) {
        Some(value) => value as usize,
        None => {
            errors.push("missing numeric expected_rows".to_string());
            0
        }
    };
    let entry_prefixes = match group.get("entry_prefixes").and_then(Value::as_array) {
        Some(values) => values.iter().filter_map(Value::as_str).map(ToOwned::to_owned).collect::<Vec<_>>(),
        None => {
            errors.push("missing entry_prefixes array".to_string());
            Vec::new()
        }
    };
    if entry_prefixes.is_empty() {
        errors.push("entry_prefixes must not be empty".to_string());
    }
    if errors.is_empty() {
        Ok(BudgetGroup {
            id,
            crate_name,
            owner_module,
            disposition,
            expected_stability,
            use_site_status,
            validation,
            rationale,
            expected_rows,
            entry_prefixes,
        })
    } else {
        Err(errors)
    }
}

fn group_str(group: &Value, key: &str, errors: &mut Vec<String>) -> String {
    match group.get(key).and_then(Value::as_str).filter(|value| !value.trim().is_empty()) {
        Some(value) => value.to_string(),
        None => {
            errors.push(format!("missing non-empty `{key}`"));
            String::new()
        }
    }
}

fn validate_groups(groups: &[BudgetGroup], errors: &mut Vec<String>) {
    let mut ids = BTreeSet::new();
    for group in groups {
        if !ids.insert(group.id.clone()) {
            errors.push(format!("duplicate budget group id `{}`", group.id));
        }
        if !matches!(
            group.disposition.as_str(),
            "promote-with-evidence" | "keep-experimental-with-rationale" | "make-private"
        ) {
            errors.push(format!("group `{}` has invalid disposition `{}`", group.id, group.disposition));
        }
        if !matches!(
            group.expected_stability.as_str(),
            "supported" | "optional-support" | "compatibility-alias" | "experimental" | "absent"
        ) {
            errors.push(format!(
                "group `{}` has invalid expected_stability `{}`",
                group.id, group.expected_stability
            ));
        }
        for (field, value) in [
            ("owner_module", &group.owner_module),
            ("use_site_status", &group.use_site_status),
            ("validation", &group.validation),
            ("rationale", &group.rationale),
        ] {
            if value.trim().len() < 8 {
                errors.push(format!("group `{}` needs actionable {field}", group.id));
            }
        }
        if group.disposition == "make-private" && group.expected_rows != 0 {
            errors.push(format!("group `{}` is make-private but expects public rows", group.id));
        }
    }
}

fn validate_rows(rows: &[InventoryRow], groups: &[BudgetGroup], policy: &Value, errors: &mut Vec<String>) {
    let expected_experimental = policy.get("expected_experimental_rows").and_then(Value::as_u64);
    let actual_experimental = rows.iter().filter(|row| row.stability == "experimental").count();
    if expected_experimental != Some(actual_experimental as u64) {
        errors.push(format!(
            "experimental row count drift: expected {:?}, got {actual_experimental}",
            expected_experimental
        ));
    }

    let mut group_rows: BTreeMap<&str, Vec<&InventoryRow>> = BTreeMap::new();
    for row in rows {
        let matches = groups.iter().filter(|group| row_matches_group(row, group)).collect::<Vec<_>>();
        if row.stability == "experimental" && matches.is_empty() {
            errors.push(format!(
                "experimental row `{}` ({}) from {} lacks a budget group",
                row.entry, row.kind, row.source
            ));
        }
        if matches.len() > 1 {
            errors.push(format!(
                "row `{}` ({}) matches multiple budget groups: {:?}",
                row.entry,
                row.kind,
                matches.iter().map(|group| group.id.as_str()).collect::<Vec<_>>()
            ));
        }
        if let Some(group) = matches.first() {
            group_rows.entry(group.id.as_str()).or_default().push(row);
        }
    }

    for group in groups {
        let rows = group_rows.remove(group.id.as_str()).unwrap_or_default();
        if rows.len() != group.expected_rows {
            errors.push(format!(
                "group `{}` row count drift: expected {}, got {}",
                group.id,
                group.expected_rows,
                rows.len()
            ));
        }
        match group.expected_stability.as_str() {
            "supported" | "optional-support" | "compatibility-alias" | "experimental" => {
                require_stability(group, &rows, &group.expected_stability, errors);
            }
            "absent" => {
                if !rows.is_empty() {
                    errors.push(format!("group `{}` expected no public rows", group.id));
                }
            }
            _ => {}
        }
    }
}

fn row_matches_group(row: &InventoryRow, group: &BudgetGroup) -> bool {
    row.crate_name == group.crate_name
        && group.entry_prefixes.iter().any(|prefix| row.entry == *prefix || row.entry.starts_with(&format!("{prefix}::")))
}

fn require_stability(group: &BudgetGroup, rows: &[&InventoryRow], expected: &str, errors: &mut Vec<String>) {
    for row in rows {
        if row.stability != expected {
            errors.push(format!(
                "group `{}` expected `{expected}` row but `{}` is `{}`",
                group.id, row.entry, row.stability
            ));
        }
    }
}
