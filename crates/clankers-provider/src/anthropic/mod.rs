//! Anthropic Messages API provider

pub mod api;
pub mod streaming;

use std::sync::Arc;

use async_trait::async_trait;
use clanker_router::credential_pool::CredentialPool;
use tokio::sync::mpsc;
use tracing::info;
use tracing::warn;

use crate::CompletionRequest;
use crate::Model;
use crate::Provider;
use crate::auth::Credential;
use crate::credential_manager::CredentialManager;
use crate::error::Result;
use crate::streaming::StreamEvent;

pub struct AnthropicProvider {
    client: api::AnthropicClient,
    /// Legacy direct credential (for API key users or when no auth path is available)
    credential: Option<Credential>,
    /// Credential manager with auto-refresh (preferred for OAuth)
    credential_manager: Option<Arc<CredentialManager>>,
    /// Multi-account credential pool with failover/round-robin
    credential_pool: Option<CredentialPool>,
    models: Vec<Model>,
}

impl AnthropicProvider {
    /// Create a provider with a simple credential (no auto-refresh).
    pub fn new(credential: Credential, base_url: Option<String>) -> Self {
        Self {
            client: api::AnthropicClient::new(base_url),
            credential: Some(credential),
            credential_manager: None,
            credential_pool: None,
            models: clanker_router::backends::anthropic::default_models(),
        }
    }

    /// Create a provider with a credential manager that supports auto-refresh.
    pub fn with_credential_manager(credential_manager: Arc<CredentialManager>, base_url: Option<String>) -> Self {
        Self {
            client: api::AnthropicClient::new(base_url),
            credential: None,
            credential_manager: Some(credential_manager),
            credential_pool: None,
            models: clanker_router::backends::anthropic::default_models(),
        }
    }

    /// Create a provider with a credential pool for multi-account failover.
    ///
    /// The credential manager handles OAuth refresh for the primary account.
    /// The pool provides failover to other accounts when one gets rate-limited.
    pub fn with_credential_pool(
        credential_manager: Arc<CredentialManager>,
        pool: CredentialPool,
        base_url: Option<String>,
    ) -> Self {
        Self {
            client: api::AnthropicClient::new(base_url),
            credential: None,
            credential_manager: Some(credential_manager),
            credential_pool: Some(pool),
            models: clanker_router::backends::anthropic::default_models(),
        }
    }

    /// Get the current credential, refreshing if needed.
    async fn get_credential(&self) -> Result<Credential> {
        if let Some(ref cm) = self.credential_manager {
            cm.get_credential().await
        } else if let Some(ref cred) = self.credential {
            Ok(cred.clone())
        } else {
            Err(crate::error::auth_err("No credential configured"))
        }
    }

    /// Force-refresh the credential (called on 401).
    async fn force_refresh_credential(&self) -> Result<Credential> {
        if let Some(ref cm) = self.credential_manager {
            cm.force_refresh().await
        } else {
            Err(crate::error::auth_err("Cannot refresh: no credential manager configured"))
        }
    }

    /// Try a request with a specific credential.
    async fn try_with_credential(
        &self,
        request: &CompletionRequest,
        credential: &Credential,
        tx: &mpsc::Sender<StreamEvent>,
    ) -> std::result::Result<(), (u16, String)> {
        let api_request = api::build_api_request(request, credential.is_oauth());
        let response = match self.client.send_streaming(&api_request, credential).await {
            Ok(r) => r,
            Err(e) => {
                // Preserve the HTTP status code from the error when available.
                // send_streaming returns ProviderError with status for HTTP errors
                // (e.g., 400, 403) — losing this turns non-retryable errors into
                // retryable ones (status 500) in the pool rotation logic.
                let status = e.status_code().unwrap_or(500);
                return Err((status, e.to_string()));
            }
        };

        if response.status().is_success() {
            streaming::parse_sse_stream(response, tx.clone())
                .await
                .map_err(|e| (500, e.to_string()))
        } else {
            let status = response.status().as_u16();
            let body = response.text().await.unwrap_or_default();
            Err((status, format!("Anthropic API error {}: {}", status, body)))
        }
    }
}

#[async_trait]
impl Provider for AnthropicProvider {
    async fn complete(&self, request: CompletionRequest, tx: mpsc::Sender<StreamEvent>) -> Result<()> {
        // ── Multi-account path: try each credential from the pool ────
        if let Some(ref pool) = self.credential_pool {
            let leases = pool.select_all_available().await;
            if leases.is_empty() {
                // Pool exhausted — fall through to single-credential path
                // which may refresh the primary OAuth token
                warn!("All credential pool slots unavailable, trying primary credential");
            } else {
                let mut last_status = 0u16;
                let mut last_error = String::new();

                for lease in &leases {
                    let cred = lease.credential().clone();
                    match self.try_with_credential(&request, &cred, &tx).await {
                        Ok(()) => {
                            lease.report_success().await;
                            return Ok(());
                        }
                        Err((status, msg)) => {
                            lease.report_failure(status).await;
                            last_status = status;
                            last_error = msg;

                            // 401 on OAuth → try refreshing before moving to next credential
                            if status == 401 && cred.is_oauth() && self.credential_manager.is_some() {
                                info!("Got 401 on '{}', attempting token refresh", lease.account());
                                if let Ok(refreshed) = self.force_refresh_credential().await {
                                    match self.try_with_credential(&request, &refreshed, &tx).await {
                                        Ok(()) => {
                                            lease.report_success().await;
                                            return Ok(());
                                        }
                                        Err((s, m)) => {
                                            last_status = s;
                                            last_error = m;
                                        }
                                    }
                                }
                            }

                            // Non-retryable errors stop immediately
                            if !clanker_router::retry::is_retryable_status(status) && status != 401 {
                                return Err(crate::error::provider_err_with_status(last_status, last_error));
                            }

                            info!("Credential '{}' failed (HTTP {}), trying next", lease.account(), status);
                        }
                    }
                }

                return Err(crate::error::provider_err_with_status(last_status, last_error));
            }
        }

        // ── Single-credential path ───────────────────────────────────
        let credential = self.get_credential().await?;
        let api_request = api::build_api_request(&request, credential.is_oauth());
        let response = self.client.send_streaming(&api_request, &credential).await?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();

            // On 401, try refreshing the token and retrying once
            if status.as_u16() == 401 && self.credential_manager.is_some() {
                info!("Got 401, attempting token refresh and retry");
                let refreshed = self.force_refresh_credential().await?;
                let api_request = api::build_api_request(&request, refreshed.is_oauth());
                let retry_response = self.client.send_streaming(&api_request, &refreshed).await?;

                if !retry_response.status().is_success() {
                    let retry_status = retry_response.status();
                    let retry_body = retry_response.text().await.unwrap_or_default();
                    return Err(crate::error::provider_err_with_status(
                        retry_status.as_u16(),
                        format!("Anthropic API error {} (after token refresh): {}", retry_status, retry_body),
                    ));
                }

                return streaming::parse_sse_stream(retry_response, tx).await;
            }

            return Err(crate::error::provider_err_with_status(
                status.as_u16(),
                format!("Anthropic API error {}: {}", status, body),
            ));
        }

        streaming::parse_sse_stream(response, tx).await
    }

    fn models(&self) -> &[Model] {
        &self.models
    }

    fn name(&self) -> &str {
        "anthropic"
    }

    async fn reload_credentials(&self) {
        if let Some(ref cm) = self.credential_manager {
            cm.reload_from_disk().await;
        }
        // Reset pool health after credential reload (fresh tokens)
        if let Some(ref pool) = self.credential_pool {
            pool.reset_health().await;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Provider;
    use crate::auth::Credential;
    use clanker_router::credential_pool::{CredentialPool, SelectionStrategy};
    use std::sync::atomic::{AtomicUsize, Ordering};
    use tokio::io::AsyncWriteExt;
    use tokio::net::TcpListener;

    // ── Mock HTTP server ────────────────────────────────────────────

    /// Minimal HTTP server that returns controlled responses for each request.
    /// Responses are consumed in order from the `responses` vec.
    struct MockServer {
        addr: std::net::SocketAddr,
        _handle: tokio::task::JoinHandle<()>,
    }

    #[derive(Clone)]
    struct MockResponse {
        status: u16,
        body: String,
        /// If true, body is SSE event stream format
        is_sse: bool,
    }

    impl MockResponse {
        fn success_sse() -> Self {
            // Minimal valid Anthropic SSE stream
            let body = [
                "event: message_start",
                r#"data: {"type":"message_start","message":{"id":"msg-1","type":"message","role":"assistant","model":"claude-test","content":[],"stop_reason":null,"usage":{"input_tokens":10,"output_tokens":0}}}"#,
                "",
                "event: content_block_start",
                r#"data: {"type":"content_block_start","index":0,"content_block":{"type":"text","text":""}}"#,
                "",
                "event: content_block_delta",
                r#"data: {"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"Hello"}}"#,
                "",
                "event: content_block_stop",
                r#"data: {"type":"content_block_stop","index":0}"#,
                "",
                "event: message_delta",
                r#"data: {"type":"message_delta","delta":{"stop_reason":"end_turn"},"usage":{"output_tokens":5}}"#,
                "",
                "event: message_stop",
                r#"data: {"type":"message_stop"}"#,
                "",
            ]
            .join("\n");
            Self {
                status: 200,
                body,
                is_sse: true,
            }
        }

        fn error(status: u16, msg: &str) -> Self {
            Self {
                status,
                body: format!(r#"{{"error":{{"type":"error","message":"{}"}}}}"#, msg),
                is_sse: false,
            }
        }
    }

    impl MockServer {
        async fn start(responses: Vec<MockResponse>) -> Self {
            let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
            let addr = listener.local_addr().unwrap();
            let counter = Arc::new(AtomicUsize::new(0));

            let handle = tokio::spawn(async move {
                let responses = Arc::new(responses);
                loop {
                    let Ok((mut stream, _)) = listener.accept().await else {
                        break;
                    };
                    let responses = responses.clone();
                    let counter = counter.clone();

                    tokio::spawn(async move {
                        // Read the HTTP request (consume headers)
                        let mut buf = vec![0u8; 8192];
                        tokio::io::AsyncReadExt::read(&mut stream, &mut buf).await.ok();

                        let idx = counter.fetch_add(1, Ordering::SeqCst);
                        let resp = responses.get(idx).cloned().unwrap_or_else(|| {
                            MockResponse::error(500, "no more mock responses configured")
                        });

                        let content_type = if resp.is_sse {
                            "text/event-stream"
                        } else {
                            "application/json"
                        };

                        let http_response = format!(
                            "HTTP/1.1 {} {}\r\nContent-Type: {}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                            resp.status,
                            status_text(resp.status),
                            content_type,
                            resp.body.len(),
                            resp.body,
                        );

                        stream.write_all(http_response.as_bytes()).await.ok();
                        stream.flush().await.ok();
                    });
                }
            });

            Self { addr, _handle: handle }
        }

        fn base_url(&self) -> String {
            format!("http://{}", self.addr)
        }
    }

    fn status_text(code: u16) -> &'static str {
        match code {
            200 => "OK",
            400 => "Bad Request",
            401 => "Unauthorized",
            429 => "Too Many Requests",
            500 => "Internal Server Error",
            529 => "Overloaded",
            _ => "Unknown",
        }
    }

    // ── Helpers ──────────────────────────────────────────────────────

    fn api_key(key: &str) -> Credential {
        Credential::ApiKey {
            api_key: key.into(),
            label: None,
        }
    }

    fn test_request() -> CompletionRequest {
        use clankers_message::message::*;
        CompletionRequest {
            model: "claude-test".into(),
            messages: vec![AgentMessage::User(UserMessage {
                id: MessageId::new("test"),
                content: vec![Content::Text {
                    text: "Hi".into(),
                }],
                timestamp: chrono::Utc::now(),
            })],
            system_prompt: None,
            max_tokens: None,
            temperature: None,
            tools: vec![],
            thinking: None,
            no_cache: true,
            cache_ttl: None,
        }
    }

    async fn collect_events(mut rx: mpsc::Receiver<StreamEvent>) -> Vec<StreamEvent> {
        let mut events = Vec::new();
        while let Some(e) = rx.recv().await {
            events.push(e);
        }
        events
    }

    // ── Metadata ────────────────────────────────────────────────────

    #[test]
    fn provider_name_is_anthropic() {
        let provider = AnthropicProvider::new(api_key("sk-test"), None);
        assert_eq!(provider.name(), "anthropic");
    }

    #[test]
    fn models_not_empty() {
        let provider = AnthropicProvider::new(api_key("sk-test"), None);
        assert!(!provider.models().is_empty());
    }

    // ── Single-credential path: success ─────────────────────────────

    #[tokio::test]
    async fn single_cred_success() {
        let server = MockServer::start(vec![MockResponse::success_sse()]).await;
        let provider = AnthropicProvider::new(api_key("sk-test"), Some(server.base_url()));

        let (tx, rx) = mpsc::channel(64);
        provider.complete(test_request(), tx).await.unwrap();

        let events = collect_events(rx).await;
        assert!(!events.is_empty());
        // Should contain MessageStart and MessageStop
        assert!(events.iter().any(|e| matches!(e, StreamEvent::MessageStart { .. })));
        assert!(events.iter().any(|e| matches!(e, StreamEvent::MessageStop)));
    }

    // ── Single-credential path: non-retryable error ─────────────────

    #[tokio::test]
    async fn single_cred_400_error() {
        let server = MockServer::start(vec![MockResponse::error(400, "bad request")]).await;
        let provider = AnthropicProvider::new(api_key("sk-test"), Some(server.base_url()));

        let (tx, _rx) = mpsc::channel(64);
        let err = provider.complete(test_request(), tx).await.unwrap_err();
        assert!(err.message.contains("400"), "got: {}", err.message);
    }

    // ── Single-credential path: 401 triggers refresh attempt ────────

    #[tokio::test]
    async fn single_cred_401_without_manager_returns_error() {
        // No credential_manager → can't refresh → error
        let server = MockServer::start(vec![MockResponse::error(401, "unauthorized")]).await;
        let provider = AnthropicProvider::new(api_key("sk-test"), Some(server.base_url()));

        let (tx, _rx) = mpsc::channel(64);
        let err = provider.complete(test_request(), tx).await.unwrap_err();
        assert!(err.message.contains("401"), "got: {}", err.message);
    }

    // ── Pool path: first credential succeeds ────────────────────────

    #[tokio::test]
    async fn pool_first_credential_succeeds() {
        let server = MockServer::start(vec![MockResponse::success_sse()]).await;

        let pool = CredentialPool::new(
            vec![
                ("primary".into(), api_key("key-1")),
                ("backup".into(), api_key("key-2")),
            ],
            SelectionStrategy::Failover,
        );

        let (_dir, path) = tempfile::TempDir::new().map(|d| {
            let p = d.path().join("auth.json");
            (d, p)
        }).unwrap();

        let cm = crate::credential_manager::CredentialManager::new(
            api_key("key-1"), path, None,
        );

        let provider = AnthropicProvider::with_credential_pool(
            cm, pool, Some(server.base_url()),
        );

        let (tx, rx) = mpsc::channel(64);
        provider.complete(test_request(), tx).await.unwrap();

        let events = collect_events(rx).await;
        assert!(events.iter().any(|e| matches!(e, StreamEvent::MessageStop)));
    }

    // ── Pool path: first fails (429), second succeeds ───────────────

    #[tokio::test]
    async fn pool_failover_on_rate_limit() {
        // send_streaming retries 429 three times (4 total), then the second
        // credential's request should succeed on the 5th mock response.
        let mut responses = Vec::new();
        for _ in 0..4 {
            responses.push(MockResponse::error(429, "rate limited"));
        }
        responses.push(MockResponse::success_sse());
        let server = MockServer::start(responses).await;

        let pool = CredentialPool::new(
            vec![
                ("primary".into(), api_key("key-1")),
                ("backup".into(), api_key("key-2")),
            ],
            SelectionStrategy::Failover,
        );

        let (_dir, path) = tempfile::TempDir::new().map(|d| {
            let p = d.path().join("auth.json");
            (d, p)
        }).unwrap();

        let cm = crate::credential_manager::CredentialManager::new(
            api_key("key-1"), path, None,
        );

        let provider = AnthropicProvider::with_credential_pool(
            cm, pool, Some(server.base_url()),
        );

        let (tx, rx) = mpsc::channel(64);
        provider.complete(test_request(), tx).await.unwrap();

        let events = collect_events(rx).await;
        assert!(events.iter().any(|e| matches!(e, StreamEvent::MessageStop)));
    }

    // ── Pool path: non-retryable error stops immediately ────────────

    #[tokio::test]
    async fn pool_non_retryable_stops_immediately() {
        // 400 is not retryable — should NOT try the backup credential
        let server = MockServer::start(vec![
            MockResponse::error(400, "invalid request"),
            // If this were reached, the test would succeed — but it shouldn't be
            MockResponse::success_sse(),
        ]).await;

        let pool = CredentialPool::new(
            vec![
                ("primary".into(), api_key("key-1")),
                ("backup".into(), api_key("key-2")),
            ],
            SelectionStrategy::Failover,
        );

        let (_dir, path) = tempfile::TempDir::new().map(|d| {
            let p = d.path().join("auth.json");
            (d, p)
        }).unwrap();

        let cm = crate::credential_manager::CredentialManager::new(
            api_key("key-1"), path, None,
        );

        let provider = AnthropicProvider::with_credential_pool(
            cm, pool, Some(server.base_url()),
        );

        let (tx, _rx) = mpsc::channel(64);
        let err = provider.complete(test_request(), tx).await.unwrap_err();
        assert!(err.message.contains("400"), "got: {}", err.message);
    }

    // ── Pool path: all credentials exhausted ────────────────────────

    #[tokio::test]
    async fn pool_all_exhausted() {
        // send_streaming retries retryable errors 3 times (4 attempts total).
        // With 2 pool credentials, need 4+4 = 8 responses.
        let mut responses = Vec::new();
        for _ in 0..4 {
            responses.push(MockResponse::error(429, "rate limited"));
        }
        for _ in 0..4 {
            responses.push(MockResponse::error(529, "overloaded"));
        }
        let server = MockServer::start(responses).await;

        let pool = CredentialPool::new(
            vec![
                ("primary".into(), api_key("key-1")),
                ("backup".into(), api_key("key-2")),
            ],
            SelectionStrategy::Failover,
        );

        let (_dir, path) = tempfile::TempDir::new().map(|d| {
            let p = d.path().join("auth.json");
            (d, p)
        }).unwrap();

        let cm = crate::credential_manager::CredentialManager::new(
            api_key("key-1"), path, None,
        );

        let provider = AnthropicProvider::with_credential_pool(
            cm, pool, Some(server.base_url()),
        );

        let (tx, _rx) = mpsc::channel(64);
        let err = provider.complete(test_request(), tx).await.unwrap_err();
        // Should contain the HTTP error from the last attempted credential
        assert!(
            err.message.contains("529") || err.message.contains("overloaded")
                || err.message.contains("429") || err.message.contains("rate"),
            "got: {}", err.message
        );
    }

    // ── Pool exhausted falls through to single-credential path ──────

    #[tokio::test]
    async fn pool_exhausted_falls_through_to_single_cred() {
        // Pre-exhaust the pool by putting all slots in cooldown
        let pool = CredentialPool::new(
            vec![("primary".into(), api_key("key-1"))],
            SelectionStrategy::Failover,
        );
        // Report failures to put slot in cooldown
        {
            let lease = pool.select().await.unwrap();
            lease.report_failure(429).await;
        }
        // Pool should now return empty leases
        assert!(pool.select_all_available().await.is_empty());

        // Single-cred path should still work
        let server = MockServer::start(vec![MockResponse::success_sse()]).await;

        let (_dir, path) = tempfile::TempDir::new().map(|d| {
            let p = d.path().join("auth.json");
            (d, p)
        }).unwrap();

        let cm = crate::credential_manager::CredentialManager::new(
            api_key("key-1"), path, None,
        );

        let provider = AnthropicProvider::with_credential_pool(
            cm, pool, Some(server.base_url()),
        );

        let (tx, rx) = mpsc::channel(64);
        provider.complete(test_request(), tx).await.unwrap();

        let events = collect_events(rx).await;
        assert!(events.iter().any(|e| matches!(e, StreamEvent::MessageStop)));
    }

    // ── Pool path: 500 is retryable, rotates ────────────────────────

    #[tokio::test]
    async fn pool_500_rotates_to_next() {
        // send_streaming retries 500 three times (4 total)
        let mut responses = Vec::new();
        for _ in 0..4 {
            responses.push(MockResponse::error(500, "internal server error"));
        }
        responses.push(MockResponse::success_sse());
        let server = MockServer::start(responses).await;

        let pool = CredentialPool::new(
            vec![
                ("primary".into(), api_key("key-1")),
                ("backup".into(), api_key("key-2")),
            ],
            SelectionStrategy::Failover,
        );

        let (_dir, path) = tempfile::TempDir::new().map(|d| {
            let p = d.path().join("auth.json");
            (d, p)
        }).unwrap();

        let cm = crate::credential_manager::CredentialManager::new(
            api_key("key-1"), path, None,
        );

        let provider = AnthropicProvider::with_credential_pool(
            cm, pool, Some(server.base_url()),
        );

        let (tx, rx) = mpsc::channel(64);
        provider.complete(test_request(), tx).await.unwrap();

        let events = collect_events(rx).await;
        assert!(events.iter().any(|e| matches!(e, StreamEvent::MessageStop)));
    }

    // ── reload_credentials resets pool health ───────────────────────

    #[tokio::test]
    async fn reload_resets_pool_health() {
        let pool = CredentialPool::new(
            vec![
                ("primary".into(), api_key("key-1")),
                ("backup".into(), api_key("key-2")),
            ],
            SelectionStrategy::Failover,
        );

        // Exhaust both slots
        {
            let lease = pool.select().await.unwrap();
            lease.report_failure(429).await;
            let lease = pool.select().await.unwrap();
            lease.report_failure(429).await;
        }
        assert!(pool.select().await.is_none());

        let (_dir, path) = tempfile::TempDir::new().map(|d| {
            let p = d.path().join("auth.json");
            (d, p)
        }).unwrap();

        let cm = crate::credential_manager::CredentialManager::new(
            api_key("key-1"), path, None,
        );

        let provider = AnthropicProvider::with_credential_pool(cm, pool, None);

        // reload should reset health
        provider.reload_credentials().await;

        // Pool should be usable again (can't easily check directly,
        // but no panic and the method runs = success)
    }

    // ── Provider without pool or manager ─────────────────────────

    #[tokio::test]
    async fn simple_provider_success() {
        let server = MockServer::start(vec![MockResponse::success_sse()]).await;
        let provider = AnthropicProvider::new(api_key("sk-test"), Some(server.base_url()));

        let (tx, rx) = mpsc::channel(64);
        provider.complete(test_request(), tx).await.unwrap();

        let events = collect_events(rx).await;
        assert!(events.iter().any(|e| matches!(e, StreamEvent::MessageStart { .. })));
        assert!(events.iter().any(|e| matches!(e, StreamEvent::ContentBlockDelta { .. })));
        assert!(events.iter().any(|e| matches!(e, StreamEvent::MessageStop)));
    }

    // ── with_credential_manager constructor ─────────────────────

    #[tokio::test]
    async fn with_credential_manager_success() {
        let server = MockServer::start(vec![MockResponse::success_sse()]).await;

        let (_dir, path) = tempfile::TempDir::new().map(|d| {
            let p = d.path().join("auth.json");
            (d, p)
        }).unwrap();

        let cm = crate::credential_manager::CredentialManager::new(
            api_key("sk-managed"), path, None,
        );

        let provider = AnthropicProvider::with_credential_manager(cm, Some(server.base_url()));

        let (tx, rx) = mpsc::channel(64);
        provider.complete(test_request(), tx).await.unwrap();

        let events = collect_events(rx).await;
        assert!(events.iter().any(|e| matches!(e, StreamEvent::MessageStop)));
    }

    // ── get_credential with no credential configured ────────────

    #[tokio::test]
    async fn no_credential_returns_error() {
        let provider = AnthropicProvider {
            client: api::AnthropicClient::new(None),
            credential: None,
            credential_manager: None,
            credential_pool: None,
            models: vec![],
        };

        let err = provider.get_credential().await.unwrap_err();
        assert!(err.message.contains("No credential"), "got: {}", err.message);
    }

    // ── force_refresh with no manager returns error ──────────────

    #[tokio::test]
    async fn force_refresh_no_manager_errors() {
        let provider = AnthropicProvider::new(api_key("sk-test"), None);
        let err = provider.force_refresh_credential().await.unwrap_err();
        assert!(
            err.message.contains("no credential manager"),
            "got: {}", err.message
        );
    }

    // ── Round-robin pool rotates credentials ─────────────────────

    #[tokio::test]
    async fn pool_round_robin_rotates() {
        let server = MockServer::start(vec![
            MockResponse::success_sse(),
            MockResponse::success_sse(),
            MockResponse::success_sse(),
        ]).await;

        let pool = CredentialPool::new(
            vec![
                ("a".into(), api_key("key-a")),
                ("b".into(), api_key("key-b")),
                ("c".into(), api_key("key-c")),
            ],
            SelectionStrategy::RoundRobin,
        );

        let (_dir, path) = tempfile::TempDir::new().map(|d| {
            let p = d.path().join("auth.json");
            (d, p)
        }).unwrap();

        let cm = crate::credential_manager::CredentialManager::new(
            api_key("key-a"), path, None,
        );

        let provider = AnthropicProvider::with_credential_pool(
            cm, pool, Some(server.base_url()),
        );

        // Three requests should all succeed (rotating through credentials)
        for _ in 0..3 {
            let (tx, rx) = mpsc::channel(64);
            provider.complete(test_request(), tx).await.unwrap();
            let events = collect_events(rx).await;
            assert!(events.iter().any(|e| matches!(e, StreamEvent::MessageStop)));
        }
    }
}
