//! Session search tool — search or browse past session content.
//!
//! Search tiers: tantivy/index when available, then JSONL scan. With no query,
//! the tool browses recent sessions from JSONL files.

use std::collections::HashSet;
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
                description: concat!(
                    "Search past conversation sessions or browse recent sessions when query is omitted. ",
                    "Supports simple terms, quoted phrases, OR expressions like `foo OR bar`, ",
                    "and prefix terms like `deploy*`. Optional role_filter limits JSONL scanning to ",
                    "roles such as `user,assistant`. Returns compact session previews with snippets."
                )
                .to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "query": {
                            "type": "string",
                            "description": "Search query. Omit or leave empty to list recent sessions. Supports terms, quoted phrases, OR, and prefix* terms."
                        },
                        "cwd": {
                            "type": "string",
                            "description": "Filter to sessions from this working directory (optional)"
                        },
                        "role_filter": {
                            "type": "string",
                            "description": "Comma-separated roles to scan in JSONL content, e.g. 'user,assistant'"
                        },
                        "limit": {
                            "type": "integer",
                            "description": "Max results to return (default: 10)"
                        }
                    },
                    "required": []
                }),
            },
        }
    }

    fn session_files(&self) -> Vec<PathBuf> {
        let mut files: Vec<PathBuf> = std::fs::read_dir(&self.sessions_dir)
            .into_iter()
            .flatten()
            .flatten()
            .filter(|e| e.path().extension().and_then(|s| s.to_str()).is_some_and(|s| s == "jsonl"))
            .map(|e| e.path())
            .collect();
        files.sort_by(|a, b| {
            let ma = a.metadata().and_then(|m| m.modified()).ok();
            let mb = b.metadata().and_then(|m| m.modified()).ok();
            mb.cmp(&ma)
        });
        files
    }

    /// Tier 1: search session index metadata via redb.
    fn search_index(
        &self,
        db: &clankers_db::Db,
        matcher: &QueryMatcher,
        cwd: Option<&str>,
        limit: usize,
    ) -> Vec<SessionResult> {
        // The redb index API accepts a plain string. Use individual positive terms
        // for OR/prefix queries and de-duplicate below.
        let mut seen = HashSet::new();
        let mut out = Vec::new();
        for term in matcher.index_terms() {
            for e in db.sessions().search(&term).unwrap_or_default() {
                if out.len() >= limit {
                    return out;
                }
                if cwd.is_some() && cwd != Some(e.cwd.as_str()) {
                    continue;
                }
                if !seen.insert(e.session_id.clone()) {
                    continue;
                }
                let haystack = format!("{} {} {} {}", e.session_id, e.cwd, e.model, e.first_prompt);
                if !matcher.matches(&haystack) {
                    continue;
                }
                out.push(SessionResult {
                    session_id: e.session_id,
                    date: e.created_at.format("%Y-%m-%d %H:%M").to_string(),
                    model: e.model,
                    cwd: e.cwd,
                    preview: e.first_prompt.clone(),
                    source: "index".to_string(),
                });
            }
        }
        out
    }

    fn recent_jsonl(&self, cwd: Option<&str>, limit: usize) -> Vec<SessionResult> {
        let mut results = Vec::new();
        for (scanned, file) in self.session_files().into_iter().enumerate() {
            if scanned >= self.max_scan_files || results.len() >= limit {
                break;
            }
            let lines = read_lines(&file);
            let metadata = extract_metadata(&file, &lines);
            if let Some(filter_cwd) = cwd
                && metadata.cwd.as_deref() != Some(filter_cwd)
            {
                continue;
            }
            results.push(SessionResult {
                session_id: metadata.session_id,
                date: metadata.date,
                model: metadata.model.unwrap_or_default(),
                cwd: metadata.cwd.unwrap_or_default(),
                preview: first_human_preview(&lines).unwrap_or_else(|| "(no preview)".to_string()),
                source: "recent".to_string(),
            });
        }
        results
    }

    /// Tier 2: scan JSONL files for content matches.
    fn search_jsonl(
        &self,
        matcher: &QueryMatcher,
        cwd: Option<&str>,
        roles: &RoleFilter,
        limit: usize,
        exclude_ids: &[String],
    ) -> Vec<SessionResult> {
        let mut results = Vec::new();
        for (scanned, file) in self.session_files().into_iter().enumerate() {
            if scanned >= self.max_scan_files || results.len() >= limit {
                break;
            }

            let session_id = file.file_stem().and_then(|s| s.to_str()).unwrap_or("unknown").to_string();
            if exclude_ids.contains(&session_id) {
                continue;
            }

            let lines = read_lines(&file);
            let metadata = extract_metadata(&file, &lines);
            if let Some(filter_cwd) = cwd
                && metadata.cwd.as_deref() != Some(filter_cwd)
            {
                continue;
            }

            let mut matches_in_file = Vec::new();
            for (i, line) in lines.iter().enumerate() {
                if matches_in_file.len() >= 3 {
                    break;
                }
                let role = json_role(line);
                if !roles.allows(role.as_deref()) {
                    continue;
                }
                let searchable = json_content(line).unwrap_or_else(|| line.clone());
                if matcher.matches(&searchable) {
                    matches_in_file.push(snippet(&lines, i));
                }
            }
            if matches_in_file.is_empty() {
                continue;
            }

            results.push(SessionResult {
                session_id: metadata.session_id,
                date: metadata.date,
                model: metadata.model.unwrap_or_default(),
                cwd: metadata.cwd.unwrap_or_default(),
                preview: matches_in_file.join("\n---\n"),
                source: "content".to_string(),
            });
        }
        results
    }
}

#[derive(Default)]
struct SessionMetadata {
    session_id: String,
    date: String,
    model: Option<String>,
    cwd: Option<String>,
}

struct SessionResult {
    session_id: String,
    date: String,
    model: String,
    cwd: String,
    preview: String,
    source: String,
}

#[derive(Debug, Clone)]
struct QueryMatcher {
    groups: Vec<Vec<QueryTerm>>,
}

#[derive(Debug, Clone)]
enum QueryTerm {
    Contains(String),
    Prefix(String),
}

impl QueryMatcher {
    fn parse(query: &str) -> Self {
        let groups = split_or(query)
            .into_iter()
            .map(|part| parse_terms(&part))
            .filter(|terms| !terms.is_empty())
            .collect();
        Self { groups }
    }

    fn matches(&self, text: &str) -> bool {
        if self.groups.is_empty() {
            return true;
        }
        let lower = text.to_lowercase();
        self.groups.iter().any(|group| group.iter().all(|term| term.matches(&lower)))
    }

    fn index_terms(&self) -> Vec<String> {
        let mut terms = Vec::new();
        for group in &self.groups {
            for term in group {
                let value = match term {
                    QueryTerm::Contains(value) | QueryTerm::Prefix(value) => value.trim_end_matches('*'),
                };
                if !value.is_empty() && !terms.iter().any(|existing| existing == value) {
                    terms.push(value.to_string());
                }
            }
        }
        if terms.is_empty() {
            terms.push(String::new());
        }
        terms
    }
}

impl QueryTerm {
    fn matches(&self, lower: &str) -> bool {
        match self {
            QueryTerm::Contains(value) => lower.contains(value),
            QueryTerm::Prefix(prefix) => lower
                .split(|ch: char| !ch.is_alphanumeric() && ch != '_' && ch != '-')
                .any(|word| word.starts_with(prefix)),
        }
    }
}

#[derive(Debug, Default)]
struct RoleFilter {
    roles: Option<HashSet<String>>,
}

impl RoleFilter {
    fn parse(value: Option<&str>) -> Self {
        let roles = value.map(|raw| {
            raw.split(',')
                .map(|role| role.trim().to_lowercase())
                .filter(|role| !role.is_empty())
                .collect::<HashSet<_>>()
        });
        Self {
            roles: roles.filter(|roles| !roles.is_empty()),
        }
    }

    fn allows(&self, role: Option<&str>) -> bool {
        let Some(roles) = &self.roles else {
            return true;
        };
        role.map(|role| roles.contains(&role.to_lowercase())).unwrap_or(false)
    }
}

#[async_trait]
impl Tool for SessionSearchTool {
    fn definition(&self) -> &ToolDefinition {
        &self.definition
    }

    async fn execute(&self, ctx: &ToolContext, params: Value) -> ToolResult {
        let query = params.get("query").and_then(|v| v.as_str()).unwrap_or("").trim();
        let cwd = params.get("cwd").and_then(|v| v.as_str());
        let roles = RoleFilter::parse(params.get("role_filter").and_then(|v| v.as_str()));
        let limit = usize::try_from(params.get("limit").and_then(|v| v.as_u64()).unwrap_or(10)).unwrap_or(10);

        if query.is_empty() {
            let results = self.recent_jsonl(cwd, limit);
            if results.is_empty() {
                return ToolResult::text("No recent sessions found.".to_string());
            }
            return ToolResult::text(format_results("Recent sessions", &results));
        }

        let matcher = QueryMatcher::parse(query);
        let mut results = search_tantivy(ctx, query, &matcher, limit, cwd);

        if results.len() < limit
            && let Some(db) = ctx.db()
        {
            let exclude_ids: Vec<String> = results.iter().map(|r| r.session_id.clone()).collect();
            let idx_results = self.search_index(db, &matcher, cwd, limit - results.len());
            results.extend(idx_results.into_iter().filter(|r| !exclude_ids.contains(&r.session_id)));
        }

        if results.len() < limit {
            let exclude_ids: Vec<String> = results.iter().map(|r| r.session_id.clone()).collect();
            let remaining = limit - results.len();
            let jsonl_results = self.search_jsonl(&matcher, cwd, &roles, remaining, &exclude_ids);
            results.extend(jsonl_results);
        }

        if results.is_empty() {
            return ToolResult::text(format!("No sessions matching '{query}'."));
        }
        ToolResult::text(format_results(&format!("Found {} session(s)", results.len()), &results))
    }
}

fn search_tantivy(
    ctx: &ToolContext,
    query: &str,
    matcher: &QueryMatcher,
    limit: usize,
    cwd: Option<&str>,
) -> Vec<SessionResult> {
    if cwd.is_some() {
        return Vec::new();
    }
    let Some(search_index) = ctx.search_index() else {
        return Vec::new();
    };
    let Ok(hits) = search_index.search(query, limit * 3) else {
        return Vec::new();
    };
    let mut seen_sessions = HashSet::new();
    hits.into_iter()
        .filter(|h| seen_sessions.insert(h.session_id.clone()))
        .filter(|h| matcher.matches(&h.snippet))
        .take(limit)
        .map(|h| SessionResult {
            session_id: h.session_id,
            date: chrono::DateTime::from_timestamp(h.timestamp, 0)
                .map(|dt| dt.format("%Y-%m-%d %H:%M").to_string())
                .unwrap_or_default(),
            model: String::new(),
            cwd: String::new(),
            preview: h.snippet,
            source: "fts".to_string(),
        })
        .collect()
}

fn format_results(title: &str, results: &[SessionResult]) -> String {
    let mut out = format!("{title}:\n\n");
    for r in results {
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
    out
}

fn split_or(query: &str) -> Vec<String> {
    let mut groups = Vec::new();
    let mut current = String::new();
    let mut in_quote = false;
    let chars: Vec<char> = query.chars().collect();
    let mut i = 0usize;
    while i < chars.len() {
        if chars[i] == '"' {
            in_quote = !in_quote;
            current.push(chars[i]);
            i += 1;
            continue;
        }
        if !in_quote && starts_or_at(&chars, i) {
            groups.push(current.trim().to_string());
            current.clear();
            i += 2;
            continue;
        }
        current.push(chars[i]);
        i += 1;
    }
    groups.push(current.trim().to_string());
    groups
}

fn starts_or_at(chars: &[char], i: usize) -> bool {
    if i + 1 >= chars.len() || !chars[i].eq_ignore_ascii_case(&'o') || !chars[i + 1].eq_ignore_ascii_case(&'r') {
        return false;
    }
    let before_ok = i == 0 || chars[i - 1].is_whitespace();
    let after_ok = i + 2 >= chars.len() || chars[i + 2].is_whitespace();
    before_ok && after_ok
}

fn parse_terms(query: &str) -> Vec<QueryTerm> {
    let mut terms = Vec::new();
    let mut current = String::new();
    let mut in_quote = false;
    for ch in query.chars() {
        match ch {
            '"' => {
                if in_quote {
                    push_term(&mut terms, &current, false);
                    current.clear();
                    in_quote = false;
                } else {
                    if !current.trim().is_empty() {
                        push_term(&mut terms, &current, true);
                        current.clear();
                    }
                    in_quote = true;
                }
            }
            ch if ch.is_whitespace() && !in_quote => {
                push_term(&mut terms, &current, true);
                current.clear();
            }
            _ => current.push(ch),
        }
    }
    push_term(&mut terms, &current, !in_quote);
    terms
}

fn push_term(terms: &mut Vec<QueryTerm>, raw: &str, allow_prefix: bool) {
    let term = raw.trim().to_lowercase();
    if term.is_empty() {
        return;
    }
    if allow_prefix && term.ends_with('*') && term.len() > 1 {
        terms.push(QueryTerm::Prefix(term.trim_end_matches('*').to_string()));
    } else {
        terms.push(QueryTerm::Contains(term));
    }
}

fn read_lines(file: &PathBuf) -> Vec<String> {
    let reader = match std::fs::File::open(file) {
        Ok(file) => std::io::BufReader::new(file),
        Err(_) => return Vec::new(),
    };
    reader.lines().map_while(Result::ok).collect()
}

fn extract_metadata(file: &PathBuf, lines: &[String]) -> SessionMetadata {
    let mut metadata = SessionMetadata {
        session_id: file.file_stem().and_then(|s| s.to_str()).unwrap_or("unknown").to_string(),
        date: file
            .metadata()
            .and_then(|m| m.modified())
            .ok()
            .map(|t| {
                let d: chrono::DateTime<chrono::Utc> = t.into();
                d.format("%Y-%m-%d %H:%M").to_string()
            })
            .unwrap_or_else(|| "unknown".to_string()),
        ..Default::default()
    };

    for line in lines.iter().take(5) {
        let Ok(value) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        if metadata.cwd.is_none() {
            metadata.cwd = value.get("cwd").and_then(|v| v.as_str()).map(String::from);
        }
        if metadata.model.is_none() {
            metadata.model = value.get("model").and_then(|v| v.as_str()).map(String::from);
        }
        if let Some(session_id) = value.get("session_id").and_then(|v| v.as_str()) {
            metadata.session_id = session_id.to_string();
        }
    }
    metadata
}

fn first_human_preview(lines: &[String]) -> Option<String> {
    lines
        .iter()
        .filter_map(|line| json_content(line))
        .find(|content| !content.trim().is_empty())
        .map(|content| truncate(&content, 240))
}

fn snippet(lines: &[String], index: usize) -> String {
    let start = index.saturating_sub(1);
    let end = (index + 2).min(lines.len());
    let context = lines[start..end]
        .iter()
        .filter_map(|line| json_content(line).or_else(|| Some(line.clone())))
        .collect::<Vec<_>>()
        .join(" ");
    truncate(&context, 240)
}

fn truncate(text: &str, max_len: usize) -> String {
    if text.len() <= max_len {
        text.to_string()
    } else {
        format!("{}...", &text[..max_len])
    }
}

fn json_role(line: &str) -> Option<String> {
    serde_json::from_str::<Value>(line)
        .ok()
        .and_then(|value| value.get("role").and_then(|role| role.as_str()).map(String::from))
}

fn json_content(line: &str) -> Option<String> {
    let value = serde_json::from_str::<Value>(line).ok()?;
    if let Some(content) = value.get("content").and_then(|content| content.as_str()) {
        return Some(content.to_string());
    }
    if let Some(text) = value.get("text").and_then(|content| content.as_str()) {
        return Some(text.to_string());
    }
    None
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
    async fn test_no_query_lists_recent() {
        let tmp = TempDir::new().unwrap();
        let mut f = std::fs::File::create(tmp.path().join("sess-000.jsonl")).unwrap();
        writeln!(f, r#"{{"role":"user","content":"recent prompt"}}"#).ok();

        let result = SessionSearchTool::new(tmp.path().to_path_buf(), 100).execute(&make_ctx(), json!({})).await;
        let text = result_text(&result);
        assert!(text.contains("Recent sessions"));
        assert!(text.contains("recent prompt"));
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
    async fn test_role_filter_excludes_tool_noise() {
        let tmp = TempDir::new().unwrap();
        let mut f = std::fs::File::create(tmp.path().join("sess-role.jsonl")).unwrap();
        writeln!(f, r#"{{"role":"tool","content":"secret needle"}}"#).ok();
        writeln!(f, r#"{{"role":"user","content":"ordinary prompt"}}"#).ok();

        let tool = SessionSearchTool::new(tmp.path().to_path_buf(), 100);
        let result = tool.execute(&make_ctx(), json!({"query": "needle", "role_filter": "user,assistant"})).await;
        assert!(result_text(&result).contains("No sessions matching"));
    }

    #[tokio::test]
    async fn test_or_phrase_and_prefix_queries() {
        let tmp = TempDir::new().unwrap();
        let mut f1 = std::fs::File::create(tmp.path().join("sess-or.jsonl")).unwrap();
        writeln!(f1, r#"{{"role":"user","content":"debug matrix bridge"}}"#).ok();
        let mut f2 = std::fs::File::create(tmp.path().join("sess-prefix.jsonl")).unwrap();
        writeln!(f2, r#"{{"role":"user","content":"deploying daemon now"}}"#).ok();

        let tool = SessionSearchTool::new(tmp.path().to_path_buf(), 100);
        let or_text = result_text(&tool.execute(&make_ctx(), json!({"query": "\"matrix bridge\" OR missing"})).await);
        assert!(or_text.contains("sess-or"));
        let prefix_text = result_text(&tool.execute(&make_ctx(), json!({"query": "deploy*"})).await);
        assert!(prefix_text.contains("sess-prefix"));
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
