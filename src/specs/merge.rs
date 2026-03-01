//! Delta-to-main-spec merging (sync)

use std::path::Path;

use super::delta::parse_delta_file;

#[derive(Debug)]
pub struct SyncResult {
    pub added: usize,
    pub modified: usize,
    pub removed: usize,
    pub errors: Vec<String>,
}

/// Sync a change's delta specs into the main specs
pub fn sync_change(main_specs_dir: &Path, change_specs_dir: &Path, dry_run: bool) -> SyncResult {
    let mut result = SyncResult {
        added: 0,
        modified: 0,
        removed: 0,
        errors: vec![],
    };

    // Find delta spec files
    let delta_files = find_delta_files(change_specs_dir);

    for delta_path in delta_files {
        let Some(delta) = parse_delta_file(&delta_path, change_specs_dir) else {
            result.errors.push(format!("Failed to parse delta: {}", delta_path.display()));
            continue;
        };

        let target_path = main_specs_dir.join(&delta.domain).join("spec.md");

        // Read existing spec content or start empty
        let existing = std::fs::read_to_string(&target_path).unwrap_or_default();
        let mut lines: Vec<String> = existing.lines().map(String::from).collect();

        // Apply REMOVED
        for heading in &delta.removed {
            remove_section(&mut lines, heading);
            result.removed += 1;
        }

        // Apply MODIFIED (remove then add)
        for req in &delta.modified {
            remove_section(&mut lines, &req.heading);
            append_requirement(&mut lines, &req.heading, &req.body);
            result.modified += 1;
        }

        // Apply ADDED
        for req in &delta.added {
            append_requirement(&mut lines, &req.heading, &req.body);
            result.added += 1;
        }

        if !dry_run {
            if let Some(parent) = target_path.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            let _ = std::fs::write(&target_path, lines.join("\n"));
        }
    }

    result
}

fn find_delta_files(dir: &Path) -> Vec<std::path::PathBuf> {
    let mut files = Vec::new();
    fn walk(dir: &Path, files: &mut Vec<std::path::PathBuf>) {
        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    walk(&path, files);
                } else if path.extension().is_some_and(|e| e == "md") {
                    files.push(path);
                }
            }
        }
    }
    walk(dir, &mut files);
    files
}

fn remove_section(lines: &mut Vec<String>, heading: &str) {
    let target = format!("## {}", heading);
    let alt = format!("### {}", heading);
    let mut start = None;
    let mut end = None;
    for (i, line) in lines.iter().enumerate() {
        if line.trim() == target || line.trim() == alt {
            start = Some(i);
        } else if start.is_some() && (line.starts_with("## ") || line.starts_with("### ")) {
            end = Some(i);
            break;
        }
    }
    if let Some(s) = start {
        let e = end.unwrap_or(lines.len());
        lines.drain(s..e);
    }
}

fn append_requirement(lines: &mut Vec<String>, heading: &str, body: &str) {
    if !lines.is_empty() && !lines.last().is_some_and(|l| l.is_empty()) {
        lines.push(String::new());
    }
    lines.push(format!("## {}", heading));
    lines.push(String::new());
    for line in body.lines() {
        lines.push(line.to_string());
    }
}

#[cfg(test)]
mod tests {
    use tempfile::TempDir;

    use super::*;

    #[test]
    fn test_sync_empty_change() {
        let main = TempDir::new().unwrap();
        let change = TempDir::new().unwrap();

        let result = sync_change(main.path(), change.path(), false);
        assert_eq!(result.added, 0);
        assert_eq!(result.modified, 0);
        assert_eq!(result.removed, 0);
    }

    #[test]
    fn test_sync_added_requirement() {
        let main = TempDir::new().unwrap();
        let change = TempDir::new().unwrap();

        // Create a delta with ADDED section
        let delta_dir = change.path().join("auth");
        std::fs::create_dir_all(&delta_dir).unwrap();
        let delta_file = delta_dir.join("spec.md");
        std::fs::write(&delta_file, "## ADDED\n\n### New Requirement\nMUST work").unwrap();

        let result = sync_change(main.path(), change.path(), false);
        assert_eq!(result.added, 1);

        let main_spec = main.path().join("auth").join("spec.md");
        assert!(main_spec.exists());
        let content = std::fs::read_to_string(&main_spec).unwrap();
        assert!(content.contains("New Requirement"));
    }

    #[test]
    fn test_sync_removed_requirement() {
        let main = TempDir::new().unwrap();
        let change = TempDir::new().unwrap();

        // Create existing main spec
        let main_dir = main.path().join("auth");
        std::fs::create_dir_all(&main_dir).unwrap();
        let main_spec = main_dir.join("spec.md");
        std::fs::write(&main_spec, "## Old Requirement\nContent").unwrap();

        // Create delta with REMOVED section
        let delta_dir = change.path().join("auth");
        std::fs::create_dir_all(&delta_dir).unwrap();
        let delta_file = delta_dir.join("spec.md");
        std::fs::write(&delta_file, "## REMOVED\n\n### Old Requirement\n").unwrap();

        let result = sync_change(main.path(), change.path(), false);
        assert_eq!(result.removed, 1);

        let content = std::fs::read_to_string(&main_spec).unwrap();
        assert!(!content.contains("Old Requirement"));
    }

    #[test]
    fn test_sync_modified_requirement() {
        let main = TempDir::new().unwrap();
        let change = TempDir::new().unwrap();

        // Create existing main spec
        let main_dir = main.path().join("auth");
        std::fs::create_dir_all(&main_dir).unwrap();
        let main_spec = main_dir.join("spec.md");
        std::fs::write(&main_spec, "## Feature\nOld content").unwrap();

        // Create delta with MODIFIED section
        let delta_dir = change.path().join("auth");
        std::fs::create_dir_all(&delta_dir).unwrap();
        let delta_file = delta_dir.join("spec.md");
        std::fs::write(&delta_file, "## MODIFIED\n\n### Feature\nNew content").unwrap();

        let result = sync_change(main.path(), change.path(), false);
        assert_eq!(result.modified, 1);

        let content = std::fs::read_to_string(&main_spec).unwrap();
        assert!(content.contains("New content"));
        assert!(!content.contains("Old content"));
    }

    #[test]
    fn test_sync_dry_run() {
        let main = TempDir::new().unwrap();
        let change = TempDir::new().unwrap();

        // Create delta
        let delta_dir = change.path().join("auth");
        std::fs::create_dir_all(&delta_dir).unwrap();
        std::fs::write(delta_dir.join("spec.md"), "## ADDED\n\n### New\nContent").unwrap();

        let result = sync_change(main.path(), change.path(), true);
        assert_eq!(result.added, 1);

        // Main spec should NOT be created in dry run
        let main_spec = main.path().join("auth").join("spec.md");
        assert!(!main_spec.exists());
    }
}
