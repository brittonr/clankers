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

/// Basic verification: check task completion and file existence.
/// Full LLM-powered verification comes in Phase 6d+.
pub fn verify_basic(change_dir: &std::path::Path) -> VerifyReport {
    let mut report = VerifyReport::default();

    // Check tasks.md completion
    let tasks_path = change_dir.join("tasks.md");
    if let Ok(content) = std::fs::read_to_string(&tasks_path) {
        let total = content.matches("[ ]").count() + content.matches("[x]").count() + content.matches("[X]").count();
        let done = content.matches("[x]").count() + content.matches("[X]").count();
        if total > 0 && done < total {
            report.items.push(VerifyItem {
                severity: Severity::Warning,
                message: format!("Tasks incomplete: {}/{} done", done, total),
                context: Some(tasks_path.display().to_string()),
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
    let specs_dir = change_dir.join("specs");
    if !specs_dir.is_dir() {
        report.items.push(VerifyItem {
            severity: Severity::Warning,
            message: "No delta specs directory found".to_string(),
            context: None,
        });
    }

    report
}
