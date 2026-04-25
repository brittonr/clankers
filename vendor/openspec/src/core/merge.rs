//! Delta-to-main-spec merging (sync)

#[cfg(feature = "fs")]
use std::path::Path;

#[cfg(feature = "fs")]
use super::delta::parse_delta_file;

#[derive(Debug)]
pub struct SyncResult {
    pub added: usize,
    pub modified: usize,
    pub removed: usize,
    pub errors: Vec<String>,
}

/// Apply delta to lines (pure version)
/// lines is modified in place with the delta applied
pub fn apply_delta_to_lines(lines: &mut Vec<String>, delta: &super::delta::DeltaSpec) {
    // Apply REMOVED
    for heading in &delta.removed {
        remove_section(lines, heading);
    }

    // Apply MODIFIED (remove then add)
    for req in &delta.modified {
        remove_section(lines, &req.heading);
        append_requirement(lines, &req.heading, &req.body);
    }

    // Apply ADDED
    for req in &delta.added {
        append_requirement(lines, &req.heading, &req.body);
    }
}

#[cfg(feature = "fs")]
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

        // Track what we're applying
        let removed_count = delta.removed.len();
        let modified_count = delta.modified.len();
        let added_count = delta.added.len();

        // Apply delta
        apply_delta_to_lines(&mut lines, &delta);

        result.removed += removed_count;
        result.modified += modified_count;
        result.added += added_count;

        if !dry_run {
            if let Some(parent) = target_path.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            let _ = std::fs::write(&target_path, lines.join("\n"));
        }
    }

    result
}

#[cfg(feature = "fs")]
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

/// Remove a section from lines (pure function)
pub fn remove_section(lines: &mut Vec<String>, heading: &str) {
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

/// Append a requirement to lines (pure function)
pub fn append_requirement(lines: &mut Vec<String>, heading: &str, body: &str) {
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
    use super::super::delta::DeltaSpec;
    use super::super::spec::Requirement;
    use super::super::spec::RequirementStrength;
    use super::*;

    #[test]
    fn test_remove_section() {
        let mut lines = vec![
            "## Intro".to_string(),
            "Content".to_string(),
            "## Target".to_string(),
            "Target content".to_string(),
            "## Other".to_string(),
            "Other content".to_string(),
        ];

        remove_section(&mut lines, "Target");

        assert_eq!(lines.len(), 4);
        assert!(lines.iter().all(|l| !l.contains("Target")));
        assert!(lines.iter().any(|l| l.contains("Intro")));
        assert!(lines.iter().any(|l| l.contains("Other")));
    }

    #[test]
    fn test_append_requirement() {
        let mut lines = vec!["## Existing".to_string(), "Content".to_string()];

        append_requirement(&mut lines, "New Section", "New content\nMultiple lines");

        assert!(lines.iter().any(|l| l == "## New Section"));
        assert!(lines.iter().any(|l| l.contains("New content")));
        assert!(lines.iter().any(|l| l.contains("Multiple lines")));
    }

    #[test]
    fn test_apply_delta_to_lines() {
        let mut lines = vec![
            "## Existing".to_string(),
            "Old content".to_string(),
            "## To Remove".to_string(),
            "This will be gone".to_string(),
        ];

        let delta = DeltaSpec {
            domain: "test".to_string(),
            added: vec![Requirement {
                heading: "New Feature".to_string(),
                body: "New content".to_string(),
                strength: RequirementStrength::Must,
                scenarios: vec![],
            }],
            modified: vec![Requirement {
                heading: "Existing".to_string(),
                body: "Updated content".to_string(),
                strength: RequirementStrength::Should,
                scenarios: vec![],
            }],
            removed: vec!["To Remove".to_string()],
        };

        apply_delta_to_lines(&mut lines, &delta);

        let content = lines.join("\n");
        assert!(content.contains("New Feature"));
        assert!(content.contains("Updated content"));
        assert!(!content.contains("To Remove"));
        assert!(!content.contains("Old content"));
    }

    #[cfg(all(test, feature = "fs"))]
    mod fs_tests {
        use tempfile::TempDir;

        use super::*;

        #[test]
        fn test_sync_empty_change() {
            let main = TempDir::new().expect("failed to create temp dir for main");
            let change = TempDir::new().expect("failed to create temp dir for change");

            let result = sync_change(main.path(), change.path(), false);
            assert_eq!(result.added, 0);
            assert_eq!(result.modified, 0);
            assert_eq!(result.removed, 0);
        }

        #[test]
        fn test_sync_added_requirement() {
            let main = TempDir::new().expect("failed to create temp dir for main");
            let change = TempDir::new().expect("failed to create temp dir for change");

            // Create a delta with ADDED section
            let delta_dir = change.path().join("auth");
            std::fs::create_dir_all(&delta_dir).expect("failed to create delta dir");
            let delta_file = delta_dir.join("spec.md");
            std::fs::write(&delta_file, "## ADDED\n\n### New Requirement\nMUST work")
                .expect("failed to write delta file");

            let result = sync_change(main.path(), change.path(), false);
            assert_eq!(result.added, 1);

            let main_spec = main.path().join("auth").join("spec.md");
            assert!(main_spec.exists());
            let content = std::fs::read_to_string(&main_spec).expect("failed to read main spec");
            assert!(content.contains("New Requirement"));
        }

        #[test]
        fn test_sync_dry_run() {
            let main = TempDir::new().expect("failed to create temp dir for main");
            let change = TempDir::new().expect("failed to create temp dir for change");

            // Create delta
            let delta_dir = change.path().join("auth");
            std::fs::create_dir_all(&delta_dir).expect("failed to create delta dir");
            std::fs::write(delta_dir.join("spec.md"), "## ADDED\n\n### New\nContent")
                .expect("failed to write delta file");

            let result = sync_change(main.path(), change.path(), true);
            assert_eq!(result.added, 1);

            // Main spec should NOT be created in dry run
            let main_spec = main.path().join("auth").join("spec.md");
            assert!(!main_spec.exists());
        }
    }
}
