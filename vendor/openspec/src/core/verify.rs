//! Implementation-to-spec verification
//!
//! Three dimensions: completeness, correctness, coherence.
//! Full implementation requires LLM analysis (Phase 6d+).

use serde::Deserialize;
use serde::Serialize;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Severity {
    Critical,
    Warning,
    Suggestion,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerifyItem {
    pub severity: Severity,
    pub message: String,
    pub context: Option<String>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct VerifyReport {
    pub items: Vec<VerifyItem>,
}

impl VerifyReport {
    pub fn has_critical(&self) -> bool {
        self.items.iter().any(|i| matches!(i.severity, Severity::Critical))
    }

    pub fn summary(&self) -> String {
        let critical = self.items.iter().filter(|i| matches!(i.severity, Severity::Critical)).count();
        let warnings = self.items.iter().filter(|i| matches!(i.severity, Severity::Warning)).count();
        let suggestions = self.items.iter().filter(|i| matches!(i.severity, Severity::Suggestion)).count();
        format!("{} critical, {} warnings, {} suggestions", critical, warnings, suggestions)
    }
}

/// Verify from content strings (pure version)
/// `tasks_content` is the content of tasks.md (if any)
/// `has_specs_dir` indicates if there are delta specs
pub fn verify_from_content(tasks_content: Option<&str>, has_specs_dir: bool) -> VerifyReport {
    let mut report = VerifyReport::default();

    // Check tasks.md completion
    if let Some(content) = tasks_content {
        let total = content.matches("[ ]").count()
            + content.matches("[~]").count()
            + content.matches("[x]").count()
            + content.matches("[X]").count();
        let done = content.matches("[x]").count() + content.matches("[X]").count();
        let wip = content.matches("[~]").count();
        if total > 0 && done < total {
            let mut msg = format!("Tasks incomplete: {}/{} done", done, total);
            if wip > 0 {
                msg.push_str(&format!(", {} in progress", wip));
            }
            report.items.push(VerifyItem {
                severity: Severity::Warning,
                message: msg,
                context: Some("tasks.md".to_string()),
            });
        }
    } else {
        report.items.push(VerifyItem {
            severity: Severity::Suggestion,
            message: "No tasks.md found".to_string(),
            context: None,
        });
    }

    // Check spec files exist in delta
    if !has_specs_dir {
        report.items.push(VerifyItem {
            severity: Severity::Warning,
            message: "No delta specs directory found".to_string(),
            context: None,
        });
    }

    report
}

#[cfg(feature = "fs")]
/// Basic verification: check task completion and file existence.
/// Full LLM-powered verification comes in Phase 6d+.
pub fn verify_basic(change_dir: &std::path::Path) -> VerifyReport {
    // Check tasks.md content
    let tasks_path = change_dir.join("tasks.md");
    let tasks_content = std::fs::read_to_string(&tasks_path).ok();

    // Check specs directory
    let specs_dir = change_dir.join("specs");
    let has_specs_dir = specs_dir.is_dir();

    verify_from_content(tasks_content.as_deref(), has_specs_dir)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_verify_from_content_complete_tasks() {
        let content = "# Tasks\n- [x] Task 1\n- [x] Task 2";
        let report = verify_from_content(Some(content), true);
        assert!(!report.has_critical());
        // Should have no warnings about incomplete tasks
        assert!(!report.items.iter().any(|i| i.message.contains("incomplete")));
    }

    #[test]
    fn test_verify_from_content_incomplete_tasks() {
        let content = "# Tasks\n- [x] Task 1\n- [ ] Task 2";
        let report = verify_from_content(Some(content), true);
        assert!(report.items.iter().any(|i| i.message.contains("incomplete")));
    }

    #[test]
    fn test_verify_from_content_no_tasks() {
        let report = verify_from_content(None, true);
        assert!(report.items.iter().any(|i| i.message.contains("No tasks.md")));
    }

    #[test]
    fn test_verify_from_content_no_specs_dir() {
        let report = verify_from_content(Some("# No tasks"), false);
        assert!(report.items.iter().any(|i| i.message.contains("No delta specs")));
    }
}
