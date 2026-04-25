//! Delta spec parsing (ADDED/MODIFIED/REMOVED sections)

#[cfg(feature = "fs")]
use std::path::Path;

use serde::Deserialize;
use serde::Serialize;

use super::spec::Requirement;
use super::spec::detect_strength;
use super::spec::parse_scenarios;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeltaSpec {
    pub domain: String,
    pub added: Vec<Requirement>,
    pub modified: Vec<Requirement>,
    pub removed: Vec<String>, // heading names of removed requirements
}

/// Parse delta spec content from a string (pure function)
pub fn parse_delta_content(content: &str, domain: &str) -> Option<DeltaSpec> {
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
        domain: domain.to_string(),
        added,
        modified,
        removed,
    })
}

#[cfg(feature = "fs")]
/// Parse a delta spec file (from changes/<name>/specs/<domain>/spec.md)
pub fn parse_delta_file(path: &Path, specs_root: &Path) -> Option<DeltaSpec> {
    let content = std::fs::read_to_string(path).ok()?;
    let domain = path.parent()?.strip_prefix(specs_root).ok()?.to_string_lossy().to_string();
    parse_delta_content(&content, &domain)
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
                strength: detect_strength(body),
                scenarios: parse_scenarios(body),
            });
        }
        "modified" => {
            modified.push(Requirement {
                heading: h.clone(),
                body: body.to_string(),
                strength: detect_strength(body),
                scenarios: parse_scenarios(body),
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
    use super::*;

    #[test]
    fn test_parse_delta_content_empty() {
        let delta = parse_delta_content("# Empty delta", "test").unwrap();
        assert!(delta.added.is_empty());
        assert!(delta.modified.is_empty());
        assert!(delta.removed.is_empty());
    }

    #[test]
    fn test_parse_delta_content_added() {
        let content = "## ADDED\n\n### New Feature\nThe system MUST support this";
        let delta = parse_delta_content(content, "test").expect("failed to parse delta");
        assert_eq!(delta.added.len(), 1);
        assert_eq!(delta.added[0].heading, "New Feature");
        assert!(delta.added[0].body.contains("MUST"));
        assert_eq!(delta.domain, "test");
    }

    #[test]
    fn test_parse_delta_content_modified() {
        let content = "## MODIFIED\n\n### Updated Feature\nChanged behavior";
        let delta = parse_delta_content(content, "test").expect("failed to parse delta");
        assert_eq!(delta.modified.len(), 1);
        assert_eq!(delta.modified[0].heading, "Updated Feature");
    }

    #[test]
    fn test_parse_delta_content_removed() {
        let content = "## REMOVED\n\n### Old Feature\n";
        let delta = parse_delta_content(content, "test").expect("failed to parse delta");
        assert_eq!(delta.removed.len(), 1);
        assert_eq!(delta.removed[0], "Old Feature");
    }

    #[test]
    fn test_parse_delta_content_all_sections() {
        let content = "## ADDED\n\n### A\nContent A\n\n## MODIFIED\n\n### B\nContent B\n\n## REMOVED\n\n### C\n";
        let delta = parse_delta_content(content, "test").expect("failed to parse delta");
        assert_eq!(delta.added.len(), 1);
        assert_eq!(delta.modified.len(), 1);
        assert_eq!(delta.removed.len(), 1);
    }

    #[cfg(all(test, feature = "fs"))]
    mod fs_tests {
        use tempfile::TempDir;

        use super::*;

        #[test]
        fn test_parse_delta_file() {
            let dir = TempDir::new().expect("failed to create temp dir for test");
            let file = dir.path().join("spec.md");
            std::fs::write(&file, "## ADDED\n\n### New Feature\nThe system MUST support this")
                .expect("failed to write delta file");

            let delta = parse_delta_file(&file, dir.path()).expect("failed to parse delta file");
            assert_eq!(delta.added.len(), 1);
            assert_eq!(delta.added[0].heading, "New Feature");
            assert!(delta.added[0].body.contains("MUST"));
        }
    }
}
