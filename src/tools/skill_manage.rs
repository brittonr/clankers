//! Skill management tool — lets the agent create, update, and delete skills
//!
//! Skills are written to ~/.clankers/agent/skills/<name>/SKILL.md.
//! Actions: create, patch, edit, delete, write_file, list.

use std::fmt::Write;
use std::path::PathBuf;

use async_trait::async_trait;
use serde_json::{Value, json};

use super::{Tool, ToolContext, ToolDefinition, ToolResult};

pub struct SkillManageTool {
    definition: ToolDefinition,
    global_skills_dir: PathBuf,
}

/// Characters allowed in skill names: lowercase alphanumeric, hyphens, underscores.
fn is_valid_skill_name(name: &str) -> bool {
    !name.is_empty()
        && name
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '_')
}

/// Reject paths containing `..` or absolute paths.
fn is_safe_relative_path(path: &str) -> bool {
    !path.is_empty()
        && !path.starts_with('/')
        && !path.contains("..")
        && !path.contains('\\')
}

impl SkillManageTool {
    pub fn new(global_skills_dir: PathBuf) -> Self {
        Self {
            global_skills_dir,
            definition: ToolDefinition {
                name: "skill_manage".to_string(),
                description: "Create, update, and delete reusable skills from experience. \
                    Skills are markdown files loaded into the system prompt for future sessions.\n\n\
                    Actions:\n\
                    - create: Create a new skill (SKILL.md)\n\
                    - patch: Targeted edit (find/replace in SKILL.md)\n\
                    - edit: Full rewrite of SKILL.md\n\
                    - delete: Remove a skill\n\
                    - write_file: Add supporting files (references, templates)\n\
                    - list: Show installed skills\n\
                    - stats: Show usage stats (load count, success/correction rates)\n\
                    - log_outcome: Record the outcome of a skill usage (success/correction/failure)"
                    .to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "action": {
                            "type": "string",
                            "enum": ["create", "patch", "edit", "delete", "write_file", "list", "stats", "log_outcome"],
                            "description": "Action to perform"
                        },
                        "outcome": {
                            "type": "string",
                            "enum": ["success", "correction", "failure"],
                            "description": "Outcome of skill usage (log_outcome action)"
                        },
                        "note": {
                            "type": "string",
                            "description": "Optional note about the outcome (log_outcome action)"
                        },
                        "name": {
                            "type": "string",
                            "description": "Skill name (kebab-case, e.g. 'deploy-k8s')"
                        },
                        "content": {
                            "type": "string",
                            "description": "Full SKILL.md content (create/edit)"
                        },
                        "old_text": {
                            "type": "string",
                            "description": "Exact text to find (patch)"
                        },
                        "new_text": {
                            "type": "string",
                            "description": "Replacement text (patch)"
                        },
                        "file_path": {
                            "type": "string",
                            "description": "Relative path within the skill dir (write_file)"
                        },
                        "file_content": {
                            "type": "string",
                            "description": "Content for the file (write_file)"
                        }
                    },
                    "required": ["action"]
                }),
            },
        }
    }

    fn skill_dir(&self, name: &str) -> PathBuf {
        self.global_skills_dir.join(name)
    }

    fn skill_md_path(&self, name: &str) -> PathBuf {
        self.skill_dir(name).join("SKILL.md")
    }

    fn validate_name(name: &str) -> std::result::Result<(), ToolResult> {
        if !is_valid_skill_name(name) {
            return Err(ToolResult::error(
                "Invalid skill name. Use lowercase alphanumeric characters, hyphens, and underscores only \
                 (e.g. 'deploy-k8s', 'rust_testing').",
            ));
        }
        Ok(())
    }

    fn handle_create(&self, params: &Value) -> ToolResult {
        let name = match params.get("name").and_then(|v| v.as_str()) {
            Some(n) => n,
            None => return ToolResult::error("Missing required 'name' parameter."),
        };
        if let Err(e) = Self::validate_name(name) {
            return e;
        }

        let content = match params.get("content").and_then(|v| v.as_str()) {
            Some(c) if !c.is_empty() => c,
            _ => return ToolResult::error("Missing required 'content' parameter."),
        };

        let skill_path = self.skill_md_path(name);
        if skill_path.exists() {
            return ToolResult::error(format!(
                "Skill '{name}' already exists at {}. Use 'patch' or 'edit' to modify it.",
                skill_path.display()
            ));
        }

        let dir = self.skill_dir(name);
        if let Err(e) = std::fs::create_dir_all(&dir) {
            return ToolResult::error(format!("Failed to create directory {}: {e}", dir.display()));
        }
        if let Err(e) = std::fs::write(&skill_path, content) {
            return ToolResult::error(format!("Failed to write {}: {e}", skill_path.display()));
        }

        ToolResult::text(format!("Created skill '{name}' at {}", skill_path.display()))
    }

    fn handle_patch(&self, params: &Value) -> ToolResult {
        let name = match params.get("name").and_then(|v| v.as_str()) {
            Some(n) => n,
            None => return ToolResult::error("Missing required 'name' parameter."),
        };
        if let Err(e) = Self::validate_name(name) {
            return e;
        }

        let old_text = match params.get("old_text").and_then(|v| v.as_str()) {
            Some(t) if !t.is_empty() => t,
            _ => return ToolResult::error("Missing required 'old_text' parameter."),
        };
        let new_text = match params.get("new_text").and_then(|v| v.as_str()) {
            Some(t) => t,
            None => return ToolResult::error("Missing required 'new_text' parameter."),
        };

        let skill_path = self.skill_md_path(name);
        let current = match std::fs::read_to_string(&skill_path) {
            Ok(c) => c,
            Err(_) => {
                return ToolResult::error(format!("Skill '{name}' not found at {}", skill_path.display()));
            }
        };

        if !current.contains(old_text) {
            let preview = if current.len() > 300 {
                format!("{}...", &current[..300])
            } else {
                current
            };
            return ToolResult::error(format!(
                "old_text not found in skill '{name}'.\n\nCurrent content preview:\n{preview}"
            ));
        }

        let updated = current.replacen(old_text, new_text, 1);
        if let Err(e) = std::fs::write(&skill_path, &updated) {
            return ToolResult::error(format!("Failed to write {}: {e}", skill_path.display()));
        }

        ToolResult::text(format!("Patched skill '{name}'."))
    }

    fn handle_edit(&self, params: &Value) -> ToolResult {
        let name = match params.get("name").and_then(|v| v.as_str()) {
            Some(n) => n,
            None => return ToolResult::error("Missing required 'name' parameter."),
        };
        if let Err(e) = Self::validate_name(name) {
            return e;
        }

        let content = match params.get("content").and_then(|v| v.as_str()) {
            Some(c) if !c.is_empty() => c,
            _ => return ToolResult::error("Missing required 'content' parameter."),
        };

        let skill_path = self.skill_md_path(name);
        if !skill_path.exists() {
            return ToolResult::error(format!("Skill '{name}' not found. Use 'create' first."));
        }

        if let Err(e) = std::fs::write(&skill_path, content) {
            return ToolResult::error(format!("Failed to write {}: {e}", skill_path.display()));
        }

        ToolResult::text(format!("Replaced skill '{name}' content."))
    }

    fn handle_delete(&self, params: &Value) -> ToolResult {
        let name = match params.get("name").and_then(|v| v.as_str()) {
            Some(n) => n,
            None => return ToolResult::error("Missing required 'name' parameter."),
        };
        if let Err(e) = Self::validate_name(name) {
            return e;
        }

        let dir = self.skill_dir(name);
        if !dir.exists() {
            return ToolResult::error(format!("Skill '{name}' not found."));
        }

        if let Err(e) = std::fs::remove_dir_all(&dir) {
            return ToolResult::error(format!("Failed to remove {}: {e}", dir.display()));
        }

        ToolResult::text(format!("Deleted skill '{name}'."))
    }

    fn handle_write_file(&self, params: &Value) -> ToolResult {
        let name = match params.get("name").and_then(|v| v.as_str()) {
            Some(n) => n,
            None => return ToolResult::error("Missing required 'name' parameter."),
        };
        if let Err(e) = Self::validate_name(name) {
            return e;
        }

        let file_path = match params.get("file_path").and_then(|v| v.as_str()) {
            Some(p) if !p.is_empty() => p,
            _ => return ToolResult::error("Missing required 'file_path' parameter."),
        };
        if !is_safe_relative_path(file_path) {
            return ToolResult::error(
                "Invalid file_path. Must be a relative path without '..' or absolute components.",
            );
        }

        let file_content = match params.get("file_content").and_then(|v| v.as_str()) {
            Some(c) => c,
            None => return ToolResult::error("Missing required 'file_content' parameter."),
        };

        let dir = self.skill_dir(name);
        if !dir.exists() {
            return ToolResult::error(format!("Skill '{name}' not found. Create it first."));
        }

        let target = dir.join(file_path);
        if let Some(parent) = target.parent()
            && let Err(e) = std::fs::create_dir_all(parent) {
                return ToolResult::error(format!("Failed to create directory: {e}"));
            }
        if let Err(e) = std::fs::write(&target, file_content) {
            return ToolResult::error(format!("Failed to write {}: {e}", target.display()));
        }

        ToolResult::text(format!("Wrote {} to skill '{name}'.", file_path))
    }

    fn handle_list(&self) -> ToolResult {
        let skills = clankers_skills::scan_skills_dir(&self.global_skills_dir);
        if skills.is_empty() {
            return ToolResult::text("No skills installed.".to_string());
        }

        let mut out = format!("{} skill(s):\n", skills.len());
        for s in &skills {
            writeln!(out, "- **{}**: {}", s.name, s.description).ok();
        }
        ToolResult::text(out)
    }

    fn handle_stats(&self, db: &clankers_db::Db, params: &Value) -> ToolResult {
        let name = params.get("name").and_then(|v| v.as_str());

        if let Some(skill_name) = name {
            let stats = match db.skill_usage().stats_for(skill_name) {
                Ok(s) => s,
                Err(e) => return ToolResult::error(format!("Failed to query stats: {e}")),
            };
            if stats.total_loads == 0 {
                return ToolResult::text(format!("No usage data for skill '{skill_name}'."));
            }
            format_single_stats(&stats)
        } else {
            let all = match db.skill_usage().all_stats() {
                Ok(s) => s,
                Err(e) => return ToolResult::error(format!("Failed to query stats: {e}")),
            };
            if all.is_empty() {
                return ToolResult::text("No skill usage data recorded yet.".to_string());
            }
            let mut out = format!("Skill usage stats ({} skill(s)):\n\n", all.len());
            out.push_str("Skill                    | Loads | Success | Corrections | Rate\n");
            out.push_str("-------------------------|-------|---------|-------------|------\n");
            for s in &all {
                writeln!(out, "{:<24} | {:>5} | {:>7} | {:>11} | {:>4.0}%",
                    s.skill_name,
                    s.total_loads,
                    s.successes,
                    s.corrections,
                    s.success_rate()).ok();
            }
            ToolResult::text(out)
        }
    }

    fn handle_log_outcome(&self, db: &clankers_db::Db, params: &Value) -> ToolResult {
        let name = match params.get("name").and_then(|v| v.as_str()) {
            Some(n) => n,
            None => return ToolResult::error("Missing required 'name' parameter."),
        };
        let outcome_str = match params.get("outcome").and_then(|v| v.as_str()) {
            Some(o) => o,
            None => return ToolResult::error("Missing required 'outcome' parameter (success/correction/failure)."),
        };
        let outcome = match outcome_str {
            "success" => clankers_db::skill_usage::SkillOutcome::Success,
            "correction" => clankers_db::skill_usage::SkillOutcome::Correction,
            "failure" => clankers_db::skill_usage::SkillOutcome::Failure,
            other => return ToolResult::error(format!("Unknown outcome '{other}'. Use success, correction, or failure.")),
        };
        let note = params.get("note").and_then(|v| v.as_str()).map(String::from);

        // Find the most recent pending entry for this skill
        let entries = match db.skill_usage().entries_for(name) {
            Ok(e) => e,
            Err(e) => return ToolResult::error(format!("Failed to query usage: {e}")),
        };

        let pending = entries
            .iter()
            .find(|e| e.outcome == clankers_db::skill_usage::SkillOutcome::Pending);

        match pending {
            Some(entry) => {
                if let Err(e) = db.skill_usage().set_outcome(entry.id, outcome.clone(), note) {
                    return ToolResult::error(format!("Failed to update outcome: {e}"));
                }
                ToolResult::text(format!("Recorded outcome '{outcome}' for skill '{name}'."))
            }
            None => {
                // No pending entry — record a new one with the outcome directly
                let id = match db.skill_usage().record_load(name, "unknown") {
                    Ok(id) => id,
                    Err(e) => return ToolResult::error(format!("Failed to record: {e}")),
                };
                if let Err(e) = db.skill_usage().set_outcome(id, outcome.clone(), note) {
                    return ToolResult::error(format!("Failed to set outcome: {e}"));
                }
                ToolResult::text(format!("Recorded outcome '{outcome}' for skill '{name}'."))
            }
        }
    }
}

fn format_single_stats(stats: &clankers_db::skill_usage::SkillStats) -> ToolResult {
    let last = stats
        .last_used
        .map(|d| d.format("%Y-%m-%d").to_string())
        .unwrap_or_else(|| "never".into());

    let mut out = format!("**{}** usage stats:\n", stats.skill_name);
    writeln!(out, "  Total loads: {}", stats.total_loads).ok();
    writeln!(out, "  Successes: {}", stats.successes).ok();
    writeln!(out, "  Corrections: {}", stats.corrections).ok();
    writeln!(out, "  Failures: {}", stats.failures).ok();
    writeln!(out, "  Pending: {}", stats.pending).ok();
    writeln!(out, "  Success rate: {:.0}%", stats.success_rate()).ok();
    writeln!(out, "  Correction rate: {:.0}%", stats.correction_rate()).ok();
    writeln!(out, "  Last used: {}", last).ok();

    if stats.correction_rate() > 30.0 {
        out.push_str("\n⚠ High correction rate — consider revising this skill.");
    }

    ToolResult::text(out)
}

#[async_trait]
impl Tool for SkillManageTool {
    fn definition(&self) -> &ToolDefinition {
        &self.definition
    }

    async fn execute(&self, ctx: &ToolContext, params: Value) -> ToolResult {
        let action = params.get("action").and_then(|v| v.as_str()).unwrap_or("");

        match action {
            "create" => self.handle_create(&params),
            "patch" => self.handle_patch(&params),
            "edit" => self.handle_edit(&params),
            "delete" => self.handle_delete(&params),
            "write_file" => self.handle_write_file(&params),
            "list" => self.handle_list(),
            "stats" => {
                let db = match ctx.db() {
                    Some(db) => db,
                    None => return ToolResult::error("Stats require a database connection."),
                };
                self.handle_stats(db, &params)
            }
            "log_outcome" => {
                let db = match ctx.db() {
                    Some(db) => db,
                    None => return ToolResult::error("Logging outcomes requires a database connection."),
                };
                self.handle_log_outcome(db, &params)
            }
            other => ToolResult::error(format!(
                "Unknown action '{other}'. Use 'create', 'patch', 'edit', 'delete', 'write_file', 'list', 'stats', or 'log_outcome'."
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use tempfile::TempDir;
    use tokio_util::sync::CancellationToken;

    use super::*;

    fn make_ctx() -> ToolContext {
        ToolContext::new("test".to_string(), CancellationToken::new(), None)
    }

    fn result_text(result: &ToolResult) -> String {
        result
            .content
            .iter()
            .filter_map(|c| match c {
                crate::tools::ToolResultContent::Text { text } => Some(text.as_str()),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("")
    }

    #[tokio::test]
    async fn test_create_and_verify() {
        let tmp = TempDir::new().unwrap();
        let tool = SkillManageTool::new(tmp.path().to_path_buf());
        let ctx = make_ctx();

        let result = tool
            .execute(
                &ctx,
                json!({"action": "create", "name": "test-skill", "content": "# Test\nSome content"}),
            )
            .await;
        assert!(!result.is_error);
        assert!(result_text(&result).contains("Created skill"));

        let skill_path = tmp.path().join("test-skill/SKILL.md");
        assert!(skill_path.exists());
        assert_eq!(std::fs::read_to_string(&skill_path).unwrap(), "# Test\nSome content");
    }

    #[tokio::test]
    async fn test_create_already_exists() {
        let tmp = TempDir::new().unwrap();
        let tool = SkillManageTool::new(tmp.path().to_path_buf());
        let ctx = make_ctx();

        tool.execute(
            &ctx,
            json!({"action": "create", "name": "dup", "content": "first"}),
        )
        .await;

        let result = tool
            .execute(
                &ctx,
                json!({"action": "create", "name": "dup", "content": "second"}),
            )
            .await;
        assert!(result.is_error);
        assert!(result_text(&result).contains("already exists"));
    }

    #[tokio::test]
    async fn test_invalid_name() {
        let tmp = TempDir::new().unwrap();
        let tool = SkillManageTool::new(tmp.path().to_path_buf());
        let ctx = make_ctx();

        let result = tool
            .execute(
                &ctx,
                json!({"action": "create", "name": "Bad Name!", "content": "x"}),
            )
            .await;
        assert!(result.is_error);
        assert!(result_text(&result).contains("Invalid skill name"));
    }

    #[tokio::test]
    async fn test_patch() {
        let tmp = TempDir::new().unwrap();
        let tool = SkillManageTool::new(tmp.path().to_path_buf());
        let ctx = make_ctx();

        tool.execute(
            &ctx,
            json!({"action": "create", "name": "patchme", "content": "use kubectl apply"}),
        )
        .await;

        let result = tool
            .execute(
                &ctx,
                json!({"action": "patch", "name": "patchme", "old_text": "kubectl apply", "new_text": "kubectl apply --server-side"}),
            )
            .await;
        assert!(!result.is_error);

        let content = std::fs::read_to_string(tmp.path().join("patchme/SKILL.md")).unwrap();
        assert!(content.contains("--server-side"));
    }

    #[tokio::test]
    async fn test_patch_missing_old_text() {
        let tmp = TempDir::new().unwrap();
        let tool = SkillManageTool::new(tmp.path().to_path_buf());
        let ctx = make_ctx();

        tool.execute(
            &ctx,
            json!({"action": "create", "name": "nopatch", "content": "hello world"}),
        )
        .await;

        let result = tool
            .execute(
                &ctx,
                json!({"action": "patch", "name": "nopatch", "old_text": "nonexistent", "new_text": "x"}),
            )
            .await;
        assert!(result.is_error);
        assert!(result_text(&result).contains("not found"));
    }

    #[tokio::test]
    async fn test_delete() {
        let tmp = TempDir::new().unwrap();
        let tool = SkillManageTool::new(tmp.path().to_path_buf());
        let ctx = make_ctx();

        tool.execute(
            &ctx,
            json!({"action": "create", "name": "deleteme", "content": "x"}),
        )
        .await;
        assert!(tmp.path().join("deleteme").exists());

        let result = tool
            .execute(&ctx, json!({"action": "delete", "name": "deleteme"}))
            .await;
        assert!(!result.is_error);
        assert!(!tmp.path().join("deleteme").exists());
    }

    #[tokio::test]
    async fn test_delete_nonexistent() {
        let tmp = TempDir::new().unwrap();
        let tool = SkillManageTool::new(tmp.path().to_path_buf());
        let ctx = make_ctx();

        let result = tool
            .execute(&ctx, json!({"action": "delete", "name": "ghost"}))
            .await;
        assert!(result.is_error);
    }

    #[tokio::test]
    async fn test_path_traversal_blocked() {
        let tmp = TempDir::new().unwrap();
        let tool = SkillManageTool::new(tmp.path().to_path_buf());
        let ctx = make_ctx();

        tool.execute(
            &ctx,
            json!({"action": "create", "name": "traverse", "content": "x"}),
        )
        .await;

        let result = tool
            .execute(
                &ctx,
                json!({"action": "write_file", "name": "traverse", "file_path": "../../../etc/passwd", "file_content": "bad"}),
            )
            .await;
        assert!(result.is_error);
        assert!(result_text(&result).contains("Invalid file_path"));
    }

    #[tokio::test]
    async fn test_write_file() {
        let tmp = TempDir::new().unwrap();
        let tool = SkillManageTool::new(tmp.path().to_path_buf());
        let ctx = make_ctx();

        tool.execute(
            &ctx,
            json!({"action": "create", "name": "with-ref", "content": "# Skill"}),
        )
        .await;

        let result = tool
            .execute(
                &ctx,
                json!({"action": "write_file", "name": "with-ref", "file_path": "references/notes.md", "file_content": "Some notes"}),
            )
            .await;
        assert!(!result.is_error);
        assert!(tmp.path().join("with-ref/references/notes.md").exists());
    }

    #[tokio::test]
    async fn test_list_after_create() {
        let tmp = TempDir::new().unwrap();
        let tool = SkillManageTool::new(tmp.path().to_path_buf());
        let ctx = make_ctx();

        // Create a skill with proper frontmatter so scan_skills_dir picks it up
        tool.execute(
            &ctx,
            json!({"action": "create", "name": "my-skill", "content": "---\nname: my-skill\ndescription: A test skill\n---\n\n# My Skill\nDo the thing."}),
        )
        .await;

        let result = tool.execute(&ctx, json!({"action": "list"})).await;
        assert!(!result.is_error);
        let text = result_text(&result);
        assert!(text.contains("my-skill"));
    }
}
