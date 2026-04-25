//! AI-powered commit tool
//!
//! Provides agentic git analysis:
//! - Hunk-level staging (`git add -p` style)
//! - Split commits (group related changes)
//! - Automatic changelog generation
//! - Conventional commit validation
//!
//! All git operations are in-process via libgit2 (see `git_ops`).

#![allow(unexpected_cfgs)]
#![cfg_attr(dylint_lib = "tigerstyle", allow(compound_assertion, ignored_result, no_unwrap, no_panic, no_todo, unjustified_no_todo_allow, no_recursion, unchecked_narrowing, unchecked_division, unbounded_loop, catch_all_on_enum, explicit_defaults, unbounded_channel, unbounded_collection_growth, assertion_density, raw_arithmetic_overflow, sentinel_fallback, acronym_style, bool_naming, negated_predicate, numeric_units, float_for_currency, function_length, nested_conditionals, platform_dependent_cast, usize_in_public_api, too_many_parameters, compound_condition, unjustified_allow, ambiguous_params, ambient_clock, verified_purity, contradictory_time, multi_lock_ordering, reason = "full workspace tigerstyle audit gate: legacy debt documented locally while cleanup proceeds incrementally"))]

use std::sync::LazyLock;

use async_trait::async_trait;
use serde_json::Value;
use serde_json::json;

use super::Tool;
use super::ToolContext;
use super::ToolDefinition;
use super::ToolResult;
use super::git_ops;

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
        use std::fmt::Write;

        // Get git status
        ctx.emit_progress("git status...");
        let status = match git_ops::status_porcelain().await {
            Ok(s) => s,
            Err(e) => return ToolResult::error(format!("Not a git repository or git error: {}", e)),
        };

        // Get diff stats (staged + unstaged)
        ctx.emit_progress("staged diff stat...");
        let staged_stat = git_ops::diff_staged_stat(files.to_vec()).await.unwrap_or_default();
        ctx.emit_progress("unstaged diff stat...");
        let unstaged_stat = git_ops::diff_unstaged_stat(files.to_vec()).await.unwrap_or_default();

        // Get detailed diffs
        ctx.emit_progress("reading detailed diffs...");
        let detailed_staged = git_ops::diff_staged(files.to_vec()).await.unwrap_or_default();
        let detailed_unstaged = git_ops::diff_unstaged(files.to_vec()).await.unwrap_or_default();

        // Categorize changes
        ctx.emit_progress("categorizing changes...");
        let analysis = categorize_changes(&status);

        let mut output = String::new();
        output.push_str("# Git Analysis\n\n");
        output.push_str("## Status\n");
        write!(output, "```\n{}\n```\n\n", status).ok();

        if !staged_stat.is_empty() {
            output.push_str("## Staged Changes\n");
            write!(output, "```\n{}\n```\n\n", staged_stat).ok();
        }
        if !unstaged_stat.is_empty() {
            output.push_str("## Unstaged Changes\n");
            write!(output, "```\n{}\n```\n\n", unstaged_stat).ok();
        }

        output.push_str("## Change Categories\n");
        for (category, files_in_cat) in &analysis {
            writeln!(output, "- **{}**: {}", category, files_in_cat.join(", ")).ok();
        }

        if !detailed_staged.is_empty() {
            // Truncate very long diffs
            let max = 10_000;
            let diff_display = if detailed_staged.len() > max {
                format!("{}...\n[Truncated: {} chars total]", &detailed_staged[..max], detailed_staged.len())
            } else {
                detailed_staged
            };
            write!(output, "\n## Detailed Staged Diff\n```diff\n{}\n```\n", diff_display).ok();
        }

        if !detailed_unstaged.is_empty() {
            let max = 10_000;
            let diff_display = if detailed_unstaged.len() > max {
                format!("{}...\n[Truncated: {} chars total]", &detailed_unstaged[..max], detailed_unstaged.len())
            } else {
                detailed_unstaged
            };
            write!(output, "\n## Detailed Unstaged Diff\n```diff\n{}\n```\n", diff_display).ok();
        }

        // Suggest a commit message
        let suggestion = suggest_commit_message(&status, &analysis);
        write!(output, "\n## Suggested Commit\n```\n{}\n```\n", suggestion).ok();

        ToolResult::text(output)
    }

    async fn stage(&self, ctx: &ToolContext, files: &[String]) -> ToolResult {
        use std::fmt::Write;

        if files.is_empty() {
            return ToolResult::error("No files specified. Provide 'files' array with paths to stage.");
        }

        for file in files {
            ctx.emit_progress(&format!("staging: {}", file));
        }

        let (staged, errors) = git_ops::stage_files(files.to_vec()).await;

        let mut output = String::new();
        if !staged.is_empty() {
            writeln!(output, "Staged {} file(s):", staged.len()).ok();
            for f in &staged {
                writeln!(output, "  ✓ {}", f).ok();
            }
        }
        if !errors.is_empty() {
            write!(output, "\nFailed to stage {} file(s):\n", errors.len()).ok();
            for e in &errors {
                writeln!(output, "  ✗ {}", e).ok();
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
        is_breaking: bool,
    ) -> ToolResult {
        // Check for staged changes
        ctx.emit_progress("checking staged changes...");
        let staged_names = git_ops::staged_file_names().await.unwrap_or_default();
        if staged_names.is_empty() {
            return ToolResult::error("No staged changes. Use action='stage' first, or use 'git add' to stage files.");
        }

        // Build commit message
        let msg = if let Some(m) = message.filter(|s| !s.is_empty()) {
            // Apply conventional commit formatting if type specified
            if let Some(ct) = commit_type {
                let scope_part = scope.map(|s| format!("({})", s)).unwrap_or_default();
                let bang = if is_breaking { "!" } else { "" };
                format!("{}{}{}: {}", ct, scope_part, bang, m)
            } else {
                m.to_string()
            }
        } else {
            // Auto-generate commit message from status
            let status = git_ops::status_porcelain().await.unwrap_or_default();
            let analysis = categorize_changes(&status);
            suggest_commit_message(&status, &analysis)
        };

        // Validate conventional commit format
        if let Some(warning) = validate_conventional_commit(&msg) {
            tracing::debug!("Commit message validation: {}", warning);
        }

        // Fire pre-commit hook (can deny the commit)
        if let Some(pipeline) = ctx.hook_pipeline() {
            let payload = clankers_hooks::HookPayload::git(
                "pre-commit",
                ctx.session_id(),
                "commit",
                None,
                Some(&msg),
                staged_names.clone(),
            );
            if let clankers_hooks::HookVerdict::Deny { reason } =
                pipeline.fire(clankers_hooks::HookPoint::PreCommit, &payload).await
            {
                return ToolResult::error(format!("🪝 Pre-commit hook denied: {reason}"));
            }
        }

        // Create the commit
        ctx.emit_progress(&format!("committing: {}", msg));
        match git_ops::commit(msg.clone()).await {
            Ok(result) => {
                // Fire post-commit hook (async, fire-and-forget)
                if let Some(pipeline) = ctx.hook_pipeline() {
                    let payload = clankers_hooks::HookPayload::git(
                        "post-commit",
                        ctx.session_id(),
                        "commit",
                        Some(&result.short_hash),
                        Some(&msg),
                        staged_names,
                    );
                    pipeline.fire_async(clankers_hooks::HookPoint::PostCommit, payload);
                }

                ToolResult::text(format!(
                    "Committed: {}\n\nMessage:\n  {}\n\n{}",
                    result.short_hash, msg, result.summary
                ))
            }
            Err(e) => ToolResult::error(format!("Commit failed: {}", e)),
        }
    }

    async fn split_analysis(&self, ctx: &ToolContext) -> ToolResult {
        use std::fmt::Write;

        // Analyze unstaged changes and suggest how to split into multiple commits
        ctx.emit_progress("analyzing working tree for split...");
        let status = git_ops::status_porcelain().await.unwrap_or_default();
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
            writeln!(output, "## Commit {} — `{}({}): ...`", i + 1, commit_type, category).ok();
            for f in files {
                writeln!(output, "  - {}", f).ok();
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
        use std::fmt::Write;

        ctx.emit_progress(&format!("reading last {} commits...", count));
        match git_ops::log(count).await {
            Ok(entries) => {
                if entries.is_empty() {
                    return ToolResult::text("No commits found.");
                }

                let mut changelog = String::new();
                changelog.push_str("# Changelog\n\n");

                // Group by conventional commit type
                let mut groups: std::collections::BTreeMap<String, Vec<ChangelogEntry>> =
                    std::collections::BTreeMap::new();

                for entry in &entries {
                    let (group, desc) = parse_conventional_prefix(&entry.subject);
                    groups.entry(group).or_default().push(ChangelogEntry {
                        hash: entry.short_hash.clone(),
                        description: desc.to_string(),
                        _author: entry.author.clone(),
                        date: entry.relative_time.clone(),
                    });
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
                        write!(changelog, "## {}\n\n", label).ok();
                        for entry in entries {
                            writeln!(changelog, "- {} ({}, {})", entry.description, entry.hash, entry.date).ok();
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
                let is_breaking = params["breaking"].as_bool().unwrap_or(false);
                self.commit(ctx, message, commit_type, scope, is_breaking).await
            }
            "split" => self.split_analysis(ctx).await,
            "changelog" => {
                let count = usize::try_from(params["count"].as_u64().unwrap_or(20)).unwrap_or(20);
                self.changelog(ctx, count).await
            }
            _ => {
                ToolResult::error(format!("Unknown action: {}. Use: analyze, stage, commit, split, changelog", action))
            }
        }
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

#[cfg_attr(
    dylint_lib = "tigerstyle",
    allow(no_unwrap, reason = "compile-time constant regex pattern")
)]
static CONVENTIONAL_PARSE_RE: LazyLock<regex::Regex> = LazyLock::new(|| {
    regex::Regex::new(r"^(feat|fix|docs|style|refactor|perf|test|build|ci|chore|revert)(\([^)]*\))?(!)?: (.*)$")
        .expect("static regex")
});

static CONVENTIONAL_VALIDATE_RE: LazyLock<regex::Regex> = LazyLock::new(|| {
    #[cfg_attr(
        dylint_lib = "tigerstyle",
        allow(no_unwrap, reason = "compile-time constant regex pattern")
    )]
    regex::Regex::new(r"^(feat|fix|docs|style|refactor|perf|test|build|ci|chore|revert)(\([^)]*\))?(!)?: .+$")
        .expect("static regex")
});

fn parse_conventional_prefix(subject: &str) -> (String, &str) {
    if let Some(caps) = CONVENTIONAL_PARSE_RE.captures(subject) {
        let commit_type = caps.get(1).map(|m| m.as_str()).unwrap_or("other");
        let desc_start = caps.get(4).map(|m| m.start()).unwrap_or(0);
        (commit_type.to_string(), &subject[desc_start..])
    } else {
        ("other".to_string(), subject)
    }
}

/// Maximum recommended length for commit message first line
const MAX_COMMIT_SUBJECT_LEN: usize = 72;

fn validate_conventional_commit(msg: &str) -> Option<String> {
    let first_line = msg.lines().next().unwrap_or(msg);
    if !CONVENTIONAL_VALIDATE_RE.is_match(first_line) {
        Some(format!(
            "Message does not follow Conventional Commits format. Expected: type(scope): description. Got: {}",
            first_line
        ))
    } else if first_line.len() > MAX_COMMIT_SUBJECT_LEN {
        Some(format!("First line is {} chars (recommended max: {})", first_line.len(), MAX_COMMIT_SUBJECT_LEN))
    } else {
        None
    }
}

struct ChangelogEntry {
    hash: String,
    description: String,
    _author: String,
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
