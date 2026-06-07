//! External memory provider tool adapter.
//!
//! External memory stays disabled by default. Local provider search uses the
//! repo-local memory database, while HTTP providers require explicit endpoint,
//! credential, timeout, and result-limit policy before any network contact.

use std::fmt::Write;
use std::time::Duration;
use std::time::Instant;

use async_trait::async_trait;
use clankers_config::ExternalMemoryConfigError;
use clankers_config::ExternalMemoryProvider;
use clankers_config::ExternalMemorySettings;
use clankers_db::memory::MemoryEntry;
use clankers_db::memory::MemoryScope;
use clankers_tool_host::ToolHostServiceKind;
use clankers_tool_host::ToolInvocationContext;
use clankers_tool_host::ToolSearchRequest;
use serde::Deserialize;
use serde::Serialize;
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
                    "Supports status plus disabled-by-default local and HTTP search providers. ",
                    "HTTP providers require endpoint and credentialEnv and attach replay-safe metadata."
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

    async fn http_search(&self, params: &Value, started: Instant) -> ToolResult {
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

        let credential_env = self.settings.credential_env.as_deref().unwrap_or_default().trim();
        let credential = match std::env::var(credential_env) {
            Ok(value) if !value.trim().is_empty() => value,
            _ => {
                let elapsed_ms = started.elapsed().as_millis();
                return ToolResult::error(format!(
                    "external_memory HTTP provider credential is unavailable: set `{credential_env}`"
                ))
                .with_details(self.error_details(
                    "search",
                    elapsed_ms,
                    "missing_credential",
                    "credential environment variable missing or blank",
                ));
            }
        };

        let limit = bounded_limit(params.get("limit"), self.settings.max_results);
        let endpoint = self.settings.endpoint.as_deref().unwrap_or_default().trim();
        let timeout_ms = self.settings.timeout_ms.unwrap_or(10_000);
        let request = RemoteSearchRequest { query, limit };
        let client = match reqwest::Client::builder().timeout(Duration::from_millis(timeout_ms)).build() {
            Ok(client) => client,
            Err(error) => {
                let elapsed_ms = started.elapsed().as_millis();
                return ToolResult::error(format!(
                    "external_memory HTTP client setup failed: {}",
                    redact_error(&error.to_string())
                ))
                .with_details(self.error_details(
                    "search",
                    elapsed_ms,
                    "client_error",
                    &error.to_string(),
                ));
            }
        };

        let response = match client.post(endpoint).bearer_auth(credential).json(&request).send().await {
            Ok(response) => response,
            Err(error) => {
                let elapsed_ms = started.elapsed().as_millis();
                return ToolResult::error(format!(
                    "external_memory HTTP request failed: {}",
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

        let status = response.status();
        if !status.is_success() {
            let elapsed_ms = started.elapsed().as_millis();
            return ToolResult::error(format!("external_memory HTTP provider returned status {status}"))
                .with_details(self.error_details("search", elapsed_ms, "provider_status", &status.to_string()));
        }

        let payload = match response.json::<RemoteSearchResponse>().await {
            Ok(payload) => payload,
            Err(error) => {
                let elapsed_ms = started.elapsed().as_millis();
                return ToolResult::error(format!(
                    "external_memory HTTP response was invalid: {}",
                    redact_error(&error.to_string())
                ))
                .with_details(self.error_details(
                    "search",
                    elapsed_ms,
                    "invalid_response",
                    &error.to_string(),
                ));
            }
        };
        let results = payload.results.into_iter().take(limit).collect::<Vec<_>>();
        let elapsed_ms = started.elapsed().as_millis();
        let mut out = format!(
            "Found {} external memor{} for '{query}':\n",
            results.len(),
            if results.len() == 1 { "y" } else { "ies" }
        );
        for result in &results {
            let label = result.id.as_deref().unwrap_or("remote");
            writeln!(out, "- [{label}] (remote) {}", result.text).ok();
        }
        ToolResult::text(out).with_details(json!({
            "source": SOURCE,
            "providerKind": provider_kind(self.settings.provider),
            "providerName": self.settings.safe_provider_name(),
            "action": "search",
            "status": "ok",
            "elapsedMs": elapsed_ms,
            "resultCount": results.len(),
            "injectIntoPrompt": self.settings.inject_into_prompt,
        }))
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

        let db = match ctx.service::<clankers_db::Db>() {
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

    fn local_search_neutral(&self, context: &ToolInvocationContext, params: &Value, started: Instant) -> ToolResult {
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

        if let Err(outcome) = context.require_service(&self.definition.name, ToolHostServiceKind::Search) {
            let elapsed_ms = started.elapsed().as_millis();
            return ToolResult::error(format!(
                "external_memory local provider requires neutral search service: {outcome:?}"
            ))
            .with_details(self.error_details(
                "search",
                elapsed_ms,
                "missing_search_service",
                "neutral search service unavailable",
            ));
        }
        let search = match context.search.as_ref() {
            Some(search) => search,
            None => {
                let elapsed_ms = started.elapsed().as_millis();
                return ToolResult::error("external_memory local provider requires neutral search service")
                    .with_details(self.error_details(
                        "search",
                        elapsed_ms,
                        "missing_search_service",
                        "neutral search service unavailable",
                    ));
            }
        };

        let limit = bounded_limit(params.get("limit"), self.settings.max_results);
        let scope = params.get("scope").and_then(|value| value.as_str()).unwrap_or("all");
        let request_limit = u32::try_from(limit).unwrap_or(u32::MAX);
        let results = match search.search(ToolSearchRequest::new(query, request_limit).with_metadata("scope", scope)) {
            Ok(result) => result.hits,
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
        for hit in &results {
            let id = hit.metadata.get("memory_id").map(String::as_str).unwrap_or(hit.title.as_str());
            let scope = hit.metadata.get("scope").map(String::as_str).unwrap_or("unknown");
            writeln!(out, "- [{id}] ({scope}) {}", hit.snippet).ok();
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
                ExternalMemoryProvider::Http => self.http_search(&params, started).await,
            },
            other => {
                let elapsed_ms = started.elapsed().as_millis();
                ToolResult::error(format!("Unknown external_memory action '{other}'. Use 'search' or 'status'."))
                    .with_details(self.error_details(other, elapsed_ms, "unknown_action", "unknown action"))
            }
        }
    }

    fn uses_neutral_tool_context(&self) -> bool {
        true
    }

    async fn execute_with_neutral_context(&self, context: ToolInvocationContext, params: Value) -> ToolResult {
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
                ExternalMemoryProvider::Local => self.local_search_neutral(&context, &params, started),
                ExternalMemoryProvider::Http => self.http_search(&params, started).await,
            },
            other => {
                let elapsed_ms = started.elapsed().as_millis();
                ToolResult::error(format!("Unknown external_memory action '{other}'. Use 'search' or 'status'."))
                    .with_details(self.error_details(other, elapsed_ms, "unknown_action", "unknown action"))
            }
        }
    }
}

#[derive(Debug, Serialize)]
struct RemoteSearchRequest<'a> {
    query: &'a str,
    limit: usize,
}

#[derive(Debug, Deserialize)]
struct RemoteSearchResponse {
    #[serde(default)]
    results: Vec<RemoteSearchResult>,
}

#[derive(Debug, Deserialize)]
struct RemoteSearchResult {
    #[serde(default)]
    id: Option<String>,
    text: String,
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
        ExternalMemoryConfigError::MissingCredentialEnv => "missing_credential_env",
        ExternalMemoryConfigError::BlankEndpoint => "blank_endpoint",
        ExternalMemoryConfigError::BlankCredentialEnv => "blank_credential_env",
        ExternalMemoryConfigError::NonPositiveTimeout => "non_positive_timeout",
        ExternalMemoryConfigError::NonPositiveMaxResults => "non_positive_max_results",
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
    use std::io::Read;
    use std::io::Write;
    use std::net::TcpListener;
    use std::sync::Arc;
    use std::time::Duration;

    use clankers_db::Db;
    use tokio_util::sync::CancellationToken;

    use super::*;

    fn make_ctx(db: &Db) -> ToolContext {
        ToolContext::new("test".to_string(), CancellationToken::new(), None).with_service(std::sync::Arc::new(db.clone()))
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

    fn spawn_memory_server() -> String {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind fake memory server");
        let addr = listener.local_addr().expect("fake server address");
        std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("accept fake request");
            stream.set_read_timeout(Some(Duration::from_secs(5))).expect("set read timeout");
            let mut buffer = [0_u8; 4096];
            let bytes = stream.read(&mut buffer).expect("read request");
            let request = String::from_utf8_lossy(&buffer[..bytes]);
            assert!(request.contains("POST /memory HTTP/1.1"), "unexpected request: {request}");
            assert!(
                request.contains("authorization: Bearer test-token")
                    || request.contains("Authorization: Bearer test-token")
            );
            assert!(request.contains(r#""query":"Rust""#));
            assert!(request.contains(r#""limit":1"#));
            let body = r#"{"results":[{"id":"r1","text":"Rust remote memory"},{"id":"r2","text":"extra memory"}]}"#;
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                body.len(),
                body
            );
            stream.write_all(response.as_bytes()).expect("write response");
        });
        format!("http://{addr}/memory")
    }

    struct FakeSearchService;

    impl clankers_tool_host::ToolSearchService for FakeSearchService {
        fn search(
            &self,
            request: clankers_tool_host::ToolSearchRequest,
        ) -> std::result::Result<clankers_tool_host::ToolSearchResult, clankers_tool_host::ToolHostError> {
            let scope = request.metadata.get("scope").cloned().unwrap_or_else(|| "all".to_string());
            Ok(clankers_tool_host::ToolSearchResult {
                hits: vec![
                    clankers_tool_host::ToolSearchHit::new("m1", format!("{} result", request.query), 1)
                        .with_metadata("memory_id", "m1")
                        .with_metadata("scope", scope),
                ],
            })
        }
    }

    fn neutral_search_context() -> clankers_tool_host::ToolInvocationContext {
        clankers_tool_host::ToolInvocationContext::new("external-memory-call")
            .with_services(clankers_tool_host::ToolHostServices::empty().with_service(
                clankers_tool_host::ToolHostServiceHandle::available(clankers_tool_host::ToolHostServiceKind::Search),
            ))
            .with_search_service(Arc::new(FakeSearchService))
    }

    fn http_settings(endpoint: String, credential_env: &str) -> ExternalMemorySettings {
        ExternalMemorySettings {
            enabled: true,
            provider: ExternalMemoryProvider::Http,
            name: Some("remote-memory".to_string()),
            endpoint: Some(endpoint),
            credential_env: Some(credential_env.to_string()),
            timeout_ms: Some(5_000),
            max_results: 1,
            inject_into_prompt: false,
        }
    }

    #[tokio::test]
    async fn http_search_returns_bounded_replay_safe_results() {
        let endpoint = spawn_memory_server();
        let env_name = "CLANKERS_TEST_EXTERNAL_MEMORY_TOKEN_HTTP_SEARCH";
        unsafe {
            std::env::set_var(env_name, "test-token");
        }
        let tool = ExternalMemoryTool::new(http_settings(endpoint, env_name));
        let result = tool
            .execute(
                &ToolContext::new("test".to_string(), CancellationToken::new(), None),
                json!({"action": "search", "query": "Rust", "limit": 5}),
            )
            .await;

        assert!(!result.is_error, "expected HTTP search to succeed: {}", result_text(&result));
        assert!(result_text(&result).contains("Rust remote memory"));
        assert!(!result_text(&result).contains("extra memory"), "HTTP results are bounded by configured maxResults");
        let details = result.details.as_ref().expect("HTTP search attaches details");
        assert_eq!(details.get("providerKind").and_then(Value::as_str), Some("http"));
        assert_eq!(details.get("providerName").and_then(Value::as_str), Some("remote-memory"));
        assert_eq!(details.get("resultCount").and_then(Value::as_u64), Some(1));
        assert!(details.get("query").is_none());
        assert!(details.get("results").is_none());
        assert!(details.get("credentialEnv").is_none());
    }

    #[tokio::test]
    async fn http_search_fails_closed_when_credential_missing() {
        let tool = ExternalMemoryTool::new(http_settings(
            "http://127.0.0.1:9/memory".to_string(),
            "CLANKERS_TEST_EXTERNAL_MEMORY_TOKEN_MISSING",
        ));
        unsafe {
            std::env::remove_var("CLANKERS_TEST_EXTERNAL_MEMORY_TOKEN_MISSING");
        }
        let result = tool
            .execute(
                &ToolContext::new("test".to_string(), CancellationToken::new(), None),
                json!({"action": "search", "query": "Rust"}),
            )
            .await;

        assert!(result.is_error);
        assert!(result_text(&result).contains("credential is unavailable"));
        assert_eq!(
            result.details.as_ref().and_then(|details| details.get("errorKind")).and_then(Value::as_str),
            Some("missing_credential")
        );
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
    async fn neutral_local_search_uses_injected_search_service() {
        let tool = ExternalMemoryTool::new(enabled_settings());
        let result = tool
            .execute_with_neutral_context(
                neutral_search_context(),
                json!({"action": "search", "query": "Rust", "limit": 1, "scope": "global"}),
            )
            .await;

        assert!(!result.is_error, "neutral local search should succeed: {}", result_text(&result));
        assert!(result_text(&result).contains("Rust result"));
        assert!(result_text(&result).contains("(global)"));
        assert_eq!(
            result.details.as_ref().and_then(|details| details.get("resultCount")).and_then(Value::as_u64),
            Some(1)
        );
    }

    #[tokio::test]
    async fn neutral_local_search_fails_closed_without_search_service() {
        let tool = ExternalMemoryTool::new(enabled_settings());
        let result = tool
            .execute_with_neutral_context(
                clankers_tool_host::ToolInvocationContext::new("external-memory-call"),
                json!({"action": "search", "query": "Rust"}),
            )
            .await;

        assert!(result.is_error);
        assert!(result_text(&result).contains("requires neutral search service"));
        assert_eq!(
            result.details.as_ref().and_then(|details| details.get("errorKind")).and_then(Value::as_str),
            Some("missing_search_service")
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

    #[tokio::test]
    async fn local_search_metadata_is_safe_for_replay() {
        let db = Db::in_memory().unwrap();
        db.memory().save(&MemoryEntry::new("project token handling notes", MemoryScope::Global)).unwrap();
        let tool = ExternalMemoryTool::new(enabled_settings());
        let result = tool.execute(&make_ctx(&db), json!({"action": "search", "query": "token", "limit": 2})).await;

        assert!(!result.is_error);
        let details = result.details.as_ref().expect("external memory stores details");
        assert_eq!(details.get("source").and_then(Value::as_str), Some(SOURCE));
        assert_eq!(details.get("providerKind").and_then(Value::as_str), Some("local"));
        assert_eq!(details.get("action").and_then(Value::as_str), Some("search"));
        assert_eq!(details.get("status").and_then(Value::as_str), Some("ok"));
        assert_eq!(details.get("resultCount").and_then(Value::as_u64), Some(1));
        assert!(details.get("query").is_none(), "metadata must not persist raw queries");
        assert!(details.get("results").is_none(), "metadata must not persist memory text");
        assert!(details.get("credentialEnv").is_none(), "metadata must not persist env-var values");
    }

    #[test]
    fn secret_like_errors_are_redacted_in_metadata() {
        assert_eq!(redact_error("bearer token leaked"), "[REDACTED]");
        assert_eq!(redact_error("plain provider unavailable"), "plain provider unavailable");
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
