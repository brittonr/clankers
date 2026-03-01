//! AI-powered commit tool
//!
//! Provides agentic git analysis:
//! - Hunk-level staging (`git add -p` style)
//! - Split commits (group related changes)
//! - Automatic changelog generation
//! - Conventional commit validation

use std::sync::LazyLock;

use async_trait::async_trait;
use serde_json::Value;
use serde_json::json;

use super::Tool;
use super::ToolContext;
use super::ToolDefinition;
use super::ToolResult;

pub struct CommitTool {
    definition: ToolDefinition,
}

impl Default for CommitTool {
    fn default() -> Self {
        Self::new()
    }
}

impl CommitTool {
    pub fn new() -> Self {
        Self {
            definition: ToolDefinition {
                name: "commit".to_string(),
                description: "AI-powered git commit tool. Analyzes staged/unstaged changes, \
                    generates conventional commit messages, supports hunk-level staging, \
                    split commits, and changelog generation. Actions: 'analyze' (examine working \
                    tree), 'stage' (stage specific files/hunks), 'commit' (create commit with \
                    generated message), 'split' (split changes into logical commits), \
                    'changelog' (generate changelog from recent commits)."
                    .to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "action": {
                            "type": "string",
                            "enum": ["analyze", "stage", "commit", "split", "changelog"],
                            "description": "The action to perform"
                        },
                        "message": {
                            "type": "string",
                            "description": "Commit message (for action='commit'). If empty, one will be generated."
                        },
                        "files": {
                            "type": "array",
                            "items": {"type": "string"},
                            "description": "Specific files to stage or analyze"
                        },
                        "commit_type": {
                            "type": "string",
                            "enum": ["feat", "fix", "docs", "style", "refactor", "perf", "test", "build", "ci", "chore", "revert"],
                            "description": "Conventional commit type prefix"
                        },
                        "scope": {
                            "type": "string",
                            "description": "Conventional commit scope (e.g., 'parser', 'auth')"
                        },
                        "breaking": {
                            "type": "boolean",
                            "description": "Whether this is a breaking change",
                            "default": false
                        },
                        "count": {
                            "type": "integer",
                            "description": "Number of recent commits for changelog (default: 20)",
                            "default": 20
                        }
                    },
                    "required": ["action"]
                }),
            },
        }
    }

    async fn analyze(&self, ctx: &ToolContext, files: &[String]) -> ToolResult {
        // Get git status
        ctx.emit_progress("git status...");
        let status_output = run_git(&["status", "--porcelain"]).await;
        if let Err(e) = &status_output {
            return ToolResult::error(format!("Not a git repository or git error: {}", e));
        }
        // Safe: we already returned on Err above
        let status = status_output.expect("checked above");

        // Get diff (staged + unstaged)
        ctx.emit_progress("git diff --cached --stat...");
        let staged_diff = run_git(&["diff", "--cached", "--stat"]).await.unwrap_or_default();
        ctx.emit_progress("git diff --stat...");
        let unstaged_diff = run_git(&["diff", "--stat"]).await.unwrap_or_default();

        // Get detailed diff for specific files or all
        ctx.emit_progress("reading detailed diffs...");
        let diff_args = if files.is_empty() {
            vec!["diff", "--cached"]
        } else {
            let mut args = vec!["diff", "--cached", "--"];
            for f in files {
                args.push(f.as_str());
            }
            args
        };
        let detailed_diff = run_git(&diff_args).await.unwrap_or_default();

        let unstaged_detail = if files.is_empty() {
            run_git(&["diff"]).await.unwrap_or_default()
        } else {
            let mut args = vec!["diff", "--"];
            for f in files {
                args.push(f.as_str());
            }
            run_git(&args).await.unwrap_or_default()
        };

        // Categorize changes
        ctx.emit_progress("categorizing changes...");
        let analysis = categorize_changes(&status);

        let mut output = String::new();
        output.push_str("# Git Analysis\n\n");
        output.push_str("## Status\n");
        output.push_str(&format!("```\n{}\n```\n\n", status));

        if !staged_diff.is_empty() {
            output.push_str("## Staged Changes\n");
            output.push_str(&format!("```\n{}\n```\n\n", staged_diff));
        }
        if !unstaged_diff.is_empty() {
            output.push_str("## Unstaged Changes\n");
            output.push_str(&format!("```\n{}\n```\n\n", unstaged_diff));
        }

        output.push_str("## Change Categories\n");
        for (category, files_in_cat) in &analysis {
            output.push_str(&format!("- **{}**: {}\n", category, files_in_cat.join(", ")));
        }

        if !detailed_diff.is_empty() {
            // Truncate very long diffs
            let max = 10_000;
            let diff_display = if detailed_diff.len() > max {
                format!("{}...\n[Truncated: {} chars total]", &detailed_diff[..max], detailed_diff.len())
            } else {
                detailed_diff
            };
            output.push_str(&format!("\n## Detailed Staged Diff\n```diff\n{}\n```\n", diff_display));
        }

        if !unstaged_detail.is_empty() {
            let max = 10_000;
            let diff_display = if unstaged_detail.len() > max {
                format!("{}...\n[Truncated: {} chars total]", &unstaged_detail[..max], unstaged_detail.len())
            } else {
                unstaged_detail
            };
            output.push_str(&format!("\n## Detailed Unstaged Diff\n```diff\n{}\n```\n", diff_display));
        }

        // Suggest a commit message
        let suggestion = suggest_commit_message(&status, &analysis);
        output.push_str(&format!("\n## Suggested Commit\n```\n{}\n```\n", suggestion));

        ToolResult::text(output)
    }

    async fn stage(&self, ctx: &ToolContext, files: &[String]) -> ToolResult {
        if files.is_empty() {
            return ToolResult::error("No files specified. Provide 'files' array with paths to stage.");
        }

        let mut staged = Vec::new();
        let mut errors = Vec::new();

        for file in files {
            ctx.emit_progress(&format!("staging: {}", file));
            let args = vec!["add", file.as_str()];
            match run_git(&args).await {
                Ok(_) => staged.push(file.clone()),
                Err(e) => errors.push(format!("{}: {}", file, e)),
            }
        }

        let mut output = String::new();
        if !staged.is_empty() {
            output.push_str(&format!("Staged {} file(s):\n", staged.len()));
            for f in &staged {
                output.push_str(&format!("  ✓ {}\n", f));
            }
        }
        if !errors.is_empty() {
            output.push_str(&format!("\nFailed to stage {} file(s):\n", errors.len()));
            for e in &errors {
                output.push_str(&format!("  ✗ {}\n", e));
            }
        }

        if errors.is_empty() {
            ToolResult::text(output)
        } else {
            ToolResult::error(output)
        }
    }

    async fn commit(
        &self,
        ctx: &ToolContext,
        message: Option<&str>,
        commit_type: Option<&str>,
        scope: Option<&str>,
        breaking: bool,
    ) -> ToolResult {
        // Check for staged changes
        ctx.emit_progress("checking staged changes...");
        let staged = run_git(&["diff", "--cached", "--name-only"]).await.unwrap_or_default();
        if staged.trim().is_empty() {
            return ToolResult::error("No staged changes. Use action='stage' first, or use 'git add' to stage files.");
        }

        // Build commit message
        let msg = if let Some(m) = message.filter(|s| !s.is_empty()) {
            // Apply conventional commit formatting if type specified
            if let Some(ct) = commit_type {
                let scope_part = scope.map(|s| format!("({})", s)).unwrap_or_default();
                let bang = if breaking { "!" } else { "" };
                format!("{}{}{}: {}", ct, scope_part, bang, m)
            } else {
                m.to_string()
            }
        } else {
            // Auto-generate commit message from staged diff
            let status = run_git(&["status", "--porcelain"]).await.unwrap_or_default();
            let analysis = categorize_changes(&status);
            suggest_commit_message(&status, &analysis)
        };

        // Validate conventional commit format
        if let Some(warning) = validate_conventional_commit(&msg) {
            // Warn but don't block
            tracing::debug!("Commit message validation: {}", warning);
        }

        // Create the commit
        ctx.emit_progress(&format!("committing: {}", msg));
        match run_git(&["commit", "-m", &msg]).await {
            Ok(output) => {
                let hash = run_git(&["rev-parse", "--short", "HEAD"]).await.unwrap_or_default();
                ToolResult::text(format!("Committed: {}\n\nMessage:\n  {}\n\n{}", hash.trim(), msg, output))
            }
            Err(e) => ToolResult::error(format!("Commit failed: {}", e)),
        }
    }

    async fn split_analysis(&self, ctx: &ToolContext) -> ToolResult {
        // Analyze unstaged changes and suggest how to split into multiple commits
        ctx.emit_progress("analyzing working tree for split...");
        let status = run_git(&["status", "--porcelain"]).await.unwrap_or_default();
        let analysis = categorize_changes(&status);

        if analysis.is_empty() {
            return ToolResult::text("Working tree is clean — nothing to split.");
        }

        let mut output = String::new();
        output.push_str("# Suggested Commit Split\n\n");
        output.push_str("Based on file categories, these changes could be split into:\n\n");

        for (i, (category, files)) in analysis.iter().enumerate() {
            let commit_type = match category.as_str() {
                "source" | "implementation" => "feat",
                "test" => "test",
                "config" | "build" => "build",
                "docs" | "documentation" => "docs",
                "style" | "formatting" => "style",
                _ => "chore",
            };
            output.push_str(&format!("## Commit {} — `{}({}): ...`\n", i + 1, commit_type, category));
            for f in files {
                output.push_str(&format!("  - {}\n", f));
            }
            output.push('\n');
        }

        output.push_str(
            "\nTo create these commits:\n\
             1. Use action='stage' with the files for each commit\n\
             2. Use action='commit' for each group\n",
        );

        ToolResult::text(output)
    }

    async fn changelog(&self, ctx: &ToolContext, count: usize) -> ToolResult {
        ctx.emit_progress(&format!("reading last {} commits...", count));
        let format_str = "--pretty=format:%h|%s|%an|%ar";
        let count_str = format!("-{}", count);
        let log = run_git(&["log", &count_str, format_str]).await;

        match log {
            Ok(output) => {
                if output.trim().is_empty() {
                    return ToolResult::text("No commits found.");
                }

                let mut changelog = String::new();
                changelog.push_str("# Changelog\n\n");

                // Group by conventional commit type
                let mut groups: std::collections::BTreeMap<String, Vec<ChangelogEntry>> =
                    std::collections::BTreeMap::new();

                for line in output.lines() {
                    let parts: Vec<&str> = line.splitn(4, '|').collect();
                    if parts.len() >= 2 {
                        let hash = parts[0];
                        let subject = parts[1];
                        let author = parts.get(2).copied().unwrap_or("");
                        let date = parts.get(3).copied().unwrap_or("");

                        let (group, desc) = parse_conventional_prefix(subject);
                        groups.entry(group).or_default().push(ChangelogEntry {
                            hash: hash.to_string(),
                            description: desc.to_string(),
                            author: author.to_string(),
                            date: date.to_string(),
                        });
                    }
                }

                let group_labels = [
                    ("feat", "✨ Features"),
                    ("fix", "🐛 Bug Fixes"),
                    ("docs", "📚 Documentation"),
                    ("perf", "⚡ Performance"),
                    ("refactor", "♻️ Refactoring"),
                    ("test", "✅ Tests"),
                    ("build", "📦 Build"),
                    ("ci", "🔧 CI"),
                    ("chore", "🔨 Chores"),
                    ("style", "💅 Style"),
                    ("other", "📋 Other"),
                ];

                for (key, label) in &group_labels {
                    if let Some(entries) = groups.get(*key) {
                        changelog.push_str(&format!("## {}\n\n", label));
                        for entry in entries {
                            changelog.push_str(&format!("- {} ({}, {})\n", entry.description, entry.hash, entry.date));
                        }
                        changelog.push('\n');
                    }
                }

                ToolResult::text(changelog)
            }
            Err(e) => ToolResult::error(format!("Failed to read git log: {}", e)),
        }
    }
}

#[async_trait]
impl Tool for CommitTool {
    fn definition(&self) -> &ToolDefinition {
        &self.definition
    }

    async fn execute(&self, ctx: &ToolContext, params: Value) -> ToolResult {
        let action = params["action"].as_str().unwrap_or("analyze");
        let files: Vec<String> = params["files"]
            .as_array()
            .map(|a| a.iter().filter_map(|v| v.as_str().map(String::from)).collect())
            .unwrap_or_default();

        ctx.emit_progress(&format!("git: {}", action));

        match action {
            "analyze" => self.analyze(ctx, &files).await,
            "stage" => self.stage(ctx, &files).await,
            "commit" => {
                let message = params["message"].as_str();
                let commit_type = params["commit_type"].as_str();
                let scope = params["scope"].as_str();
                let breaking = params["breaking"].as_bool().unwrap_or(false);
                self.commit(ctx, message, commit_type, scope, breaking).await
            }
            "split" => self.split_analysis(ctx).await,
            "changelog" => {
                let count = params["count"].as_u64().unwrap_or(20) as usize;
                self.changelog(ctx, count).await
            }
            _ => {
                ToolResult::error(format!("Unknown action: {}. Use: analyze, stage, commit, split, changelog", action))
            }
        }
    }
}

/// Run a git command and return stdout
async fn run_git(args: &[&str]) -> Result<String, String> {
    let output = tokio::process::Command::new("git")
        .args(args)
        .output()
        .await
        .map_err(|e| format!("Failed to run git: {}", e))?;

    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        Err(stderr.to_string())
    }
}

/// Categorize changed files by directory/type
fn categorize_changes(status: &str) -> Vec<(String, Vec<String>)> {
    let mut categories: std::collections::HashMap<String, Vec<String>> = std::collections::HashMap::new();

    for line in status.lines() {
        let line = line.trim();
        if line.len() < 4 {
            continue;
        }
        let file = &line[3..];
        let category = categorize_file(file);
        categories.entry(category).or_default().push(file.to_string());
    }

    let mut result: Vec<(String, Vec<String>)> = categories.into_iter().collect();
    result.sort_by(|a, b| a.0.cmp(&b.0));
    result
}

fn categorize_file(path: &str) -> String {
    let lower = path.to_lowercase();
    if lower.contains("test") || lower.starts_with("tests/") {
        "test".to_string()
    } else if lower.ends_with(".md") || lower.ends_with(".txt") || lower.starts_with("docs/") {
        "docs".to_string()
    } else if lower.ends_with(".toml")
        || lower.ends_with(".yaml")
        || lower.ends_with(".yml")
        || lower.ends_with(".json")
        || lower.ends_with(".lock")
        || lower == "cargo.toml"
        || lower == "cargo.lock"
    {
        "config".to_string()
    } else if lower.ends_with(".rs")
        || lower.ends_with(".py")
        || lower.ends_with(".ts")
        || lower.ends_with(".js")
        || lower.ends_with(".go")
        || lower.ends_with(".c")
        || lower.ends_with(".cpp")
    {
        "source".to_string()
    } else {
        "other".to_string()
    }
}

/// Generate a commit message suggestion from status and categories
fn suggest_commit_message(_status: &str, categories: &[(String, Vec<String>)]) -> String {
    let total_files: usize = categories.iter().map(|(_, f)| f.len()).sum();
    if total_files == 0 {
        return "chore: no changes detected".to_string();
    }

    // If all changes are in one category, use that as the type
    if categories.len() == 1 {
        let (cat, files) = &categories[0];
        let commit_type = match cat.as_str() {
            "test" => "test",
            "docs" => "docs",
            "config" => "build",
            "source" => "feat",
            _ => "chore",
        };
        if files.len() == 1 {
            return format!("{}: update {}", commit_type, files[0]);
        }
        return format!("{}: update {} files", commit_type, files.len());
    }

    // Mixed changes
    let types: Vec<&str> = categories.iter().map(|(c, _)| c.as_str()).collect();
    format!("chore: update {} files across {}", total_files, types.join(", "))
}

static CONVENTIONAL_PARSE_RE: LazyLock<regex::Regex> = LazyLock::new(|| {
    regex::Regex::new(r"^(feat|fix|docs|style|refactor|perf|test|build|ci|chore|revert)(\([^)]*\))?(!)?: (.*)$")
        .expect("static regex")
});

static CONVENTIONAL_VALIDATE_RE: LazyLock<regex::Regex> = LazyLock::new(|| {
    regex::Regex::new(r"^(feat|fix|docs|style|refactor|perf|test|build|ci|chore|revert)(\([^)]*\))?(!)?: .+$")
        .expect("static regex")
});

fn parse_conventional_prefix(subject: &str) -> (String, &str) {
    // Match patterns like "feat(scope): msg" or "fix: msg" or "feat!: msg"
    if let Some(caps) = CONVENTIONAL_PARSE_RE.captures(subject) {
        let commit_type = caps.get(1).map(|m| m.as_str()).unwrap_or("other");
        let desc_start = caps.get(4).map(|m| m.start()).unwrap_or(0);
        (commit_type.to_string(), &subject[desc_start..])
    } else {
        ("other".to_string(), subject)
    }
}

fn validate_conventional_commit(msg: &str) -> Option<String> {
    let first_line = msg.lines().next().unwrap_or(msg);
    if !CONVENTIONAL_VALIDATE_RE.is_match(first_line) {
        Some(format!(
            "Message does not follow Conventional Commits format. Expected: type(scope): description. Got: {}",
            first_line
        ))
    } else if first_line.len() > 72 {
        Some(format!("First line is {} chars (recommended max: 72)", first_line.len()))
    } else {
        None
    }
}

#[allow(dead_code)]
struct ChangelogEntry {
    hash: String,
    description: String,
    author: String,
    date: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_categorize_file() {
        assert_eq!(categorize_file("src/main.rs"), "source");
        assert_eq!(categorize_file("tests/test_foo.rs"), "test");
        assert_eq!(categorize_file("README.md"), "docs");
        assert_eq!(categorize_file("Cargo.toml"), "config");
        assert_eq!(categorize_file("assets/logo.png"), "other");
    }

    #[test]
    fn test_validate_conventional_commit_valid() {
        assert!(validate_conventional_commit("feat: add web search").is_none());
        assert!(validate_conventional_commit("fix(auth): handle expired tokens").is_none());
        assert!(validate_conventional_commit("feat!: breaking API change").is_none());
    }

    #[test]
    fn test_validate_conventional_commit_invalid() {
        assert!(validate_conventional_commit("added some stuff").is_some());
        assert!(validate_conventional_commit("WIP").is_some());
    }

    #[test]
    fn test_parse_conventional_prefix() {
        let (t, d) = parse_conventional_prefix("feat(auth): add login flow");
        assert_eq!(t, "feat");
        assert_eq!(d, "add login flow");

        let (t, _d) = parse_conventional_prefix("just a message");
        assert_eq!(t, "other");
    }

    #[test]
    fn test_suggest_commit_message_single_file() {
        let cats = vec![("source".to_string(), vec!["src/main.rs".to_string()])];
        let msg = suggest_commit_message("", &cats);
        assert!(msg.starts_with("feat:"));
        assert!(msg.contains("main.rs"));
    }

    #[test]
    fn test_categorize_changes() {
        let status = " M src/main.rs\n M tests/test.rs\n?? README.md\n";
        let cats = categorize_changes(status);
        assert!(cats.len() >= 2);
    }
}
