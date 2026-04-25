//! Memory tool — lets the agent manage cross-session memory
//!
//! Actions: add, replace, remove, search.
//! Capacity-bounded with usage reporting.

#![allow(unexpected_cfgs)]
#![cfg_attr(dylint_lib = "tigerstyle", allow(compound_assertion, ignored_result, no_unwrap, no_panic, no_todo, unjustified_no_todo_allow, no_recursion, unchecked_narrowing, unchecked_division, unbounded_loop, catch_all_on_enum, explicit_defaults, unbounded_channel, unbounded_collection_growth, assertion_density, raw_arithmetic_overflow, sentinel_fallback, acronym_style, bool_naming, negated_predicate, numeric_units, float_for_currency, function_length, nested_conditionals, platform_dependent_cast, usize_in_public_api, too_many_parameters, compound_condition, unjustified_allow, ambiguous_params, ambient_clock, verified_purity, contradictory_time, multi_lock_ordering, reason = "full workspace tigerstyle audit gate: legacy debt documented locally while cleanup proceeds incrementally"))]

use std::fmt::Write;

use async_trait::async_trait;
use clankers_config::settings::MemoryLimits;
use clankers_db::memory::MemoryEntry;
use clankers_db::memory::MemoryScope;
use clankers_db::memory::MemorySource;
use serde_json::Value;
use serde_json::json;

use super::Tool;
use super::ToolContext;
use super::ToolDefinition;
use super::ToolResult;

pub struct MemoryTool {
    definition: ToolDefinition,
    limits: MemoryLimits,
}

impl MemoryTool {
    pub fn new(limits: MemoryLimits) -> Self {
        Self {
            limits,
            definition: ToolDefinition {
                name: "memory".to_string(),
                description: "Manage cross-session memory. Saves facts, preferences, and project \
                    knowledge that persist across sessions and are injected into the system prompt.\n\n\
                    Actions:\n\
                    - add: Save a new memory entry\n\
                    - replace: Update an existing entry (matched by substring)\n\
                    - remove: Delete an entry (matched by substring)\n\
                    - search: Find entries by keyword"
                    .to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "action": {
                            "type": "string",
                            "enum": ["add", "replace", "remove", "search"],
                            "description": "Action to perform"
                        },
                        "text": {
                            "type": "string",
                            "description": "Memory text to save (add) or new text (replace)"
                        },
                        "old_text": {
                            "type": "string",
                            "description": "Substring to match existing entry (replace/remove)"
                        },
                        "scope": {
                            "type": "string",
                            "enum": ["global", "project"],
                            "description": "Memory scope: 'global' (all projects) or 'project' (current cwd). Default: global"
                        },
                        "query": {
                            "type": "string",
                            "description": "Search query (search action)"
                        },
                        "tags": {
                            "type": "array",
                            "items": { "type": "string" },
                            "description": "Optional tags for the entry"
                        }
                    },
                    "required": ["action"]
                }),
            },
        }
    }

    fn resolve_scope(&self, params: &Value) -> MemoryScope {
        let scope_str = params.get("scope").and_then(|v| v.as_str()).unwrap_or("global");
        if scope_str == "project" {
            let cwd = std::env::current_dir().ok().and_then(|p| p.to_str().map(String::from)).unwrap_or_default();
            MemoryScope::Project { path: cwd }
        } else {
            MemoryScope::Global
        }
    }

    fn char_limit_for_scope(&self, scope: &MemoryScope) -> usize {
        match scope {
            MemoryScope::Global => self.limits.global_char_limit,
            MemoryScope::Project { .. } => self.limits.project_char_limit,
        }
    }

    fn format_usage(&self, db: &clankers_db::Db, scope: &MemoryScope) -> String {
        let current = db.memory().total_chars(Some(scope)).unwrap_or(0);
        let limit = self.char_limit_for_scope(scope);
        format!("{}/{}", current, limit)
    }

    fn handle_add(&self, db: &clankers_db::Db, params: &Value) -> ToolResult {
        let text = match params.get("text").and_then(|v| v.as_str()) {
            Some(t) if !t.is_empty() => t,
            _ => return ToolResult::error("Missing required 'text' parameter for add action."),
        };

        let scope = self.resolve_scope(params);
        let limit = self.char_limit_for_scope(&scope);
        let current = db.memory().total_chars(Some(&scope)).unwrap_or(0);

        if current + text.len() > limit {
            let entries = db.memory().list(Some(&scope)).unwrap_or_default();
            let mut listing = String::new();
            for e in &entries {
                writeln!(listing, "- [{}] {} ({}ch)", e.id, e.text, e.text.len()).ok();
            }
            return ToolResult::error(format!(
                "Memory at {current}/{limit} chars. Adding this entry ({} chars) would exceed the limit.\n\
                 Replace or remove existing entries first.\n\nCurrent entries:\n{listing}",
                text.len()
            ));
        }

        let tags: Vec<String> = params
            .get("tags")
            .and_then(|v| v.as_array())
            .map(|arr| arr.iter().filter_map(|v| v.as_str().map(String::from)).collect())
            .unwrap_or_default();

        let mut entry = MemoryEntry::new(text, scope.clone()).with_source(MemorySource::Agent);
        if !tags.is_empty() {
            entry = entry.with_tags(tags);
        }

        let id = entry.id;
        if let Err(e) = db.memory().save(&entry) {
            return ToolResult::error(format!("Failed to save memory: {e}"));
        }

        let usage = self.format_usage(db, &scope);
        ToolResult::text(format!("Saved memory (id: {id}, scope: {scope}).\nUsage: {usage}"))
    }

    fn handle_replace(&self, db: &clankers_db::Db, params: &Value) -> ToolResult {
        let old_text = match params.get("old_text").and_then(|v| v.as_str()) {
            Some(t) if !t.is_empty() => t,
            _ => return ToolResult::error("Missing required 'old_text' parameter for replace action."),
        };
        let new_text = match params.get("text").and_then(|v| v.as_str()) {
            Some(t) if !t.is_empty() => t,
            _ => return ToolResult::error("Missing required 'text' parameter for replace action."),
        };

        let entries = db.memory().list(None).unwrap_or_default();
        let lower = old_text.to_lowercase();
        let matches: Vec<&MemoryEntry> = entries.iter().filter(|e| e.text.to_lowercase().contains(&lower)).collect();

        if matches.is_empty() {
            return ToolResult::error(format!("No memory entry found matching '{old_text}'."));
        }
        if matches.len() > 1 {
            let mut listing = String::from("Multiple entries match. Be more specific:\n");
            for e in &matches {
                writeln!(listing, "- [{}] {}", e.id, e.text).ok();
            }
            return ToolResult::error(listing);
        }

        let mut entry = matches[0].clone();
        let scope = entry.scope.clone();
        let limit = self.char_limit_for_scope(&scope);
        let current = db.memory().total_chars(Some(&scope)).unwrap_or(0);
        let size_delta = new_text.len() as isize - entry.text.len() as isize;

        if size_delta > 0 && current as isize + size_delta > limit as isize {
            return ToolResult::error(format!(
                "Replacing would exceed capacity ({current}/{limit} chars, delta: +{size_delta}).\n\
                 Remove other entries or shorten the replacement text."
            ));
        }

        entry.text = new_text.to_string();
        if let Err(e) = db.memory().update(&entry) {
            return ToolResult::error(format!("Failed to update memory: {e}"));
        }

        let usage = self.format_usage(db, &scope);
        ToolResult::text(format!("Replaced memory (id: {}).\nUsage: {usage}", entry.id))
    }

    fn handle_remove(&self, db: &clankers_db::Db, params: &Value) -> ToolResult {
        let old_text = match params.get("old_text").and_then(|v| v.as_str()) {
            Some(t) if !t.is_empty() => t,
            _ => return ToolResult::error("Missing required 'old_text' parameter for remove action."),
        };

        let entries = db.memory().list(None).unwrap_or_default();
        let lower = old_text.to_lowercase();
        let matches: Vec<&MemoryEntry> = entries.iter().filter(|e| e.text.to_lowercase().contains(&lower)).collect();

        if matches.is_empty() {
            return ToolResult::error(format!("No memory entry found matching '{old_text}'."));
        }
        if matches.len() > 1 {
            let mut listing = String::from("Multiple entries match. Be more specific:\n");
            for e in &matches {
                writeln!(listing, "- [{}] {}", e.id, e.text).ok();
            }
            return ToolResult::error(listing);
        }

        let entry = matches[0];
        let scope = entry.scope.clone();
        let id = entry.id;
        if let Err(e) = db.memory().remove(id) {
            return ToolResult::error(format!("Failed to remove memory: {e}"));
        }

        let usage = self.format_usage(db, &scope);
        ToolResult::text(format!("Removed memory (id: {id}).\nUsage: {usage}"))
    }

    fn handle_search(&self, db: &clankers_db::Db, params: &Value) -> ToolResult {
        let query = match params.get("query").and_then(|v| v.as_str()) {
            Some(q) if !q.is_empty() => q,
            _ => return ToolResult::error("Missing required 'query' parameter for search action."),
        };

        let results = db.memory().search(query).unwrap_or_default();
        if results.is_empty() {
            return ToolResult::text(format!("No memories matching '{query}'."));
        }

        let mut out = format!("Found {} memor{}:\n", results.len(), if results.len() == 1 { "y" } else { "ies" });
        for e in &results {
            let tags = if e.tags.is_empty() {
                String::new()
            } else {
                format!(" [{}]", e.tags.join(", "))
            };
            writeln!(out, "- [{}] ({}) {}{}", e.id, e.scope, e.text, tags).ok();
        }
        ToolResult::text(out)
    }
}

#[async_trait]
impl Tool for MemoryTool {
    fn definition(&self) -> &ToolDefinition {
        &self.definition
    }

    async fn execute(&self, ctx: &ToolContext, params: Value) -> ToolResult {
        let db = match ctx.db() {
            Some(db) => db,
            None => return ToolResult::error("Memory tool requires a database connection."),
        };

        let action = params.get("action").and_then(|v| v.as_str()).unwrap_or("");

        match action {
            "add" => self.handle_add(db, &params),
            "replace" => self.handle_replace(db, &params),
            "remove" => self.handle_remove(db, &params),
            "search" => self.handle_search(db, &params),
            other => {
                ToolResult::error(format!("Unknown action '{other}'. Use 'add', 'replace', 'remove', or 'search'."))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use clankers_db::Db;
    use tokio_util::sync::CancellationToken;

    use super::*;

    fn make_ctx(db: &Db) -> ToolContext {
        ToolContext::new("test".to_string(), CancellationToken::new(), None).with_db(db.clone())
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
    async fn test_add_global() {
        let db = Db::in_memory().unwrap();
        let tool = MemoryTool::new(MemoryLimits::default());
        let ctx = make_ctx(&db);

        let result = tool.execute(&ctx, json!({"action": "add", "text": "User prefers tabs"})).await;
        assert!(!result.is_error);
        let text = result_text(&result);
        assert!(text.contains("Saved memory"));
        assert!(text.contains("Usage:"));

        // Verify persisted
        let entries = db.memory().list(Some(&MemoryScope::Global)).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].text, "User prefers tabs");
    }

    #[tokio::test]
    async fn test_add_at_capacity() {
        let limits = MemoryLimits {
            global_char_limit: 20,
            project_char_limit: 20,
        };
        let db = Db::in_memory().unwrap();
        let tool = MemoryTool::new(limits);
        let ctx = make_ctx(&db);

        // Fill up (15 chars)
        tool.execute(&ctx, json!({"action": "add", "text": "123456789012345"})).await;

        // Try adding 10 more (would be 25 > 20)
        let result = tool.execute(&ctx, json!({"action": "add", "text": "1234567890"})).await;
        assert!(result.is_error);
        let text = result_text(&result);
        assert!(text.contains("exceed the limit"));
        assert!(text.contains("Current entries:"));
    }

    #[tokio::test]
    async fn test_replace() {
        let db = Db::in_memory().unwrap();
        let tool = MemoryTool::new(MemoryLimits::default());
        let ctx = make_ctx(&db);

        tool.execute(&ctx, json!({"action": "add", "text": "User prefers snake_case"})).await;

        let result = tool
            .execute(&ctx, json!({"action": "replace", "old_text": "snake_case", "text": "User prefers camelCase"}))
            .await;
        assert!(!result.is_error);
        assert!(result_text(&result).contains("Replaced memory"));

        let entries = db.memory().list(None).unwrap();
        assert_eq!(entries[0].text, "User prefers camelCase");
    }

    #[tokio::test]
    async fn test_replace_ambiguous() {
        let db = Db::in_memory().unwrap();
        let tool = MemoryTool::new(MemoryLimits::default());
        let ctx = make_ctx(&db);

        tool.execute(&ctx, json!({"action": "add", "text": "User prefers dark mode"})).await;
        tool.execute(&ctx, json!({"action": "add", "text": "User profile is dark theme"})).await;

        let result = tool.execute(&ctx, json!({"action": "replace", "old_text": "dark", "text": "something"})).await;
        assert!(result.is_error);
        assert!(result_text(&result).contains("Multiple entries match"));
    }

    #[tokio::test]
    async fn test_remove() {
        let db = Db::in_memory().unwrap();
        let tool = MemoryTool::new(MemoryLimits::default());
        let ctx = make_ctx(&db);

        tool.execute(&ctx, json!({"action": "add", "text": "temp fact"})).await;
        assert_eq!(db.memory().count().unwrap(), 1);

        let result = tool.execute(&ctx, json!({"action": "remove", "old_text": "temp"})).await;
        assert!(!result.is_error);
        assert!(result_text(&result).contains("Removed memory"));
        assert_eq!(db.memory().count().unwrap(), 0);
    }

    #[tokio::test]
    async fn test_remove_nonexistent() {
        let db = Db::in_memory().unwrap();
        let tool = MemoryTool::new(MemoryLimits::default());
        let ctx = make_ctx(&db);

        let result = tool.execute(&ctx, json!({"action": "remove", "old_text": "ghost"})).await;
        assert!(result.is_error);
        assert!(result_text(&result).contains("No memory entry found"));
    }

    #[tokio::test]
    async fn test_search() {
        let db = Db::in_memory().unwrap();
        let tool = MemoryTool::new(MemoryLimits::default());
        let ctx = make_ctx(&db);

        tool.execute(&ctx, json!({"action": "add", "text": "Uses PostgreSQL 16"})).await;
        tool.execute(&ctx, json!({"action": "add", "text": "Runs on Ubuntu 22.04"})).await;

        let result = tool.execute(&ctx, json!({"action": "search", "query": "postgres"})).await;
        assert!(!result.is_error);
        let text = result_text(&result);
        assert!(text.contains("PostgreSQL"));
        assert!(!text.contains("Ubuntu"));
    }

    #[tokio::test]
    async fn test_search_empty() {
        let db = Db::in_memory().unwrap();
        let tool = MemoryTool::new(MemoryLimits::default());
        let ctx = make_ctx(&db);

        let result = tool.execute(&ctx, json!({"action": "search", "query": "nonexistent"})).await;
        assert!(!result.is_error);
        assert!(result_text(&result).contains("No memories matching"));
    }

    #[tokio::test]
    async fn test_no_db() {
        let tool = MemoryTool::new(MemoryLimits::default());
        let ctx = ToolContext::new("test".to_string(), CancellationToken::new(), None);
        let result = tool.execute(&ctx, json!({"action": "add", "text": "hi"})).await;
        assert!(result.is_error);
        assert!(result_text(&result).contains("database"));
    }

    #[tokio::test]
    async fn test_unknown_action() {
        let db = Db::in_memory().unwrap();
        let tool = MemoryTool::new(MemoryLimits::default());
        let ctx = make_ctx(&db);

        let result = tool.execute(&ctx, json!({"action": "foobar"})).await;
        assert!(result.is_error);
    }
}
