//! Skill management tool — lets the agent create, update, and delete skills.
//!
//! Skills are written to ~/.clankers/agent/skills/<name>/SKILL.md or
//! ~/.clankers/agent/skills/<category>/<name>/SKILL.md.

#![allow(unexpected_cfgs)]
#![cfg_attr(dylint_lib = "tigerstyle", allow(compound_assertion, ignored_result, no_unwrap, no_panic, no_todo, unjustified_no_todo_allow, no_recursion, unchecked_narrowing, unchecked_division, unbounded_loop, catch_all_on_enum, explicit_defaults, unbounded_channel, unbounded_collection_growth, assertion_density, raw_arithmetic_overflow, sentinel_fallback, acronym_style, bool_naming, negated_predicate, numeric_units, float_for_currency, function_length, nested_conditionals, platform_dependent_cast, usize_in_public_api, too_many_parameters, compound_condition, unjustified_allow, ambiguous_params, ambient_clock, verified_purity, contradictory_time, multi_lock_ordering, reason = "full workspace tigerstyle audit gate: legacy debt documented locally while cleanup proceeds incrementally"))]

use std::fmt::Write;
use std::path::Path;
use std::path::PathBuf;

use async_trait::async_trait;
use serde_json::Value;
use serde_json::json;

use super::Tool;
use super::ToolContext;
use super::ToolDefinition;
use super::ToolResult;

pub struct SkillManageTool {
    definition: ToolDefinition,
    global_skills_dir: PathBuf,
}

impl SkillManageTool {
    pub fn new(global_skills_dir: PathBuf) -> Self {
        Self {
            global_skills_dir,
            definition: ToolDefinition {
                name: "skill_manage".to_string(),
                description: "Create, update, and delete reusable skills from experience. Skills are markdown files loaded into the system prompt for future sessions."
                    .to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "action": {
                            "type": "string",
                            "enum": ["create", "patch", "edit", "delete", "write_file", "remove_file", "list"],
                            "description": "Action to perform"
                        },
                        "name": {
                            "type": "string",
                            "description": "Skill name, e.g. 'deploy-k8s'"
                        },
                        "category": {
                            "type": "string",
                            "description": "Optional category directory for create, e.g. 'devops'"
                        },
                        "content": {
                            "type": "string",
                            "description": "Full SKILL.md content for create/edit"
                        },
                        "old_text": {
                            "type": "string",
                            "description": "Exact text to replace for patch"
                        },
                        "new_text": {
                            "type": "string",
                            "description": "Replacement text for patch"
                        },
                        "file_path": {
                            "type": "string",
                            "description": "Relative supporting file path under references/, templates/, assets/, or scripts/"
                        },
                        "file_content": {
                            "type": "string",
                            "description": "Supporting file content for write_file"
                        }
                    },
                    "required": ["action"]
                }),
            },
        }
    }

    fn handle_create(&self, params: &Value) -> ToolResult {
        let name = match required_str(params, "name") {
            Ok(value) => value,
            Err(err) => return err,
        };
        let content = match required_str(params, "content") {
            Ok(value) => value,
            Err(err) => return err,
        };
        let category = optional_str(params, "category");

        match clankers_skills::write_skill(&self.global_skills_dir, name, category, content) {
            Ok(path) => ToolResult::text(format!("Created skill '{name}' at {}", path.display())),
            Err(err) => skill_error(err),
        }
    }

    fn handle_patch(&self, params: &Value) -> ToolResult {
        let name = match required_str(params, "name") {
            Ok(value) => value,
            Err(err) => return err,
        };
        let old_text = match required_str(params, "old_text") {
            Ok(value) => value,
            Err(err) => return err,
        };
        let new_text = match required_str_allow_empty(params, "new_text") {
            Ok(value) => value,
            Err(err) => return err,
        };
        let file = optional_str(params, "file_path").map(Path::new);

        match clankers_skills::patch_skill(&self.global_skills_dir, name, old_text, new_text, file) {
            Ok(()) => ToolResult::text(format!("Patched skill '{name}'.")),
            Err(err) => skill_error(err),
        }
    }

    fn handle_edit(&self, params: &Value) -> ToolResult {
        let name = match required_str(params, "name") {
            Ok(value) => value,
            Err(err) => return err,
        };
        let content = match required_str(params, "content") {
            Ok(value) => value,
            Err(err) => return err,
        };

        match clankers_skills::edit_skill(&self.global_skills_dir, name, content) {
            Ok(()) => ToolResult::text(format!("Replaced skill '{name}' content.")),
            Err(err) => skill_error(err),
        }
    }

    fn handle_delete(&self, params: &Value) -> ToolResult {
        let name = match required_str(params, "name") {
            Ok(value) => value,
            Err(err) => return err,
        };

        match clankers_skills::delete_skill(&self.global_skills_dir, name) {
            Ok(()) => ToolResult::text(format!("Deleted skill '{name}'.")),
            Err(err) => skill_error(err),
        }
    }

    fn handle_write_file(&self, params: &Value) -> ToolResult {
        let name = match required_str(params, "name") {
            Ok(value) => value,
            Err(err) => return err,
        };
        let file_path = match required_str(params, "file_path") {
            Ok(value) => value,
            Err(err) => return err,
        };
        let file_content = match required_str_allow_empty(params, "file_content") {
            Ok(value) => value,
            Err(err) => return err,
        };

        match clankers_skills::write_skill_file(&self.global_skills_dir, name, Path::new(file_path), file_content) {
            Ok(()) => ToolResult::text(format!("Wrote {file_path} to skill '{name}'.")),
            Err(err) => skill_error(err),
        }
    }

    fn handle_remove_file(&self, params: &Value) -> ToolResult {
        let name = match required_str(params, "name") {
            Ok(value) => value,
            Err(err) => return err,
        };
        let file_path = match required_str(params, "file_path") {
            Ok(value) => value,
            Err(err) => return err,
        };

        match clankers_skills::remove_skill_file(&self.global_skills_dir, name, Path::new(file_path)) {
            Ok(()) => ToolResult::text(format!("Removed {file_path} from skill '{name}'.")),
            Err(err) => skill_error(err),
        }
    }

    fn handle_list(&self) -> ToolResult {
        let skills = clankers_skills::scan_skills_dir(&self.global_skills_dir);
        if skills.is_empty() {
            return ToolResult::text("No skills installed.".to_string());
        }

        let mut out = format!("{} skill(s):\n", skills.len());
        for skill in &skills {
            writeln!(out, "- **{}**: {}", skill.name, skill.description).ok();
        }
        ToolResult::text(out)
    }
}

fn required_str<'a>(params: &'a Value, key: &str) -> Result<&'a str, ToolResult> {
    match params.get(key).and_then(|value| value.as_str()) {
        Some(value) if !value.is_empty() => Ok(value),
        _ => Err(ToolResult::error(format!("Missing required '{key}' parameter."))),
    }
}

fn required_str_allow_empty<'a>(params: &'a Value, key: &str) -> Result<&'a str, ToolResult> {
    match params.get(key).and_then(|value| value.as_str()) {
        Some(value) => Ok(value),
        None => Err(ToolResult::error(format!("Missing required '{key}' parameter."))),
    }
}

fn optional_str<'a>(params: &'a Value, key: &str) -> Option<&'a str> {
    params.get(key).and_then(|value| value.as_str())
}

fn skill_error(err: clankers_skills::SkillError) -> ToolResult {
    ToolResult::error(err.to_string())
}

#[async_trait]
impl Tool for SkillManageTool {
    fn definition(&self) -> &ToolDefinition {
        &self.definition
    }

    async fn execute(&self, _ctx: &ToolContext, params: Value) -> ToolResult {
        let action = params.get("action").and_then(|v| v.as_str()).unwrap_or("");

        match action {
            "create" => self.handle_create(&params),
            "patch" => self.handle_patch(&params),
            "edit" => self.handle_edit(&params),
            "delete" => self.handle_delete(&params),
            "write_file" => self.handle_write_file(&params),
            "remove_file" => self.handle_remove_file(&params),
            "list" => self.handle_list(),
            other => ToolResult::error(format!(
                "Unknown action '{other}'. Use 'create', 'patch', 'edit', 'delete', 'write_file', 'remove_file', or 'list'."
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use tempfile::TempDir;
    use tokio_util::sync::CancellationToken;

    use super::*;

    const VALID_SKILL: &str = "---\nname: test-skill\ndescription: A test skill\n---\n# Test\nSome content\n";
    const UPDATED_SKILL: &str =
        "---\nname: test-skill\ndescription: Updated test skill\n---\n# Test\nUpdated content\n";

    fn make_ctx() -> ToolContext {
        ToolContext::new("test".to_string(), CancellationToken::new(), None)
    }

    fn result_text(result: &ToolResult) -> String {
        result
            .content
            .iter()
            .filter_map(|content| match content {
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

        let result =
            tool.execute(&ctx, json!({"action": "create", "name": "test-skill", "content": VALID_SKILL})).await;
        assert!(!result.is_error);
        assert!(result_text(&result).contains("Created skill"));

        let skill_path = tmp.path().join("test-skill").join("SKILL.md");
        assert!(skill_path.exists());
        assert_eq!(std::fs::read_to_string(&skill_path).unwrap(), VALID_SKILL);
    }

    #[tokio::test]
    async fn test_create_already_exists() {
        let tmp = TempDir::new().unwrap();
        let tool = SkillManageTool::new(tmp.path().to_path_buf());
        let ctx = make_ctx();

        tool.execute(&ctx, json!({"action": "create", "name": "test-skill", "content": VALID_SKILL})).await;

        let result =
            tool.execute(&ctx, json!({"action": "create", "name": "test-skill", "content": VALID_SKILL})).await;
        assert!(result.is_error);
        assert!(result_text(&result).contains("already exists"));
    }

    #[tokio::test]
    async fn test_invalid_name() {
        let tmp = TempDir::new().unwrap();
        let tool = SkillManageTool::new(tmp.path().to_path_buf());
        let ctx = make_ctx();

        let result = tool.execute(&ctx, json!({"action": "create", "name": "Bad Name!", "content": VALID_SKILL})).await;
        assert!(result.is_error);
        assert!(result_text(&result).contains("invalid skill name"));
    }

    #[tokio::test]
    async fn test_patch_skill_file() {
        let tmp = TempDir::new().unwrap();
        let tool = SkillManageTool::new(tmp.path().to_path_buf());
        let ctx = make_ctx();

        tool.execute(&ctx, json!({"action": "create", "name": "test-skill", "content": VALID_SKILL})).await;

        let result = tool
            .execute(
                &ctx,
                json!({
                    "action": "patch",
                    "name": "test-skill",
                    "old_text": "Some content",
                    "new_text": "Patched content"
                }),
            )
            .await;
        assert!(!result.is_error);
        let content = std::fs::read_to_string(tmp.path().join("test-skill").join("SKILL.md")).unwrap();
        assert!(content.contains("Patched content"));
    }

    #[tokio::test]
    async fn test_delete() {
        let tmp = TempDir::new().unwrap();
        let tool = SkillManageTool::new(tmp.path().to_path_buf());
        let ctx = make_ctx();

        tool.execute(&ctx, json!({"action": "create", "name": "test-skill", "content": VALID_SKILL})).await;
        let result = tool.execute(&ctx, json!({"action": "delete", "name": "test-skill"})).await;
        assert!(!result.is_error);
        assert!(!tmp.path().join("test-skill").exists());
    }

    #[tokio::test]
    async fn test_write_and_remove_supporting_file() {
        let tmp = TempDir::new().unwrap();
        let tool = SkillManageTool::new(tmp.path().to_path_buf());
        let ctx = make_ctx();

        tool.execute(&ctx, json!({"action": "create", "name": "test-skill", "content": VALID_SKILL})).await;

        let write_result = tool
            .execute(
                &ctx,
                json!({
                    "action": "write_file",
                    "name": "test-skill",
                    "file_path": "references/notes.md",
                    "file_content": "Some notes"
                }),
            )
            .await;
        assert!(!write_result.is_error);

        let remove_result = tool
            .execute(
                &ctx,
                json!({
                    "action": "remove_file",
                    "name": "test-skill",
                    "file_path": "references/notes.md"
                }),
            )
            .await;
        assert!(!remove_result.is_error);
        assert!(!tmp.path().join("test-skill").join("references").join("notes.md").exists());
    }

    #[tokio::test]
    async fn test_list_after_create() {
        let tmp = TempDir::new().unwrap();
        let tool = SkillManageTool::new(tmp.path().to_path_buf());
        let ctx = make_ctx();

        tool.execute(&ctx, json!({"action": "create", "name": "test-skill", "content": VALID_SKILL})).await;

        let result = tool.execute(&ctx, json!({"action": "list"})).await;
        assert!(!result.is_error);
        assert!(result_text(&result).contains("test-skill"));
    }

    #[tokio::test]
    async fn test_edit_rewrites_skill() {
        let tmp = TempDir::new().unwrap();
        let tool = SkillManageTool::new(tmp.path().to_path_buf());
        let ctx = make_ctx();

        tool.execute(&ctx, json!({"action": "create", "name": "test-skill", "content": VALID_SKILL})).await;

        let result =
            tool.execute(&ctx, json!({"action": "edit", "name": "test-skill", "content": UPDATED_SKILL})).await;
        assert!(!result.is_error);
        assert_eq!(std::fs::read_to_string(tmp.path().join("test-skill").join("SKILL.md")).unwrap(), UPDATED_SKILL);
    }
}
