//! Session search tool — search past session content
//!
//! Two-tier search: index metadata (fast) → JSONL content scan (fallback).

use std::fmt::Write;
use std::io::BufRead;
use std::path::PathBuf;

use async_trait::async_trait;
use serde_json::Value;
use serde_json::json;

use super::Tool;
use super::ToolContext;
use super::ToolDefinition;
use super::ToolResult;

pub struct SessionSearchTool {
    definition: ToolDefinition,
    sessions_dir: PathBuf,
    max_scan_files: usize,
}

impl SessionSearchTool {
    pub fn new(sessions_dir: PathBuf, max_scan_files: usize) -> Self {
        Self {
            sessions_dir,
            max_scan_files,
            definition: ToolDefinition {
                name: "session_search".to_string(),
                description: "Search past conversation sessions. Finds previous sessions by topic, \
                    content, model, or working directory. Returns session previews with matching context."
                    .to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "query": {
                            "type": "string",
                            "description": "Search query (substring match, case-insensitive)"
                        },
                        "cwd": {
                            "type": "string",
                            "description": "Filter to sessions from this working directory (optional)"
                        },
                        "limit": {
                            "type": "integer",
                            "description": "Max results to return (default: 10)"
                        }
                    },
                    "required": ["query"]
                }),
            },
        }
    }

    /// Tier 1: search session index metadata via redb.
    fn search_index(&self, db: &clankers_db::Db, query: &str, cwd: Option<&str>, limit: usize) -> Vec<SessionResult> {
        let entries = db.sessions().search(query).unwrap_or_default();

        entries
            .into_iter()
            .filter(|e| cwd.is_none() || cwd == Some(e.cwd.as_str()))
            .take(limit)
            .map(|e| SessionResult {
                session_id: e.session_id,
                date: e.created_at.format("%Y-%m-%d %H:%M").to_string(),
                model: e.model,
                cwd: e.cwd,
                preview: e.first_prompt.clone(),
                source: "index".to_string(),
            })
            .collect()
    }

    /// Tier 2: scan JSONL files for content matches.
    fn search_jsonl(&self, query: &str, cwd: Option<&str>, limit: usize, exclude_ids: &[String]) -> Vec<SessionResult> {
        let lower_query = query.to_lowercase();
        let mut results = Vec::new();

        // List JSONL files, newest first (by modification time)
        let mut files: Vec<PathBuf> = std::fs::read_dir(&self.sessions_dir)
            .into_iter()
            .flatten()
            .flatten()
            .filter(|e| e.path().extension().and_then(|s| s.to_str()).map(|s| s == "jsonl").unwrap_or(false))
            .map(|e| e.path())
            .collect();

        files.sort_by(|a, b| {
            let ma = a.metadata().and_then(|m| m.modified()).ok();
            let mb = b.metadata().and_then(|m| m.modified()).ok();
            mb.cmp(&ma)
        });

        for (scanned, file) in files.into_iter().enumerate() {
            if scanned >= self.max_scan_files || results.len() >= limit {
                break;
            }

            // Extract session ID from filename
            let session_id = file.file_stem().and_then(|s| s.to_str()).unwrap_or("unknown").to_string();

            if exclude_ids.contains(&session_id) {
                continue;
            }

            let reader = match std::fs::File::open(&file) {
                Ok(f) => std::io::BufReader::new(f),
                Err(_) => continue,
            };

            let mut matches_in_file: Vec<String> = Vec::new();
            let lines: Vec<String> = reader.lines().map_while(Result::ok).collect();

            for (i, line) in lines.iter().enumerate() {
                if matches_in_file.len() >= 3 {
                    break;
                }
                if line.to_lowercase().contains(&lower_query) {
                    // Grab surrounding context
                    let start = i.saturating_sub(1);
                    let end = (i + 2).min(lines.len());
                    let context: String = lines[start..end].join(" ");
                    let preview = if context.len() > 200 {
                        format!("{}...", &context[..200])
                    } else {
                        context
                    };
                    matches_in_file.push(preview);
                }
            }

            if matches_in_file.is_empty() {
                continue;
            }

            // Try to extract cwd from the first line (session metadata)
            let file_cwd = lines.first().and_then(|l| {
                serde_json::from_str::<Value>(l)
                    .ok()
                    .and_then(|v| v.get("cwd").and_then(|c| c.as_str()).map(String::from))
            });

            if let Some(filter_cwd) = cwd
                && file_cwd.as_deref() != Some(filter_cwd)
            {
                continue;
            }

            results.push(SessionResult {
                session_id,
                date: file
                    .metadata()
                    .and_then(|m| m.modified())
                    .ok()
                    .map(|t| {
                        let d: chrono::DateTime<chrono::Utc> = t.into();
                        d.format("%Y-%m-%d %H:%M").to_string()
                    })
                    .unwrap_or_else(|| "unknown".into()),
                model: String::new(),
                cwd: file_cwd.unwrap_or_default(),
                preview: matches_in_file.join("\n---\n"),
                source: "content".to_string(),
            });
        }

        results
    }
}

struct SessionResult {
    session_id: String,
    date: String,
    model: String,
    cwd: String,
    preview: String,
    source: String,
}

#[async_trait]
impl Tool for SessionSearchTool {
    fn definition(&self) -> &ToolDefinition {
        &self.definition
    }

    async fn execute(&self, ctx: &ToolContext, params: Value) -> ToolResult {
        let query = match params.get("query").and_then(|v| v.as_str()) {
            Some(q) if !q.is_empty() => q,
            _ => return ToolResult::error("Missing required 'query' parameter."),
        };
        let cwd = params.get("cwd").and_then(|v| v.as_str());
        let limit = usize::try_from(params.get("limit").and_then(|v| v.as_u64()).unwrap_or(10)).unwrap_or(10);

        // Tier 1: index search
        let mut results = if let Some(db) = ctx.db() {
            self.search_index(db, query, cwd, limit)
        } else {
            Vec::new()
        };

        // Tier 2: JSONL scan if we need more results
        if results.len() < limit {
            let exclude_ids: Vec<String> = results.iter().map(|r| r.session_id.clone()).collect();
            let remaining = limit - results.len();
            let jsonl_results = self.search_jsonl(query, cwd, remaining, &exclude_ids);
            results.extend(jsonl_results);
        }

        if results.is_empty() {
            return ToolResult::text(format!("No sessions matching '{query}'."));
        }

        let mut out = format!("Found {} session(s):\n\n", results.len());
        for r in &results {
            writeln!(out, "**{}** ({})", r.session_id, r.date).ok();
            if !r.model.is_empty() {
                writeln!(out, "  Model: {}", r.model).ok();
            }
            if !r.cwd.is_empty() {
                writeln!(out, "  Dir: {}", r.cwd).ok();
            }
            writeln!(out, "  {}", r.preview).ok();
            writeln!(out, "  (source: {})\n", r.source).ok();
        }
        ToolResult::text(out)
    }
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use tempfile::TempDir;
    use tokio_util::sync::CancellationToken;

    use super::*;

    fn make_ctx_with_db(db: &clankers_db::Db) -> ToolContext {
        ToolContext::new("test".to_string(), CancellationToken::new(), None).with_db(db.clone())
    }

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
    async fn test_jsonl_scan_finds_content() {
        let tmp = TempDir::new().unwrap();
        let mut f = std::fs::File::create(tmp.path().join("sess-001.jsonl")).unwrap();
        writeln!(f, r#"{{"role":"user","content":"fix the database migration"}}"#).ok();
        writeln!(f, r#"{{"role":"assistant","content":"I'll help with the migration"}}"#).ok();

        let tool = SessionSearchTool::new(tmp.path().to_path_buf(), 100);
        let ctx = make_ctx();

        let result = tool.execute(&ctx, json!({"query": "database migration"})).await;
        assert!(!result.is_error);
        let text = result_text(&result);
        assert!(text.contains("sess-001"));
        assert!(text.contains("database migration"));
    }

    #[tokio::test]
    async fn test_no_results() {
        let tmp = TempDir::new().unwrap();
        let tool = SessionSearchTool::new(tmp.path().to_path_buf(), 100);
        let ctx = make_ctx();

        let result = tool.execute(&ctx, json!({"query": "nonexistent query"})).await;
        assert!(!result.is_error);
        assert!(result_text(&result).contains("No sessions matching"));
    }

    #[tokio::test]
    async fn test_limit_respected() {
        let tmp = TempDir::new().unwrap();
        for i in 0..5 {
            let mut f = std::fs::File::create(tmp.path().join(format!("sess-{i:03}.jsonl"))).unwrap();
            writeln!(f, r#"{{"content":"keyword match here"}}"#).ok();
        }

        let tool = SessionSearchTool::new(tmp.path().to_path_buf(), 100);
        let ctx = make_ctx();

        let result = tool.execute(&ctx, json!({"query": "keyword", "limit": 2})).await;
        let text = result_text(&result);
        assert!(text.contains("Found 2 session"));
    }

    #[tokio::test]
    async fn test_index_search_with_db() {
        let db = clankers_db::Db::in_memory().unwrap();
        let entry = clankers_db::session_index::SessionIndexEntry {
            session_id: "idx-001".to_string(),
            cwd: "/home/test/proj".to_string(),
            model: "claude-sonnet".to_string(),
            created_at: chrono::Utc::now(),
            message_count: 10,
            first_prompt: "refactor the authentication module".to_string(),
            file_path: "/sessions/idx-001.jsonl".to_string(),
            agent: None,
            updated_at: chrono::Utc::now(),
        };
        db.sessions().upsert(&entry).unwrap();

        let tmp = TempDir::new().unwrap();
        let tool = SessionSearchTool::new(tmp.path().to_path_buf(), 100);
        let ctx = make_ctx_with_db(&db);

        let result = tool.execute(&ctx, json!({"query": "authentication"})).await;
        let text = result_text(&result);
        assert!(text.contains("idx-001"));
        assert!(text.contains("authentication"));
    }
}
