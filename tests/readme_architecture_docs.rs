use std::collections::BTreeSet;

const ROOT_CARGO_TOML: &str = include_str!("../Cargo.toml");
const README: &str = include_str!("../README.md");

#[test]
fn readme_architecture_table_lists_all_workspace_local_crates() {
    let workspace_crates = workspace_local_crates(ROOT_CARGO_TOML);
    assert!(
        workspace_crates.len() >= 30,
        "expected the workspace to have many local crates, got {workspace_crates:?}"
    );

    let readme_crates = readme_architecture_table_crates(README);
    assert!(
        readme_crates.len() >= workspace_crates.len(),
        "README architecture table should not be missing workspace-local crates"
    );

    let missing: Vec<_> = workspace_crates.difference(&readme_crates).cloned().collect();
    assert!(missing.is_empty(), "README architecture table is missing workspace-local crates: {missing:?}");
}

#[test]
fn readme_architecture_table_has_no_stale_workspace_crates() {
    let workspace_crates = workspace_local_crates(ROOT_CARGO_TOML);
    let readme_crates = readme_architecture_table_crates(README);
    let stale: Vec<_> = readme_crates.difference(&workspace_crates).cloned().collect();
    assert!(
        stale.is_empty(),
        "README architecture table lists crates that are not workspace members under crates/: {stale:?}"
    );
}

#[test]
fn extracted_first_party_dependencies_are_not_misclassified_as_workspace_local() {
    let readme_crates = readme_architecture_table_crates(README);
    for extracted in ["clanker-actor", "clanker-loop", "clanker-scheduler", "graggle"] {
        assert!(
            !readme_crates.contains(extracted),
            "{extracted} should be documented as an extracted dependency, not a workspace-local crate"
        );
        assert!(
            README.contains(&format!("github.com/brittonr/{extracted}")),
            "README should still mention extracted first-party dependency {extracted}"
        );
    }
}

fn workspace_local_crates(manifest: &str) -> BTreeSet<String> {
    workspace_member_paths(manifest)
        .into_iter()
        .filter_map(|path| path.strip_prefix("crates/").map(str::to_string))
        .collect()
}

fn workspace_member_paths(manifest: &str) -> Vec<String> {
    let mut paths = Vec::new();
    let mut in_members = false;

    for line in manifest.lines() {
        let trimmed = line.trim();
        if trimmed == "members = [" {
            in_members = true;
            continue;
        }
        if in_members && trimmed == "]" {
            break;
        }
        if !in_members {
            continue;
        }
        if let Some(path) = quoted_value(trimmed) {
            paths.push(path.to_string());
        }
    }

    paths
}

fn readme_architecture_table_crates(readme: &str) -> BTreeSet<String> {
    let mut crates = BTreeSet::new();
    let marker = "Workspace-local crates under `crates/`:";
    let start = readme.find(marker).expect("README should have a workspace-local crates architecture section");
    let section = &readme[start..];
    let table_start =
        section.find("| Crate | Purpose |").expect("README architecture section should contain crate table");
    let table = &section[table_start..];

    for line in table.lines().skip(2) {
        if !line.starts_with('|') {
            break;
        }
        let Some(crate_name) = table_crate_name(line) else {
            continue;
        };
        crates.insert(crate_name.to_string());
    }

    crates
}

fn table_crate_name(line: &str) -> Option<&str> {
    let mut columns = line.split('|');
    columns.next()?;
    let crate_column = columns.next()?.trim();
    crate_column.strip_prefix('`')?.strip_suffix('`')
}

fn quoted_value(line: &str) -> Option<&str> {
    let after_open = line.strip_prefix('"')?;
    let end = after_open.find('"')?;
    Some(&after_open[..end])
}
