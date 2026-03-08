//! Delta spec parsing (ADDED/MODIFIED/REMOVED sections)

use std::path::Path;

use serde::Deserialize;
use serde::Serialize;

use super::spec::Requirement;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeltaSpec {
    pub domain: String,
    pub added: Vec<Requirement>,
    pub modified: Vec<Requirement>,
    pub removed: Vec<String>, // heading names of removed requirements
}

/// Parse a delta spec file (from changes/<name>/specs/<domain>/spec.md)
pub fn parse_delta_file(path: &Path, specs_root: &Path) -> Option<DeltaSpec> {
    let content = std::fs::read_to_string(path).ok()?;
    let domain = path.parent()?.strip_prefix(specs_root).ok()?.to_string_lossy().to_string();

    let mut added = Vec::new();
    let mut modified = Vec::new();
    let mut removed = Vec::new();
    let mut section = ""; // "added", "modified", "removed"
    let mut current_heading: Option<String> = None;
    let mut current_body = String::new();

    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("## ADDED") {
            flush_delta(section, &current_heading, &current_body, &mut added, &mut modified, &mut removed);
            section = "added";
            current_heading = None;
            current_body.clear();
        } else if trimmed.starts_with("## MODIFIED") {
            flush_delta(section, &current_heading, &current_body, &mut added, &mut modified, &mut removed);
            section = "modified";
            current_heading = None;
            current_body.clear();
        } else if trimmed.starts_with("## REMOVED") {
            flush_delta(section, &current_heading, &current_body, &mut added, &mut modified, &mut removed);
            section = "removed";
            current_heading = None;
            current_body.clear();
        } else if trimmed.starts_with("### ") {
            flush_delta(section, &current_heading, &current_body, &mut added, &mut modified, &mut removed);
            current_heading = Some(trimmed.trim_start_matches('#').trim().to_string());
            current_body.clear();
        } else {
            current_body.push_str(line);
            current_body.push('\n');
        }
    }
    flush_delta(section, &current_heading, &current_body, &mut added, &mut modified, &mut removed);

    Some(DeltaSpec {
        domain,
        added,
        modified,
        removed,
    })
}

fn flush_delta(
    section: &str,
    heading: &Option<String>,
    body: &str,
    added: &mut Vec<Requirement>,
    modified: &mut Vec<Requirement>,
    removed: &mut Vec<String>,
) {
    let Some(h) = heading else { return };
    let body = body.trim();
    match section {
        "added" => {
            added.push(Requirement {
                heading: h.clone(),
                body: body.to_string(),
                strength: super::spec::detect_strength(body),
                scenarios: super::spec::parse_scenarios(body),
            });
        }
        "modified" => {
            modified.push(Requirement {
                heading: h.clone(),
                body: body.to_string(),
                strength: super::spec::detect_strength(body),
                scenarios: super::spec::parse_scenarios(body),
            });
        }
        "removed" => {
            removed.push(h.clone());
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use tempfile::TempDir;

    use super::*;

    #[test]
    fn test_parse_delta_empty() {
        let dir = TempDir::new().expect("failed to create temp dir for test");
        let file = dir.path().join("spec.md");
        std::fs::write(&file, "# Empty delta").expect("failed to write delta file");

        let delta = parse_delta_file(&file, dir.path());
        assert!(delta.is_some());
        let delta = delta.expect("delta should be parsed successfully");
        assert!(delta.added.is_empty());
        assert!(delta.modified.is_empty());
        assert!(delta.removed.is_empty());
    }

    #[test]
    fn test_parse_delta_added() {
        let dir = TempDir::new().expect("failed to create temp dir for test");
        let file = dir.path().join("spec.md");
        std::fs::write(&file, "## ADDED\n\n### New Feature\nThe system MUST support this").expect("failed to write delta file");

        let delta = parse_delta_file(&file, dir.path()).expect("failed to parse delta file");
        assert_eq!(delta.added.len(), 1);
        assert_eq!(delta.added[0].heading, "New Feature");
        assert!(delta.added[0].body.contains("MUST"));
    }

    #[test]
    fn test_parse_delta_modified() {
        let dir = TempDir::new().expect("failed to create temp dir for test");
        let file = dir.path().join("spec.md");
        std::fs::write(&file, "## MODIFIED\n\n### Updated Feature\nChanged behavior").expect("failed to write delta file");

        let delta = parse_delta_file(&file, dir.path()).expect("failed to parse delta file");
        assert_eq!(delta.modified.len(), 1);
        assert_eq!(delta.modified[0].heading, "Updated Feature");
    }

    #[test]
    fn test_parse_delta_removed() {
        let dir = TempDir::new().expect("failed to create temp dir for test");
        let file = dir.path().join("spec.md");
        std::fs::write(&file, "## REMOVED\n\n### Old Feature\n").expect("failed to write delta file");

        let delta = parse_delta_file(&file, dir.path()).expect("failed to parse delta file");
        assert_eq!(delta.removed.len(), 1);
        assert_eq!(delta.removed[0], "Old Feature");
    }

    #[test]
    fn test_parse_delta_all_sections() {
        let dir = TempDir::new().expect("failed to create temp dir for test");
        let file = dir.path().join("spec.md");
        let content = "## ADDED\n\n### A\nContent A\n\n## MODIFIED\n\n### B\nContent B\n\n## REMOVED\n\n### C\n";
        std::fs::write(&file, content).expect("failed to write delta file");

        let delta = parse_delta_file(&file, dir.path()).expect("failed to parse delta file");
        assert_eq!(delta.added.len(), 1);
        assert_eq!(delta.modified.len(), 1);
        assert_eq!(delta.removed.len(), 1);
    }
}
