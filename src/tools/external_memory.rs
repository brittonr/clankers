//! External memory provider tool adapter.
//!
//! First pass keeps external memory disabled by default and exposes a small,
//! deterministic local provider seam. Remote provider kinds return explicit
//! unsupported/configuration errors before contact.

use std::fmt::Write;
use std::time::Instant;

use async_trait::async_trait;
use clankers_config::ExternalMemoryConfigError;
use clankers_config::ExternalMemoryProvider;
use clankers_config::ExternalMemorySettings;
use clankers_db::memory::MemoryEntry;
use clankers_db::memory::MemoryScope;
use serde_json::Value;
use serde_json::json;

use super::Tool;
use super::ToolContext;
use super::ToolDefinition;
use super::ToolResult;

const DEFAULT_ACTION: &str = "search";
const SOURCE: &str = "external_memory_provider";

pub struct ExternalMemoryTool {
    definition: ToolDefinition,
    settings: ExternalMemorySettings,
}

impl ExternalMemoryTool {
    pub fn new(settings: ExternalMemorySettings) -> Self {
        Self {
            settings,
            definition: ToolDefinition {
                name: "external_memory".to_string(),
                description: concat!(
                    "Query a configured external memory/personalization provider. ",
                    "First pass supports status and local-provider search. ",
                    "Remote providers return explicit unsupported/configuration errors until implemented."
                )
                .to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "action": {
                            "type": "string",
                            "enum": ["search", "status"],
                            "description": "Action to perform. Default: search"
                        },
                        "query": {
                            "type": "string",
                            "description": "Search query for external memories"
                        },
                        "limit": {
                            "type": "integer",
                            "description": "Maximum results, bounded by externalMemory.maxResults"
                        },
                        "scope": {
                            "type": "string",
                            "enum": ["all", "global", "project"],
                            "description": "Scope filter for local provider search. Default: all"
                        }
                    },
                    "required": []
                }),
            },
        }
    }

    fn status_details(&self, status: &str, elapsed_ms: u128, result_count: usize) -> Value {
        json!({
            "source": SOURCE,
            "providerKind": provider_kind(self.settings.provider),
            "providerName": self.settings.safe_provider_name(),
            "action": status,
            "status": "ok",
            "elapsedMs": elapsed_ms,
            "resultCount": result_count,
            "injectIntoPrompt": self.settings.inject_into_prompt,
        })
    }

    fn error_details(&self, action: &str, elapsed_ms: u128, error_kind: &str, error: &str) -> Value {
        json!({
            "source": SOURCE,
            "providerKind": provider_kind(self.settings.provider),
            "providerName": self.settings.safe_provider_name(),
            "action": action,
            "status": "error",
            "elapsedMs": elapsed_ms,
            "resultCount": 0,
            "errorKind": error_kind,
            "error": redact_error(error),
        })
    }

    fn status(&self, started: Instant) -> ToolResult {
        let elapsed_ms = started.elapsed().as_millis();
        let mut out = String::new();
        writeln!(out, "External memory provider status").ok();
        writeln!(out, "- provider: {}", provider_kind(self.settings.provider)).ok();
        writeln!(out, "- name: {}", self.settings.safe_provider_name()).ok();
        writeln!(out, "- enabled: {}", self.settings.enabled).ok();
        writeln!(out, "- maxResults: {}", self.settings.max_results).ok();
        writeln!(out, "- injectIntoPrompt: {}", self.settings.inject_into_prompt).ok();
        ToolResult::text(out).with_details(self.status_details("status", elapsed_ms, 0))
    }

    fn local_search(&self, ctx: &ToolContext, params: &Value, started: Instant) -> ToolResult {
        let query = match params.get("query").and_then(|value| value.as_str()).map(str::trim) {
            Some(query) if !query.is_empty() => query,
            _ => {
                let elapsed_ms = started.elapsed().as_millis();
                return ToolResult::error("external_memory search requires a non-empty `query` parameter")
                    .with_details(self.error_details(
                        "search",
                        elapsed_ms,
                        "missing_query",
                        "missing non-empty query",
                    ));
            }
        };

        let db = match ctx.db() {
            Some(db) => db,
            None => {
                let elapsed_ms = started.elapsed().as_millis();
                return ToolResult::error("external_memory local provider requires a database connection")
                    .with_details(self.error_details(
                        "search",
                        elapsed_ms,
                        "missing_database",
                        "database connection unavailable",
                    ));
            }
        };

        let limit = bounded_limit(params.get("limit"), self.settings.max_results);
        let scope = params.get("scope").and_then(|value| value.as_str()).unwrap_or("all");
        let results = match db.memory().search(query) {
            Ok(entries) => filter_scope(entries, scope).into_iter().take(limit).collect::<Vec<_>>(),
            Err(error) => {
                let elapsed_ms = started.elapsed().as_millis();
                return ToolResult::error(format!(
                    "external_memory search failed: {}",
                    redact_error(&error.to_string())
                ))
                .with_details(self.error_details(
                    "search",
                    elapsed_ms,
                    "provider_error",
                    &error.to_string(),
                ));
            }
        };

        let elapsed_ms = started.elapsed().as_millis();
        let mut out = format!(
            "Found {} external memor{} for '{query}':\n",
            results.len(),
            if results.len() == 1 { "y" } else { "ies" }
        );
        for entry in &results {
            writeln!(out, "- [{}] ({}) {}", entry.id, entry.scope, entry.text).ok();
        }
        ToolResult::text(out).with_details(json!({
            "source": SOURCE,
            "providerKind": provider_kind(self.settings.provider),
            "providerName": self.settings.safe_provider_name(),
            "action": "search",
            "status": "ok",
            "elapsedMs": elapsed_ms,
            "resultCount": results.len(),
        }))
    }
}

#[async_trait]
impl Tool for ExternalMemoryTool {
    fn definition(&self) -> &ToolDefinition {
        &self.definition
    }

    async fn execute(&self, ctx: &ToolContext, params: Value) -> ToolResult {
        let started = Instant::now();
        if let Err(error) = self.settings.validate() {
            let elapsed_ms = started.elapsed().as_millis();
            return ToolResult::error(format!("externalMemory configuration invalid: {error}")).with_details(
                self.error_details("validate", elapsed_ms, config_error_kind(&error), &error.to_string()),
            );
        }

        let action = params.get("action").and_then(|value| value.as_str()).unwrap_or(DEFAULT_ACTION);
        match action {
            "status" => self.status(started),
            "search" => match self.settings.provider {
                ExternalMemoryProvider::Local => self.local_search(ctx, &params, started),
                ExternalMemoryProvider::Http => {
                    let elapsed_ms = started.elapsed().as_millis();
                    ToolResult::error("HTTP external memory providers are not implemented in this first pass")
                        .with_details(self.error_details(
                            "search",
                            elapsed_ms,
                            "unsupported_provider",
                            "HTTP external memory unsupported",
                        ))
                }
            },
            other => {
                let elapsed_ms = started.elapsed().as_millis();
                ToolResult::error(format!("Unknown external_memory action '{other}'. Use 'search' or 'status'."))
                    .with_details(self.error_details(other, elapsed_ms, "unknown_action", "unknown action"))
            }
        }
    }
}

pub fn build_external_memory_tool_from_settings(settings: &ExternalMemorySettings) -> Option<std::sync::Arc<dyn Tool>> {
    if !settings.enabled || settings.validate().is_err() {
        return None;
    }
    Some(std::sync::Arc::new(ExternalMemoryTool::new(settings.clone())))
}

fn bounded_limit(value: Option<&Value>, max_results: usize) -> usize {
    value
        .and_then(Value::as_u64)
        .and_then(|limit| usize::try_from(limit).ok())
        .filter(|limit| *limit > 0)
        .map(|limit| limit.min(max_results))
        .unwrap_or(max_results)
}

fn filter_scope(entries: Vec<MemoryEntry>, scope: &str) -> Vec<MemoryEntry> {
    entries
        .into_iter()
        .filter(|entry| match scope {
            "global" => matches!(entry.scope, MemoryScope::Global),
            "project" => matches!(entry.scope, MemoryScope::Project { .. }),
            _ => true,
        })
        .collect()
}

fn provider_kind(provider: ExternalMemoryProvider) -> &'static str {
    match provider {
        ExternalMemoryProvider::Local => "local",
        ExternalMemoryProvider::Http => "http",
    }
}

fn config_error_kind(error: &ExternalMemoryConfigError) -> &'static str {
    match error {
        ExternalMemoryConfigError::BlankName => "blank_name",
        ExternalMemoryConfigError::MissingHttpEndpoint => "missing_endpoint",
        ExternalMemoryConfigError::BlankEndpoint => "blank_endpoint",
        ExternalMemoryConfigError::BlankCredentialEnv => "blank_credential_env",
        ExternalMemoryConfigError::NonPositiveTimeout => "non_positive_timeout",
        ExternalMemoryConfigError::NonPositiveMaxResults => "non_positive_max_results",
        ExternalMemoryConfigError::HttpUnsupported => "unsupported_provider",
    }
}

fn redact_error(error: &str) -> String {
    let mut out = error.to_string();
    for marker in [
        "token",
        "secret",
        "password",
        "api_key",
        "apikey",
        "authorization",
        "bearer",
    ] {
        if out.to_lowercase().contains(marker) {
            out = "[REDACTED]".to_string();
            break;
        }
    }
    out
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
            .filter_map(|content| match content {
                super::super::ToolResultContent::Text { text } => Some(text.as_str()),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("")
    }

    fn enabled_settings() -> ExternalMemorySettings {
        ExternalMemorySettings {
            enabled: true,
            name: Some("test-memory".to_string()),
            ..ExternalMemorySettings::default()
        }
    }

    #[tokio::test]
    async fn local_search_returns_bounded_results() {
        let db = Db::in_memory().unwrap();
        db.memory().save(&MemoryEntry::new("User prefers Rust automation", MemoryScope::Global)).unwrap();
        db.memory().save(&MemoryEntry::new("Rust tests use nextest", MemoryScope::Global)).unwrap();
        let tool = ExternalMemoryTool::new(enabled_settings());
        let result = tool.execute(&make_ctx(&db), json!({"action": "search", "query": "Rust", "limit": 1})).await;

        assert!(!result.is_error);
        assert!(result_text(&result).contains("Found 1 external memory"));
        assert_eq!(
            result.details.as_ref().and_then(|details| details.get("resultCount")).and_then(Value::as_u64),
            Some(1)
        );
    }

    #[tokio::test]
    async fn missing_query_is_actionable_error() {
        let db = Db::in_memory().unwrap();
        let tool = ExternalMemoryTool::new(enabled_settings());
        let result = tool.execute(&make_ctx(&db), json!({"action": "search"})).await;

        assert!(result.is_error);
        assert!(result_text(&result).contains("non-empty `query`"));
        assert_eq!(
            result.details.as_ref().and_then(|details| details.get("errorKind")).and_then(Value::as_str),
            Some("missing_query")
        );
    }

    #[test]
    fn disabled_or_invalid_config_is_not_published() {
        assert!(build_external_memory_tool_from_settings(&ExternalMemorySettings::default()).is_none());
        let invalid = ExternalMemorySettings {
            enabled: true,
            max_results: 0,
            ..ExternalMemorySettings::default()
        };
        assert!(build_external_memory_tool_from_settings(&invalid).is_none());
    }
}
