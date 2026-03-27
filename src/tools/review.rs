//! Interactive code review tool (`/review`)
//!
//! Performs structured code review with:
//! - Priority levels (P0-P3)
//! - Category classification (bug, security, performance, style, etc.)
//! - File-level and hunk-level annotations
//! - Verdict rendering (approve, request changes, comment)
//!
//! All git operations are in-process via libgit2 (see `git_ops`).

use async_trait::async_trait;
use serde::Deserialize;
use serde::Serialize;
use serde_json::Value;
use serde_json::json;

use super::Tool;
use super::ToolContext;
use super::ToolDefinition;
use super::ToolResult;
use super::git_ops;

/// Priority levels for review findings
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Priority {
    /// Critical: Must fix before merge (security holes, data loss, crashes)
    P0,
    /// High: Should fix before merge (bugs, correctness issues)
    P1,
    /// Medium: Consider fixing (performance, design concerns)
    P2,
    /// Low: Nice to have (style, naming, minor improvements)
    P3,
}

impl Priority {
    pub fn emoji(&self) -> &'static str {
        match self {
            Priority::P0 => "🔴",
            Priority::P1 => "🟠",
            Priority::P2 => "🟡",
            Priority::P3 => "🟢",
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            Priority::P0 => "CRITICAL",
            Priority::P1 => "HIGH",
            Priority::P2 => "MEDIUM",
            Priority::P3 => "LOW",
        }
    }
}

/// Category of a review finding
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FindingCategory {
    Bug,
    Security,
    Performance,
    Style,
    Design,
    Documentation,
    Testing,
    Error,
    Suggestion,
}

impl FindingCategory {
    pub fn emoji(&self) -> &'static str {
        match self {
            FindingCategory::Bug => "🐛",
            FindingCategory::Security => "🔒",
            FindingCategory::Performance => "⚡",
            FindingCategory::Style => "💅",
            FindingCategory::Design => "🏗️",
            FindingCategory::Documentation => "📚",
            FindingCategory::Testing => "✅",
            FindingCategory::Error => "❌",
            FindingCategory::Suggestion => "💡",
        }
    }
}

/// A single review finding
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Finding {
    pub priority: Priority,
    pub category: FindingCategory,
    pub file: String,
    pub line: Option<u32>,
    pub title: String,
    pub description: String,
    pub suggestion: Option<String>,
}

/// Overall review verdict
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Verdict {
    Approve,
    RequestChanges,
    Comment,
}

impl Verdict {
    pub fn emoji(&self) -> &'static str {
        match self {
            Verdict::Approve => "✅",
            Verdict::RequestChanges => "❌",
            Verdict::Comment => "💬",
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            Verdict::Approve => "APPROVED",
            Verdict::RequestChanges => "CHANGES REQUESTED",
            Verdict::Comment => "COMMENTED",
        }
    }
}

/// Complete review report
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReviewReport {
    pub verdict: Verdict,
    pub summary: String,
    pub findings: Vec<Finding>,
}

impl ReviewReport {
    /// Render the report as a formatted markdown string
    pub fn render(&self) -> String {
        use std::fmt::Write;
        let mut out = String::new();

        // Header
        write!(out, "# Code Review — {} {}\n\n", self.verdict.emoji(), self.verdict.label()).unwrap();
        write!(out, "{}\n\n", self.summary).unwrap();

        if self.findings.is_empty() {
            out.push_str("No findings.\n");
            return out;
        }

        // Stats
        let p0 = self.findings.iter().filter(|f| f.priority == Priority::P0).count();
        let p1 = self.findings.iter().filter(|f| f.priority == Priority::P1).count();
        let p2 = self.findings.iter().filter(|f| f.priority == Priority::P2).count();
        let p3 = self.findings.iter().filter(|f| f.priority == Priority::P3).count();
        write!(out, "**Findings:** {} total — {} P0 {} P1 {} P2 {} P3\n\n", self.findings.len(), p0, p1, p2, p3)
            .unwrap();

        // Group by priority
        let mut sorted = self.findings.clone();
        sorted.sort_by_key(|f| f.priority);

        let mut current_priority: Option<Priority> = None;
        for finding in &sorted {
            if current_priority != Some(finding.priority) {
                current_priority = Some(finding.priority);
                write!(out, "## {} {} Priority\n\n", finding.priority.emoji(), finding.priority.label()).unwrap();
            }

            let location = if let Some(line) = finding.line {
                format!("{}:{}", finding.file, line)
            } else {
                finding.file.clone()
            };

            writeln!(out, "### {} {} — {}", finding.category.emoji(), finding.title, location).unwrap();
            writeln!(out, "{}", finding.description).unwrap();

            if let Some(ref suggestion) = finding.suggestion {
                write!(out, "\n> 💡 **Suggestion:** {}\n", suggestion).unwrap();
            }
            out.push('\n');
        }

        out
    }
}

pub struct ReviewTool {
    definition: ToolDefinition,
}

impl Default for ReviewTool {
    fn default() -> Self {
        Self::new()
    }
}

impl ReviewTool {
    pub fn new() -> Self {
        Self {
            definition: ToolDefinition {
                name: "review".to_string(),
                description: "Submit a structured code review with findings. Each finding has a \
                    priority (P0-P3), category, file location, and description. The tool renders \
                    a formatted review report. Use action='submit' to create a review, or \
                    action='diff' to get the diff for review."
                    .to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "action": {
                            "type": "string",
                            "enum": ["diff", "submit"],
                            "description": "Get the diff to review, or submit findings"
                        },
                        "verdict": {
                            "type": "string",
                            "enum": ["approve", "request_changes", "comment"],
                            "description": "Overall review verdict (for action='submit')"
                        },
                        "summary": {
                            "type": "string",
                            "description": "Brief summary of the review (for action='submit')"
                        },
                        "findings": {
                            "type": "array",
                            "description": "List of findings (for action='submit')",
                            "items": {
                                "type": "object",
                                "properties": {
                                    "priority": {
                                        "type": "string",
                                        "enum": ["P0", "P1", "P2", "P3"]
                                    },
                                    "category": {
                                        "type": "string",
                                        "enum": ["bug", "security", "performance", "style", "design", "documentation", "testing", "error", "suggestion"]
                                    },
                                    "file": {"type": "string"},
                                    "line": {"type": "integer"},
                                    "title": {"type": "string"},
                                    "description": {"type": "string"},
                                    "suggestion": {"type": "string"}
                                },
                                "required": ["priority", "category", "file", "title", "description"]
                            }
                        },
                        "base": {
                            "type": "string",
                            "description": "Base ref for diff (default: HEAD, or main/master)"
                        },
                        "files": {
                            "type": "array",
                            "items": {"type": "string"},
                            "description": "Specific files to diff"
                        }
                    },
                    "required": ["action"]
                }),
            },
        }
    }

    async fn get_diff(&self, ctx: &ToolContext, base: Option<&str>, files: &[String]) -> ToolResult {
        // Determine base ref
        ctx.emit_progress("resolving base ref...");
        let base_ref = if let Some(b) = base {
            b.to_string()
        } else {
            // Try to find a sensible base: main, master, or HEAD~5
            if git_ops::ref_exists("main".to_string()).await {
                "main".to_string()
            } else if git_ops::ref_exists("master".to_string()).await {
                "master".to_string()
            } else {
                "HEAD~5".to_string()
            }
        };

        ctx.emit_progress(&format!("diff: HEAD vs {}...", base_ref));
        let diff = git_ops::diff_ref(base_ref.clone(), files.to_vec()).await.unwrap_or_default();
        ctx.emit_progress("diff --stat...");
        let stat = git_ops::diff_ref_stat(base_ref.clone(), files.to_vec()).await.unwrap_or_default();

        if diff.trim().is_empty() {
            return ToolResult::text(format!("No changes found relative to {}", base_ref));
        }

        ctx.emit_progress(&format!("diff: {} bytes", diff.len()));

        let max = 30_000;
        let display = if diff.len() > max {
            ctx.emit_progress(&format!("truncating: {} → {} chars", diff.len(), max));
            format!("{}...\n\n[Truncated: {} chars total]", &diff[..max], diff.len())
        } else {
            diff
        };

        ToolResult::text(format!(
            "# Diff: HEAD vs {}\n\n## Summary\n```\n{}\n```\n\n## Full Diff\n```diff\n{}\n```",
            base_ref, stat, display
        ))
    }

    fn submit_review(&self, ctx: &ToolContext, params: &Value) -> ToolResult {
        let verdict = match params["verdict"].as_str().unwrap_or("comment") {
            "approve" => Verdict::Approve,
            "request_changes" => Verdict::RequestChanges,
            _ => Verdict::Comment,
        };

        ctx.emit_progress(&format!("{} {}", verdict.emoji(), verdict.label()));

        let summary = params["summary"].as_str().unwrap_or("No summary provided.").to_string();

        let findings: Vec<Finding> = params["findings"]
            .as_array()
            .map(|arr| {
                arr.iter()
                    .filter_map(|f| {
                        Some(Finding {
                            priority: match f["priority"].as_str()? {
                                "P0" => Priority::P0,
                                "P1" => Priority::P1,
                                "P2" => Priority::P2,
                                "P3" => Priority::P3,
                                _ => Priority::P2,
                            },
                            category: serde_json::from_value(f["category"].clone())
                                .unwrap_or(FindingCategory::Suggestion),
                            file: f["file"].as_str()?.to_string(),
                            line: f["line"].as_u64().and_then(|n| u32::try_from(n).ok()),
                            title: f["title"].as_str()?.to_string(),
                            description: f["description"].as_str()?.to_string(),
                            suggestion: f["suggestion"].as_str().map(|s| s.to_string()),
                        })
                    })
                    .collect()
            })
            .unwrap_or_default();

        // Stream each finding as it's processed
        for finding in &findings {
            ctx.emit_progress(&format!(
                "{} {} {} — {}",
                finding.priority.emoji(),
                finding.priority.label(),
                finding.category.emoji(),
                finding.title
            ));
        }

        ctx.emit_progress(&format!("{} findings total", findings.len()));

        let report = ReviewReport {
            verdict,
            summary,
            findings,
        };

        ToolResult::text(report.render())
    }
}

#[async_trait]
impl Tool for ReviewTool {
    fn definition(&self) -> &ToolDefinition {
        &self.definition
    }

    async fn execute(&self, ctx: &ToolContext, params: Value) -> ToolResult {
        let action = params["action"].as_str().unwrap_or("diff");
        match action {
            "diff" => {
                let base = params["base"].as_str();
                let files: Vec<String> = params["files"]
                    .as_array()
                    .map(|a| a.iter().filter_map(|v| v.as_str().map(String::from)).collect())
                    .unwrap_or_default();
                self.get_diff(ctx, base, &files).await
            }
            "submit" => self.submit_review(ctx, &params),
            _ => ToolResult::error(format!("Unknown action: {}. Use 'diff' or 'submit'.", action)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_render_report() {
        let report = ReviewReport {
            verdict: Verdict::RequestChanges,
            summary: "Found some issues.".to_string(),
            findings: vec![
                Finding {
                    priority: Priority::P0,
                    category: FindingCategory::Security,
                    file: "src/auth.rs".to_string(),
                    line: Some(42),
                    title: "SQL Injection".to_string(),
                    description: "User input is not sanitized.".to_string(),
                    suggestion: Some("Use parameterized queries.".to_string()),
                },
                Finding {
                    priority: Priority::P3,
                    category: FindingCategory::Style,
                    file: "src/main.rs".to_string(),
                    line: None,
                    title: "Naming".to_string(),
                    description: "Variable name could be more descriptive.".to_string(),
                    suggestion: None,
                },
            ],
        };

        let rendered = report.render();
        assert!(rendered.contains("CHANGES REQUESTED"));
        assert!(rendered.contains("SQL Injection"));
        assert!(rendered.contains("P0"));
        assert!(rendered.contains("src/auth.rs:42"));
    }

    #[test]
    fn test_priority_ordering() {
        assert!(Priority::P0 < Priority::P1);
        assert!(Priority::P1 < Priority::P2);
        assert!(Priority::P2 < Priority::P3);
    }

    #[test]
    fn test_empty_report() {
        let report = ReviewReport {
            verdict: Verdict::Approve,
            summary: "All good!".to_string(),
            findings: vec![],
        };
        let rendered = report.render();
        assert!(rendered.contains("APPROVED"));
        assert!(rendered.contains("No findings"));
    }
}
