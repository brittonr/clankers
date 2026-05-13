use std::sync::Arc;
#[cfg(test)]
use std::sync::Mutex;
#[cfg(test)]
use std::sync::OnceLock;

use async_trait::async_trait;
use tokio::sync::mpsc;

use super::common;
use crate::auth::AuthStore;
#[cfg(test)]
use crate::auth::StoredCredential;
use crate::credential::CredentialManager;
#[cfg(test)]
use crate::credential::OAuthTokens;
use crate::error::Error;
use crate::error::Result;
use crate::model::Model;
use crate::provider::CompletionRequest;
use crate::provider::Provider;
use crate::streaming::StreamEvent;

pub const OPENAI_CODEX_PROVIDER: &str = "openai-codex";
pub const OPENAI_CODEX_RESPONSES_URL: &str = "https://chatgpt.com/backend-api/codex/responses";
const OPENAI_CODEX_BETA_HEADER: &str = "responses=experimental";
const OPENAI_CODEX_NOT_ENTITLED_CODE: &str = "usage_not_included";

pub const OPENAI_CODEX_MODEL_IDS: [&str; 2] = ["gpt-5.3-codex", "gpt-5.3-codex-spark"];
const OPENAI_CODEX_PROBE_MODEL: &str = "gpt-5.3-codex";

mod attempt;
mod entitlement;
mod request;
mod streaming;

pub(crate) use attempt::OpenAICodexAttempt;
pub use entitlement::EntitlementRecord;
pub use entitlement::EntitlementState;
#[cfg(test)]
pub(crate) use entitlement::ProbeOutcome;
#[cfg(test)]
pub(crate) use entitlement::SleepHook;
#[cfg(test)]
pub(crate) use entitlement::build_probe_request;
#[cfg(test)]
pub(crate) use entitlement::build_probe_request_body;
pub use entitlement::catalog_for_active_account;
pub use entitlement::catalog_for_active_account_with_manager;
#[cfg(test)]
pub(crate) use entitlement::classify_probe_response;
pub use entitlement::codex_models;
pub(crate) use entitlement::codex_sleep;
pub use entitlement::codex_status_suffix;
pub use entitlement::codex_status_suffix_with_manager;
pub use entitlement::ensure_entitlement;
pub use entitlement::entitlement_record;
#[cfg(test)]
pub(crate) use entitlement::live_probe;
#[cfg(test)]
pub(crate) use entitlement::now_ms;
#[cfg(test)]
pub(crate) use entitlement::probe_hook;
pub use entitlement::refresh_fn_for_codex;
pub use entitlement::reset_entitlement;
pub(crate) use entitlement::responses_url;
#[cfg(test)]
pub(crate) use entitlement::responses_url_override;
#[cfg(test)]
pub(crate) use entitlement::sleep_hook;
#[cfg(test)]
pub(crate) use request::build_codex_input;
pub(crate) use request::build_codex_request;
#[cfg(test)]
pub(crate) use request::build_codex_request_body;
pub(crate) use request::map_codex_error;

pub struct OpenAICodexProvider {
    credential_manager: Arc<CredentialManager>,
    models: Vec<Model>,
    account: String,
}

impl OpenAICodexProvider {
    pub fn new(credential_manager: Arc<CredentialManager>, models: Vec<Model>, account: String) -> Arc<dyn Provider> {
        Arc::new(Self {
            credential_manager,
            models,
            account,
        })
    }
}

#[async_trait]
impl Provider for OpenAICodexProvider {
    async fn complete(&self, request: CompletionRequest, tx: mpsc::Sender<StreamEvent>) -> Result<()> {
        let credential = self.credential_manager.get_credential().await?;
        let mut record = entitlement_record(&self.account);
        if matches!(record.state, EntitlementState::Unknown) {
            let mut store = AuthStore::default();
            store.set_credential(OPENAI_CODEX_PROVIDER, &self.account, credential.clone());
            record = ensure_entitlement(&store, &self.account, Some(self.credential_manager.as_ref())).await;
        }

        match &record.state {
            EntitlementState::Entitled { .. } => {}
            EntitlementState::NotEntitled { reason, .. } => {
                return Err(Error::Auth {
                    message: format!("{reason}. ChatGPT Plus or Pro is required for openai-codex"),
                });
            }
            EntitlementState::Unknown => {
                if let Some(error) = record.last_error {
                    return Err(Error::provider_with_status(
                        503,
                        format!("openai-codex entitlement check failed: {error}"),
                    ));
                }
                return Err(Error::provider_with_status(503, "openai-codex entitlement check failed".to_string()));
            }
        }

        let mut attempt = OpenAICodexAttempt::new(request, tx, credential, Arc::clone(&self.credential_manager));
        attempt.run().await
    }

    fn models(&self) -> &[Model] {
        &self.models
    }

    fn name(&self) -> &str {
        OPENAI_CODEX_PROVIDER
    }

    async fn reload_credentials(&self) {
        reset_entitlement(OPENAI_CODEX_PROVIDER, None);
        self.credential_manager.reload_from_disk().await;
    }

    async fn is_available(&self) -> bool {
        let credential = self.credential_manager.get_credential().await;
        credential.is_ok()
    }
}

#[cfg(test)]
pub(crate) use streaming::CodexStreamState;
pub(crate) use streaming::parse_codex_sse;

#[cfg(test)]
fn codex_test_lock() -> &'static Mutex<()> {
    static TEST_LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    TEST_LOCK.get_or_init(|| Mutex::new(()))
}

#[cfg(test)]
pub(crate) async fn with_test_probe_hook_async<F, Fut, R>(hook: F, f: impl FnOnce() -> Fut) -> R
where
    F: Fn(&StoredCredential) -> ProbeOutcome + Send + Sync + 'static,
    Fut: std::future::Future<Output = R>,
{
    let _guard = codex_test_lock().lock().unwrap_or_else(|poison| poison.into_inner());

    reset_entitlement(OPENAI_CODEX_PROVIDER, None);
    *probe_hook().lock().expect("probe hook lock poisoned") = Some(Arc::new(hook));
    let result = f().await;
    *probe_hook().lock().expect("probe hook lock poisoned") = None;
    reset_entitlement(OPENAI_CODEX_PROVIDER, None);
    result
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::collections::HashMap;
    use std::collections::HashSet;
    use std::future::Future;
    use std::sync::Arc;
    use std::sync::atomic::AtomicUsize;
    use std::sync::atomic::Ordering;
    use std::time::Duration;

    use base64::Engine;
    use serde_json::Value;
    use serde_json::json;
    use tokio::io::AsyncReadExt;
    use tokio::io::AsyncWriteExt;
    use tokio::net::TcpListener;

    use super::*;
    use crate::auth::AuthStorePaths;
    use crate::streaming::ContentBlock;
    use crate::streaming::ContentDelta;

    fn fake_openai_codex_jwt(account_id: &str) -> String {
        let header = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(r#"{"alg":"none","typ":"JWT"}"#);
        let payload = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(
            serde_json::json!({
                "https://api.openai.com/auth": {
                    "chatgpt_account_id": account_id,
                }
            })
            .to_string()
            .as_bytes(),
        );
        format!("{header}.{payload}.sig")
    }

    fn codex_store() -> AuthStore {
        let mut store = AuthStore::default();
        store.set_credential(OPENAI_CODEX_PROVIDER, "work", StoredCredential::OAuth {
            access_token: fake_openai_codex_jwt("acct-123"),
            refresh_token: "refresh".to_string(),
            expires_at_ms: now_ms() + 3_600_000,
            label: None,
        });
        store.switch_account(OPENAI_CODEX_PROVIDER, "work");
        store
    }

    fn codex_request(session_id: Option<&str>) -> CompletionRequest {
        let mut extra_params = HashMap::new();
        if let Some(session_id) = session_id {
            extra_params.insert("_session_id".to_string(), json!(session_id));
        }

        CompletionRequest {
            model: format!("{OPENAI_CODEX_PROVIDER}/{}", OPENAI_CODEX_MODEL_IDS[0]),
            messages: vec![json!({"role": "user", "content": [{"type": "text", "text": "hello"}]})],
            system_prompt: Some("system".to_string()),
            max_tokens: Some(128),
            temperature: Some(0.2),
            tools: vec![crate::provider::ToolDefinition {
                name: "read".to_string(),
                description: "Read a file".to_string(),
                input_schema: json!({"type": "object"}),
            }],
            thinking: Some(crate::provider::ThinkingConfig {
                enabled: true,
                budget_tokens: Some(512),
            }),
            no_cache: false,
            cache_ttl: None,
            extra_params,
        }
    }

    fn codex_reasoning_signature_fixture() -> String {
        json!({
            "type": "reasoning",
            "id": "rs_123",
            "summary": [{"type": "summary_text", "text": "hidden reasoning"}],
        })
        .to_string()
    }

    fn codex_request_with_history(session_id: Option<&str>) -> CompletionRequest {
        let mut request = codex_request(session_id);
        request.messages = vec![
            json!({
                "role": "user",
                "content": [{"type": "text", "text": "previous user"}],
            }),
            json!({
                "role": "assistant",
                "content": [
                    {
                        "type": "thinking",
                        "thinking": "display-only reasoning",
                        "signature": codex_reasoning_signature_fixture(),
                    },
                    {"type": "text", "text": "assistant text"},
                    {"type": "refusal", "text": "assistant refusal"},
                    {
                        "type": "tool_use",
                        "id": "call-1|item-1",
                        "name": "read",
                        "input": {"path": "Cargo.toml"},
                    },
                ],
            }),
            json!({
                "role": "user",
                "content": [{
                    "type": "tool_result",
                    "tool_use_id": "call-1|item-1",
                    "output": "file body",
                }],
            }),
            json!({
                "role": "user",
                "content": [{"type": "text", "text": "current user"}],
            }),
        ];
        request
    }

    fn codex_request_body_fixture(session_id: Option<&str>) -> Value {
        let mut body = json!({
            "model": OPENAI_CODEX_MODEL_IDS[0],
            "store": false,
            "stream": true,
            "instructions": "system",
            "input": [
                {
                    "type": "message",
                    "role": "user",
                    "content": [{"type": "input_text", "text": "previous user"}],
                },
                {
                    "type": "reasoning",
                    "id": "rs_123",
                    "summary": [{"type": "summary_text", "text": "hidden reasoning"}],
                },
                {
                    "type": "message",
                    "role": "assistant",
                    "content": [
                        {"type": "output_text", "text": "assistant text", "annotations": []},
                        {"type": "refusal", "refusal": "assistant refusal"},
                    ],
                    "status": "completed",
                },
                {
                    "type": "function_call",
                    "call_id": "call-1",
                    "id": "item-1",
                    "name": "read",
                    "arguments": "{\"path\":\"Cargo.toml\"}",
                },
                {
                    "type": "function_call_output",
                    "call_id": "call-1",
                    "output": "file body",
                },
                {
                    "type": "message",
                    "role": "user",
                    "content": [{"type": "input_text", "text": "current user"}],
                },
            ],
            "text": {"verbosity": "medium"},
            "include": ["reasoning.encrypted_content"],
            "tool_choice": "auto",
            "parallel_tool_calls": true,
            "tools": [{
                "type": "function",
                "name": "read",
                "description": "Read a file",
                "parameters": {"type": "object"},
                "strict": null,
            }],
            "temperature": 0.2,
            "reasoning": {"effort": "medium", "summary": "auto"},
        });

        if let Some(session_id) = session_id {
            body["prompt_cache_key"] = json!(session_id);
        }

        body
    }

    fn oauth_credential(account_id: &str) -> StoredCredential {
        StoredCredential::OAuth {
            access_token: fake_openai_codex_jwt(account_id),
            refresh_token: "refresh".to_string(),
            expires_at_ms: now_ms() + 3_600_000,
            label: None,
        }
    }

    fn test_provider(credential: StoredCredential) -> Arc<dyn Provider> {
        let dir = tempfile::TempDir::new().expect("tempdir should exist");
        let auth_paths = AuthStorePaths::single(dir.path().join("auth.json"));
        let manager = CredentialManager::new(OPENAI_CODEX_PROVIDER.to_string(), credential, auth_paths, None);
        OpenAICodexProvider::new(manager, codex_models(), "work".to_string())
    }

    fn header_subset(request: &reqwest::Request, names: &[&str]) -> BTreeMap<String, String> {
        names
            .iter()
            .filter_map(|name| {
                request
                    .headers()
                    .get(*name)
                    .and_then(|value| value.to_str().ok())
                    .map(|value| ((*name).to_string(), value.to_string()))
            })
            .collect()
    }

    fn request_body_json(request: &reqwest::Request) -> Value {
        serde_json::from_slice(request.body().and_then(|body| body.as_bytes()).expect("body bytes")).expect("json body")
    }

    fn collect_stream_events(raw_events: &[Value]) -> Result<Vec<StreamEvent>> {
        let mut state = CodexStreamState::new("gpt-5.1-codex".to_string());
        let mut out = Vec::new();
        for event in raw_events {
            out.extend(state.handle_event(event)?);
        }
        Ok(out)
    }

    fn codex_stream_fixture_events() -> Vec<Value> {
        vec![
            json!({
                "type": "response.output_item.added",
                "item": {"type": "reasoning", "id": "rs_123"},
            }),
            json!({
                "type": "response.reasoning_summary_part.added",
                "item_id": "rs_123",
            }),
            json!({
                "type": "response.reasoning_summary_text.delta",
                "item_id": "rs_123",
                "delta": "plan",
            }),
            json!({
                "type": "response.reasoning_summary_part.done",
                "item_id": "rs_123",
            }),
            json!({
                "type": "response.reasoning_summary_part.added",
                "item_id": "rs_123",
            }),
            json!({
                "type": "response.reasoning_summary_text.delta",
                "item_id": "rs_123",
                "delta": "next",
            }),
            json!({
                "type": "response.output_item.done",
                "item": {
                    "type": "reasoning",
                    "id": "rs_123",
                    "summary": [{"text": "fallback summary"}],
                },
            }),
            json!({
                "type": "response.output_item.added",
                "item": {"type": "message", "id": "msg_1"},
            }),
            json!({
                "type": "response.content_part.added",
                "item_id": "msg_1",
            }),
            json!({
                "type": "response.output_text.delta",
                "item_id": "msg_1",
                "delta": "hello ",
            }),
            json!({
                "type": "response.refusal.delta",
                "item_id": "msg_1",
                "delta": "no",
            }),
            json!({
                "type": "response.output_item.done",
                "item": {
                    "type": "message",
                    "id": "msg_1",
                    "content": [{"type": "output_text", "text": "fallback text"}],
                },
            }),
            json!({
                "type": "response.output_item.added",
                "item": {
                    "type": "function_call",
                    "id": "fc_item_1",
                    "call_id": "call-1",
                    "name": "read",
                    "arguments": "{",
                },
            }),
            json!({
                "type": "response.function_call_arguments.delta",
                "item_id": "fc_item_1",
                "delta": "\"path\"",
            }),
            json!({
                "type": "response.function_call_arguments.done",
                "item_id": "fc_item_1",
                "arguments": "{\"path\":\"Cargo.toml\"}",
            }),
            json!({
                "type": "response.output_item.done",
                "item": {
                    "type": "function_call",
                    "id": "fc_item_1",
                    "call_id": "call-1",
                    "name": "read",
                    "arguments": "{\"path\":\"Cargo.toml\"}",
                },
            }),
            json!({
                "type": "response.completed",
                "response": {
                    "status": "completed",
                    "usage": {
                        "input_tokens": 10,
                        "output_tokens": 5,
                        "input_tokens_details": {"cached_tokens": 3},
                    },
                },
            }),
        ]
    }

    fn assert_codex_stream_fixture(events: &[StreamEvent]) {
        assert_eq!(events.len(), 18, "events: {events:#?}");

        match &events[0] {
            StreamEvent::MessageStart { message } => {
                assert_eq!(message.id, "rs_123");
                assert_eq!(message.model, "gpt-5.1-codex");
                assert_eq!(message.role, "assistant");
            }
            other => panic!("expected MessageStart, got {other:?}"),
        }

        match &events[1] {
            StreamEvent::ContentBlockStart { index, content_block } => {
                assert_eq!(*index, 0);
                match content_block {
                    ContentBlock::Thinking { thinking, signature } => {
                        assert!(thinking.is_empty());
                        assert!(signature.is_empty());
                    }
                    other => panic!("expected Thinking block, got {other:?}"),
                }
            }
            other => panic!("expected thinking block start, got {other:?}"),
        }

        match &events[2] {
            StreamEvent::ContentBlockDelta { index, delta } => {
                assert_eq!(*index, 0);
                match delta {
                    ContentDelta::ThinkingDelta { thinking } => assert_eq!(thinking, "plan"),
                    other => panic!("expected ThinkingDelta, got {other:?}"),
                }
            }
            other => panic!("expected thinking delta, got {other:?}"),
        }

        match &events[3] {
            StreamEvent::ContentBlockDelta { index, delta } => {
                assert_eq!(*index, 0);
                match delta {
                    ContentDelta::ThinkingDelta { thinking } => assert_eq!(thinking, "\n\n"),
                    other => panic!("expected ThinkingDelta separator, got {other:?}"),
                }
            }
            other => panic!("expected thinking separator delta, got {other:?}"),
        }

        match &events[4] {
            StreamEvent::ContentBlockDelta { index, delta } => {
                assert_eq!(*index, 0);
                match delta {
                    ContentDelta::ThinkingDelta { thinking } => assert_eq!(thinking, "next"),
                    other => panic!("expected ThinkingDelta, got {other:?}"),
                }
            }
            other => panic!("expected thinking delta, got {other:?}"),
        }

        let expected_signature = serde_json::to_string(&json!({
            "type": "reasoning",
            "id": "rs_123",
            "summary": [{"text": "fallback summary"}],
        }))
        .expect("signature json");
        match &events[5] {
            StreamEvent::ContentBlockDelta { index, delta } => {
                assert_eq!(*index, 0);
                match delta {
                    ContentDelta::SignatureDelta { signature } => {
                        assert_eq!(signature, &expected_signature)
                    }
                    other => panic!("expected SignatureDelta, got {other:?}"),
                }
            }
            other => panic!("expected signature delta, got {other:?}"),
        }

        match &events[6] {
            StreamEvent::ContentBlockStop { index } => assert_eq!(*index, 0),
            other => panic!("expected thinking stop, got {other:?}"),
        }

        match &events[7] {
            StreamEvent::ContentBlockStart { index, content_block } => {
                assert_eq!(*index, 1);
                match content_block {
                    ContentBlock::Text { text } => assert!(text.is_empty()),
                    other => panic!("expected Text block, got {other:?}"),
                }
            }
            other => panic!("expected text block start, got {other:?}"),
        }

        match &events[8] {
            StreamEvent::ContentBlockDelta { index, delta } => {
                assert_eq!(*index, 1);
                match delta {
                    ContentDelta::TextDelta { text } => assert_eq!(text, "hello "),
                    other => panic!("expected TextDelta, got {other:?}"),
                }
            }
            other => panic!("expected text delta, got {other:?}"),
        }

        match &events[9] {
            StreamEvent::ContentBlockDelta { index, delta } => {
                assert_eq!(*index, 1);
                match delta {
                    ContentDelta::TextDelta { text } => assert_eq!(text, "no"),
                    other => panic!("expected TextDelta, got {other:?}"),
                }
            }
            other => panic!("expected refusal delta mapped as text delta, got {other:?}"),
        }

        match &events[10] {
            StreamEvent::ContentBlockStop { index } => assert_eq!(*index, 1),
            other => panic!("expected text stop, got {other:?}"),
        }

        match &events[11] {
            StreamEvent::ContentBlockStart { index, content_block } => {
                assert_eq!(*index, 2);
                match content_block {
                    ContentBlock::ToolUse { id, name, input } => {
                        assert_eq!(id, "call-1|fc_item_1");
                        assert_eq!(name, "read");
                        assert_eq!(input, &json!({}));
                    }
                    other => panic!("expected ToolUse block, got {other:?}"),
                }
            }
            other => panic!("expected tool block start, got {other:?}"),
        }

        match &events[12] {
            StreamEvent::ContentBlockDelta { index, delta } => {
                assert_eq!(*index, 2);
                match delta {
                    ContentDelta::InputJsonDelta { partial_json } => assert_eq!(partial_json, "{"),
                    other => panic!("expected InputJsonDelta, got {other:?}"),
                }
            }
            other => panic!("expected tool input delta, got {other:?}"),
        }

        match &events[13] {
            StreamEvent::ContentBlockDelta { index, delta } => {
                assert_eq!(*index, 2);
                match delta {
                    ContentDelta::InputJsonDelta { partial_json } => {
                        assert_eq!(partial_json, "\"path\"")
                    }
                    other => panic!("expected InputJsonDelta, got {other:?}"),
                }
            }
            other => panic!("expected tool input delta, got {other:?}"),
        }

        match &events[14] {
            StreamEvent::ContentBlockDelta { index, delta } => {
                assert_eq!(*index, 2);
                match delta {
                    ContentDelta::InputJsonDelta { partial_json } => {
                        assert_eq!(partial_json, ":\"Cargo.toml\"}")
                    }
                    other => panic!("expected final InputJsonDelta suffix, got {other:?}"),
                }
            }
            other => panic!("expected tool suffix delta, got {other:?}"),
        }

        match &events[15] {
            StreamEvent::ContentBlockStop { index } => assert_eq!(*index, 2),
            other => panic!("expected tool stop, got {other:?}"),
        }

        match &events[16] {
            StreamEvent::MessageDelta { stop_reason, usage } => {
                assert_eq!(stop_reason.as_deref(), Some("tool_use"));
                assert_eq!(usage.input_tokens, 7);
                assert_eq!(usage.output_tokens, 5);
                assert_eq!(usage.cache_read_input_tokens, 3);
            }
            other => panic!("expected message delta, got {other:?}"),
        }

        match &events[17] {
            StreamEvent::MessageStop => {}
            other => panic!("expected message stop, got {other:?}"),
        }
    }

    struct MockSseServer {
        url: String,
        _handle: tokio::task::JoinHandle<()>,
    }

    impl MockSseServer {
        async fn start(events: &[Value]) -> Self {
            let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind test listener");
            let addr = listener.local_addr().expect("listener addr");
            let body = events
                .iter()
                .map(|event| format!("data: {}\n\n", serde_json::to_string(event).expect("event json")))
                .chain(std::iter::once("data: [DONE]\n\n".to_string()))
                .collect::<String>();

            let handle = tokio::spawn(async move {
                let (mut stream, _) = listener.accept().await.expect("accept connection");
                let mut buf = Vec::new();
                let mut chunk = [0u8; 1024];
                while !buf.windows(4).any(|window| window == b"\r\n\r\n") {
                    let read = stream.read(&mut chunk).await.expect("read request");
                    assert!(read > 0, "request closed before headers");
                    buf.extend_from_slice(&chunk[..read]);
                }

                let response = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                    body.len(),
                    body,
                );
                stream.write_all(response.as_bytes()).await.expect("write response");
                stream.flush().await.expect("flush response");
            });

            Self {
                url: format!("http://{addr}/stream"),
                _handle: handle,
            }
        }
    }

    async fn collect_runtime_stream_events(events: &[Value]) -> Result<Vec<StreamEvent>> {
        let server = MockSseServer::start(events).await;
        let response = reqwest::get(&server.url).await?;
        let (tx, mut rx) = mpsc::channel(64);
        parse_codex_sse(response, "gpt-5.1-codex", tx).await?;
        let mut out = Vec::new();
        while let Some(event) = rx.recv().await {
            out.push(event);
        }
        Ok(out)
    }

    struct HttpHookGuard;

    impl Drop for HttpHookGuard {
        fn drop(&mut self) {
            *responses_url_override().lock().expect("responses url override lock poisoned") = None;
            *sleep_hook().lock().expect("sleep hook lock poisoned") = None;
            reset_entitlement(OPENAI_CODEX_PROVIDER, None);
        }
    }

    async fn with_test_http_hooks<F, Fut, R>(url: String, sleep_log: Option<Arc<Mutex<Vec<Duration>>>>, f: F) -> R
    where
        F: FnOnce() -> Fut,
        Fut: Future<Output = R>,
    {
        let _guard = codex_test_lock().lock().unwrap_or_else(|poison| poison.into_inner());
        let _cleanup = HttpHookGuard;

        reset_entitlement(OPENAI_CODEX_PROVIDER, None);
        *responses_url_override().lock().expect("responses url override lock poisoned") = Some(url);
        *sleep_hook().lock().expect("sleep hook lock poisoned") = sleep_log.map(|log| {
            Arc::new(move |duration| {
                log.lock().expect("sleep log lock poisoned").push(duration);
            }) as SleepHook
        });

        f().await
    }

    #[derive(Clone)]
    struct MockHttpResponse {
        status: u16,
        content_type: &'static str,
        body: String,
    }

    #[derive(Debug, Clone)]
    struct CapturedHttpRequest {
        headers: HashMap<String, String>,
        body: String,
    }

    struct MockHttpSequenceServer {
        url: String,
        request_count: Arc<AtomicUsize>,
        requests: Arc<Mutex<Vec<CapturedHttpRequest>>>,
        _handle: tokio::task::JoinHandle<()>,
    }

    impl MockHttpSequenceServer {
        async fn start(responses: Vec<MockHttpResponse>) -> Self {
            let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind test listener");
            let addr = listener.local_addr().expect("listener addr");
            let request_count = Arc::new(AtomicUsize::new(0));
            let requests = Arc::new(Mutex::new(Vec::new()));
            let request_count_clone = Arc::clone(&request_count);
            let requests_clone = Arc::clone(&requests);

            let handle = tokio::spawn(async move {
                for response in responses {
                    let (mut stream, _) = listener.accept().await.expect("accept connection");
                    let mut buf = Vec::new();
                    let mut chunk = [0u8; 1024];
                    while !buf.windows(4).any(|window| window == b"\r\n\r\n") {
                        let read = stream.read(&mut chunk).await.expect("read request");
                        assert!(read > 0, "request closed before headers");
                        buf.extend_from_slice(&chunk[..read]);
                    }

                    let headers_end = buf.windows(4).position(|window| window == b"\r\n\r\n").expect("headers end");
                    let header_text = std::str::from_utf8(&buf[..headers_end]).expect("request headers should be utf8");
                    let headers = header_text
                        .lines()
                        .skip(1)
                        .filter_map(|line| {
                            line.split_once(':')
                                .map(|(name, value)| (name.trim().to_ascii_lowercase(), value.trim().to_string()))
                        })
                        .collect::<HashMap<_, _>>();
                    let content_length =
                        headers.get("content-length").and_then(|value| value.parse::<usize>().ok()).unwrap_or(0);
                    let body_start = headers_end + 4;
                    while buf.len() < body_start + content_length {
                        let read = stream.read(&mut chunk).await.expect("read request body");
                        assert!(read > 0, "request closed before body");
                        buf.extend_from_slice(&chunk[..read]);
                    }
                    let body = String::from_utf8(buf[body_start..body_start + content_length].to_vec())
                        .expect("request body should be utf8");
                    requests_clone.lock().expect("requests lock poisoned").push(CapturedHttpRequest { headers, body });
                    request_count_clone.fetch_add(1, Ordering::SeqCst);

                    let response_text = format!(
                        "HTTP/1.1 {} TEST\r\nContent-Type: {}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                        response.status,
                        response.content_type,
                        response.body.len(),
                        response.body,
                    );
                    stream.write_all(response_text.as_bytes()).await.expect("write response");
                    stream.flush().await.expect("flush response");
                }
            });

            Self {
                url: format!("http://{addr}/responses"),
                request_count,
                requests,
                _handle: handle,
            }
        }
    }

    fn probe_json_response(status: u16, body: Value) -> MockHttpResponse {
        MockHttpResponse {
            status,
            content_type: "application/json",
            body: serde_json::to_string(&body).expect("json body"),
        }
    }

    fn codex_sse_ok_response() -> MockHttpResponse {
        MockHttpResponse {
            status: 200,
            content_type: "text/event-stream",
            body: "data: [DONE]\n\n".to_string(),
        }
    }

    fn test_request_manager(credential: StoredCredential, refresh_calls: Arc<AtomicUsize>) -> Arc<CredentialManager> {
        let auth_path =
            std::env::temp_dir().join(format!("clanker-router-codex-refresh-{}-{}.json", std::process::id(), now_ms()));
        let auth_paths = AuthStorePaths::single(auth_path);
        CredentialManager::with_refresh_fn(OPENAI_CODEX_PROVIDER.to_string(), credential, auth_paths, None, move |_| {
            let refresh_calls = Arc::clone(&refresh_calls);
            Box::pin(async move {
                refresh_calls.fetch_add(1, Ordering::SeqCst);
                Ok(OAuthTokens {
                    access_token: fake_openai_codex_jwt("acct-refreshed"),
                    refresh_token: "refresh-2".to_string(),
                    expires_at_ms: now_ms() + 3_600_000,
                })
            })
        })
    }

    #[test]
    fn codex_stream_state_normalizes_reasoning_text_and_tool_events() {
        let events = collect_stream_events(&codex_stream_fixture_events()).expect("stream events should normalize");
        assert_codex_stream_fixture(&events);
    }

    #[tokio::test]
    async fn parse_codex_sse_runtime_seam_normalizes_raw_events() {
        let events = collect_runtime_stream_events(&codex_stream_fixture_events())
            .await
            .expect("runtime seam should normalize events");
        assert_codex_stream_fixture(&events);
    }

    #[test]
    fn codex_reasoning_summary_part_events_do_not_create_extra_block_boundaries() {
        let events = collect_stream_events(&codex_stream_fixture_events()).expect("stream events should normalize");
        let thinking_starts = events
            .iter()
            .filter(|event| {
                matches!(event, StreamEvent::ContentBlockStart {
                    content_block: ContentBlock::Thinking { .. },
                    ..
                })
            })
            .count();
        let thinking_stops =
            events.iter().filter(|event| matches!(event, StreamEvent::ContentBlockStop { index: 0 })).count();

        assert_eq!(thinking_starts, 1, "events: {events:#?}");
        assert_eq!(thinking_stops, 1, "events: {events:#?}");
    }

    #[test]
    fn codex_completed_incomplete_maps_to_max_tokens_stop() {
        let events = collect_stream_events(&[
            json!({
                "type": "response.output_item.added",
                "item": {"type": "message", "id": "msg_incomplete"},
            }),
            json!({
                "type": "response.output_text.delta",
                "item_id": "msg_incomplete",
                "delta": "partial",
            }),
            json!({
                "type": "response.output_item.done",
                "item": {
                    "type": "message",
                    "id": "msg_incomplete",
                    "content": [{"type": "output_text", "text": "partial"}],
                },
            }),
            json!({
                "type": "response.completed",
                "response": {
                    "status": "incomplete",
                    "usage": {"input_tokens": 4, "output_tokens": 2},
                },
            }),
        ])
        .expect("incomplete stream should normalize");

        match events.last().expect("message stop should exist") {
            StreamEvent::MessageStop => {}
            other => panic!("expected MessageStop, got {other:?}"),
        }
        match &events[events.len() - 2] {
            StreamEvent::MessageDelta { stop_reason, usage } => {
                assert_eq!(stop_reason.as_deref(), Some("max_tokens"));
                assert_eq!(usage.input_tokens, 4);
                assert_eq!(usage.output_tokens, 2);
            }
            other => panic!("expected MessageDelta, got {other:?}"),
        }
    }

    #[test]
    fn codex_completed_failed_and_cancelled_return_provider_errors() {
        for status in ["failed", "cancelled"] {
            let err = collect_stream_events(&[json!({
                "type": "response.completed",
                "response": {
                    "status": status,
                    "error": {"message": format!("{status} boom")},
                },
            })])
            .expect_err("failed/cancelled responses should error");
            assert_eq!(err.to_string(), format!("provider error: {status} boom"));
            assert_eq!(err.status_code(), Some(500));
        }
    }

    #[test]
    fn codex_completed_queued_and_in_progress_suppress_final_stop_reason() {
        for status in ["queued", "in_progress"] {
            let events = collect_stream_events(&[json!({
                "type": "response.completed",
                "response": {
                    "status": status,
                    "usage": {"input_tokens": 1, "output_tokens": 0},
                },
            })])
            .expect("queued/in_progress should not error");
            assert_eq!(events.len(), 2, "status {status}, events {events:#?}");
            match &events[0] {
                StreamEvent::MessageDelta { stop_reason, usage } => {
                    assert_eq!(stop_reason, &None, "status {status}");
                    assert_eq!(usage.input_tokens, 1);
                    assert_eq!(usage.output_tokens, 0);
                }
                other => panic!("expected MessageDelta for {status}, got {other:?}"),
            }
            match &events[1] {
                StreamEvent::MessageStop => {}
                other => panic!("expected MessageStop for {status}, got {other:?}"),
            }
        }
    }

    #[test]
    fn codex_catalog_is_exact_fixed_set() {
        let ids: Vec<&str> = OPENAI_CODEX_MODEL_IDS.to_vec();
        let unique: HashSet<&str> = ids.iter().copied().collect();
        assert_eq!(ids.len(), 2);
        assert_eq!(unique.len(), 2);
        assert_eq!(ids, vec!["gpt-5.3-codex", "gpt-5.3-codex-spark"]);
    }

    #[tokio::test]
    async fn codex_status_suffix_reports_not_entitled() {
        with_test_probe_hook_async(
            |_| ProbeOutcome::NotEntitled("authenticated but not entitled for Codex use".to_string()),
            || async {
                let store = codex_store();
                let suffix = codex_status_suffix(&store, "work").await.expect("suffix should exist");
                assert_eq!(suffix, "authenticated but not entitled for Codex use");
            },
        )
        .await;
    }

    #[tokio::test]
    async fn codex_status_suffix_reports_probe_failure() {
        with_test_probe_hook_async(
            |_| ProbeOutcome::Error("boom".to_string()),
            || async {
                let store = codex_store();
                let suffix = codex_status_suffix(&store, "work").await.expect("suffix should exist");
                assert_eq!(suffix, "authenticated, entitlement check failed");
            },
        )
        .await;
    }

    #[tokio::test]
    async fn codex_catalog_requires_entitlement() {
        with_test_probe_hook_async(
            |_| ProbeOutcome::Entitled,
            || async {
                let store = codex_store();
                let models = catalog_for_active_account(&store, "work").await;
                let ids: Vec<String> = models.into_iter().map(|m| m.id).collect();
                assert_eq!(ids, OPENAI_CODEX_MODEL_IDS.iter().map(|id| id.to_string()).collect::<Vec<_>>());
            },
        )
        .await;

        with_test_probe_hook_async(
            |_| ProbeOutcome::NotEntitled("authenticated but not entitled for Codex use".to_string()),
            || async {
                let store = codex_store();
                assert!(catalog_for_active_account(&store, "work").await.is_empty());
            },
        )
        .await;
    }

    #[tokio::test]
    async fn codex_complete_fails_closed_when_probe_reports_not_entitled() {
        with_test_probe_hook_async(
            |_| {
                ProbeOutcome::NotEntitled(
                    "authenticated but not entitled for Codex use".to_string(),
                )
            },
            || async {
                let provider = test_provider(oauth_credential("acct-123"));
                let (tx, _rx) = mpsc::channel(1);
                let err = provider
                    .complete(codex_request(None), tx)
                    .await
                    .expect_err("unsupported account should fail closed");
                assert!(matches!(err, Error::Auth { .. }));
                assert_eq!(
                    err.to_string(),
                    "auth error: authenticated but not entitled for Codex use. ChatGPT Plus or Pro is required for openai-codex"
                );
                assert!(matches!(
                    entitlement_record("work").state,
                    EntitlementState::NotEntitled { .. }
                ));
            },
        )
        .await;
    }

    #[tokio::test]
    async fn codex_complete_fails_closed_when_probe_cannot_classify_account() {
        with_test_probe_hook_async(
            |_| ProbeOutcome::Error("boom".to_string()),
            || async {
                let provider = test_provider(oauth_credential("acct-123"));
                let (tx, _rx) = mpsc::channel(1);
                let err = provider
                    .complete(codex_request(None), tx)
                    .await
                    .expect_err("probe failure should surface as retriable provider error");
                assert_eq!(err.status_code(), Some(503));
                assert_eq!(err.to_string(), "provider error: openai-codex entitlement check failed: boom");
                assert!(matches!(entitlement_record("work").state, EntitlementState::Unknown));
                assert_eq!(entitlement_record("work").last_error.as_deref(), Some("boom"));
            },
        )
        .await;
    }

    #[test]
    fn classify_probe_response_treats_usage_not_included_as_not_entitled() {
        let outcome = classify_probe_response(400, r#"{"error":{"code":"usage_not_included"}}"#);
        assert_eq!(outcome, ProbeOutcome::NotEntitled("authenticated but not entitled for Codex use".to_string()));
    }

    #[test]
    fn classify_probe_response_treats_http_403_as_not_entitled() {
        let outcome = classify_probe_response(403, "forbidden");
        assert_eq!(outcome, ProbeOutcome::NotEntitled("authenticated but not entitled for Codex use".to_string()));
    }

    #[test]
    fn build_codex_request_body_matches_deterministic_fixture() {
        let request = codex_request_with_history(Some("session-1"));
        let body = build_codex_request_body(&request, Some("session-1")).expect("body should build");
        assert_eq!(body, codex_request_body_fixture(Some("session-1")));
    }

    #[test]
    fn build_codex_request_body_defaults_to_medium_verbosity_and_allows_override() {
        let default_request = codex_request_with_history(Some("session-1"));
        let default_body =
            build_codex_request_body(&default_request, Some("session-1")).expect("default body should build");
        assert_eq!(default_body.get("text"), Some(&json!({"verbosity": "medium"})));

        let mut overridden_request = codex_request_with_history(Some("session-1"));
        overridden_request.extra_params.insert("verbosity".to_string(), json!("low"));
        let overridden_body =
            build_codex_request_body(&overridden_request, Some("session-1")).expect("overridden body should build");
        assert_eq!(overridden_body.get("text"), Some(&json!({"verbosity": "low"})));
        assert_eq!(overridden_body.get("prompt_cache_key"), Some(&json!("session-1")));
        assert_eq!(overridden_body.get("input"), codex_request_body_fixture(Some("session-1")).get("input"));
    }

    #[test]
    fn build_codex_input_first_turn_contains_only_current_user_item() {
        let request = codex_request(None);
        let input = build_codex_input(&request.messages).expect("input should build");
        assert_eq!(input, vec![json!({
            "type": "message",
            "role": "user",
            "content": [{"type": "input_text", "text": "hello"}],
        })]);
    }

    #[test]
    fn build_codex_input_replays_signature_without_display_thinking_text() {
        let request = codex_request_with_history(Some("session-1"));
        let input = build_codex_input(&request.messages).expect("input should build");
        assert!(input.contains(
            &serde_json::from_str::<Value>(&codex_reasoning_signature_fixture()).expect("reasoning fixture json")
        ));
        let serialized = serde_json::to_string(&input).expect("input should serialize");
        assert!(!serialized.contains("display-only reasoning"), "input: {serialized}");
        assert!(input.iter().any(|item| {
            item.get("type") == Some(&json!("function_call"))
                && item.get("call_id") == Some(&json!("call-1"))
                && item.get("id") == Some(&json!("item-1"))
                && item.get("name") == Some(&json!("read"))
        }));
        assert!(input.iter().any(|item| {
            item.get("type") == Some(&json!("function_call_output"))
                && item.get("call_id") == Some(&json!("call-1"))
                && item.get("output") == Some(&json!("file body"))
        }));
    }

    #[test]
    fn build_codex_request_preserves_deterministic_body_fixture_on_initial_transient_and_refresh_retry_paths() {
        let client = common::build_http_client(Duration::from_secs(30)).expect("client should build");
        let request = codex_request_with_history(Some("session-1"));
        let expected_body = codex_request_body_fixture(Some("session-1"));
        let initial = build_codex_request(&client, &oauth_credential("acct-123"), &request)
            .expect("initial request should build");
        let transient_retry =
            build_codex_request(&client, &oauth_credential("acct-123"), &request).expect("retry request should build");
        let refresh_retry = build_codex_request(&client, &oauth_credential("acct-999"), &request)
            .expect("refresh retry request should build");

        for built in [&initial, &transient_retry, &refresh_retry] {
            assert_eq!(built.method(), reqwest::Method::POST);
            assert_eq!(built.url().as_str(), OPENAI_CODEX_RESPONSES_URL);
            assert_eq!(request_body_json(built), expected_body);
            assert_eq!(built.headers().get("OpenAI-Beta").unwrap(), OPENAI_CODEX_BETA_HEADER);
            assert_eq!(built.headers().get("originator").unwrap(), "pi");
            assert_eq!(built.headers().get("accept").unwrap(), "text/event-stream");
            assert_eq!(built.headers().get("content-type").unwrap(), "application/json");
            assert_eq!(built.headers().get("session_id").unwrap(), "session-1");
        }

        assert_eq!(
            header_subset(&initial, &["authorization", "chatgpt-account-id", "session_id"]),
            header_subset(&transient_retry, &["authorization", "chatgpt-account-id", "session_id"],)
        );
        assert_eq!(
            header_subset(&initial, &["authorization", "chatgpt-account-id", "session_id"]),
            BTreeMap::from([
                ("authorization".to_string(), format!("Bearer {}", fake_openai_codex_jwt("acct-123")),),
                ("chatgpt-account-id".to_string(), "acct-123".to_string()),
                ("session_id".to_string(), "session-1".to_string()),
            ])
        );
        assert_eq!(
            header_subset(&refresh_retry, &["authorization", "chatgpt-account-id", "session_id"],),
            BTreeMap::from([
                ("authorization".to_string(), format!("Bearer {}", fake_openai_codex_jwt("acct-999")),),
                ("chatgpt-account-id".to_string(), "acct-999".to_string()),
                ("session_id".to_string(), "session-1".to_string()),
            ])
        );
    }

    #[test]
    fn build_codex_request_omits_session_header_without_session_id() {
        let client = common::build_http_client(Duration::from_secs(30)).expect("client should build");
        let request = codex_request(None);
        let built =
            build_codex_request(&client, &oauth_credential("acct-123"), &request).expect("request should build");
        let body = request_body_json(&built);

        assert!(built.headers().get("session_id").is_none());
        assert!(body.get("prompt_cache_key").is_none());
    }

    #[test]
    fn build_probe_request_body_matches_contract() {
        let body = build_probe_request_body();
        assert_eq!(body.get("model"), Some(&json!("gpt-5.3-codex")));
        assert_eq!(body.get("store"), Some(&json!(false)));
        assert_eq!(body.get("stream"), Some(&json!(true)));
        assert_eq!(body.get("instructions"), Some(&json!("codex entitlement probe")));
        assert_eq!(body.get("text"), Some(&json!({"verbosity": "low"})));
        assert_eq!(
            body.get("input"),
            Some(&json!([{
                "role": "user",
                "content": [{"type": "input_text", "text": "ping"}],
            }]))
        );
        assert!(body.get("tools").is_none());
        assert!(body.get("prompt_cache_key").is_none());
    }

    #[tokio::test]
    async fn live_probe_does_not_retry_non_401_4xx() {
        let sleep_log = Arc::new(Mutex::new(Vec::new()));
        let server =
            MockHttpSequenceServer::start(vec![probe_json_response(400, json!({"error": {"message": "bad request"}}))])
                .await;

        let outcome = with_test_http_hooks(server.url.clone(), Some(Arc::clone(&sleep_log)), || async {
            live_probe(&oauth_credential("acct-123"), None).await
        })
        .await;

        assert_eq!(
            outcome,
            ProbeOutcome::Error(
                "entitlement probe failed with HTTP 400: {\"error\":{\"message\":\"bad request\"}}".to_string()
            )
        );
        assert_eq!(server.request_count.load(Ordering::SeqCst), 1);
        assert!(sleep_log.lock().expect("sleep log lock poisoned").is_empty());
    }

    #[tokio::test]
    async fn live_probe_retries_retryable_statuses_with_deterministic_backoff() {
        let sleep_log = Arc::new(Mutex::new(Vec::new()));
        let server = MockHttpSequenceServer::start(vec![
            probe_json_response(429, json!({"error": {"message": "rate limited"}})),
            probe_json_response(500, json!({"error": {"message": "server boom"}})),
            probe_json_response(504, json!({"error": {"message": "gateway timeout"}})),
            probe_json_response(200, json!({"ok": true})),
        ])
        .await;

        let outcome = with_test_http_hooks(server.url.clone(), Some(Arc::clone(&sleep_log)), || async {
            live_probe(&oauth_credential("acct-123"), None).await
        })
        .await;

        assert_eq!(outcome, ProbeOutcome::Entitled);
        assert_eq!(server.request_count.load(Ordering::SeqCst), 4);
        assert_eq!(*sleep_log.lock().expect("sleep log lock poisoned"), vec![
            Duration::from_secs(1),
            Duration::from_secs(2),
            Duration::from_secs(4)
        ]);
    }

    #[tokio::test]
    async fn openai_codex_attempt_does_not_retry_non_401_4xx() {
        let sleep_log = Arc::new(Mutex::new(Vec::new()));
        let refresh_calls = Arc::new(AtomicUsize::new(0));
        let server = MockHttpSequenceServer::start(vec![MockHttpResponse {
            status: 400,
            content_type: "application/json",
            body: json!({"error": {"message": "bad request"}}).to_string(),
        }])
        .await;

        let manager = test_request_manager(oauth_credential("acct-123"), Arc::clone(&refresh_calls));
        let (tx, _rx) = mpsc::channel(8);
        let mut attempt =
            OpenAICodexAttempt::new(codex_request(Some("session-1")), tx, oauth_credential("acct-123"), manager);
        let err = with_test_http_hooks(server.url.clone(), Some(Arc::clone(&sleep_log)), || async {
            attempt.run().await.expect_err("non-401 4xx should fail without retry")
        })
        .await;

        assert_eq!(server.request_count.load(Ordering::SeqCst), 1);
        assert_eq!(refresh_calls.load(Ordering::SeqCst), 0);
        assert!(sleep_log.lock().expect("sleep log lock poisoned").is_empty());
        assert_eq!(err.status_code(), Some(400));
        assert_eq!(err.to_string(), "provider error: bad request");
    }

    #[tokio::test]
    async fn openai_codex_attempt_refreshes_only_once_and_preserves_remaining_transient_budget() {
        let sleep_log = Arc::new(Mutex::new(Vec::new()));
        let refresh_calls = Arc::new(AtomicUsize::new(0));
        let server = MockHttpSequenceServer::start(vec![
            MockHttpResponse {
                status: 429,
                content_type: "application/json",
                body: json!({"error": {"message": "rate limited"}}).to_string(),
            },
            MockHttpResponse {
                status: 401,
                content_type: "application/json",
                body: json!({"error": {"message": "expired"}}).to_string(),
            },
            MockHttpResponse {
                status: 503,
                content_type: "application/json",
                body: json!({"error": {"message": "retry 1"}}).to_string(),
            },
            MockHttpResponse {
                status: 504,
                content_type: "application/json",
                body: json!({"error": {"message": "retry 2"}}).to_string(),
            },
            MockHttpResponse {
                status: 429,
                content_type: "application/json",
                body: json!({"error": {"message": "still limited"}}).to_string(),
            },
        ])
        .await;

        let manager = test_request_manager(oauth_credential("acct-123"), Arc::clone(&refresh_calls));
        let (tx, _rx) = mpsc::channel(8);
        let mut attempt =
            OpenAICodexAttempt::new(codex_request(Some("session-1")), tx, oauth_credential("acct-123"), manager);
        let err = with_test_http_hooks(server.url.clone(), Some(Arc::clone(&sleep_log)), || async {
            attempt.run().await.expect_err("retries should eventually exhaust")
        })
        .await;

        assert_eq!(server.request_count.load(Ordering::SeqCst), 5);
        assert_eq!(refresh_calls.load(Ordering::SeqCst), 1);
        assert_eq!(*sleep_log.lock().expect("sleep log lock poisoned"), vec![
            Duration::from_secs(1),
            Duration::from_secs(2),
            Duration::from_secs(4)
        ]);
        let captured = server.requests.lock().expect("requests lock poisoned");
        assert_eq!(
            captured[0].headers.get("authorization"),
            Some(&format!("Bearer {}", fake_openai_codex_jwt("acct-123")))
        );
        assert_eq!(
            captured[1].headers.get("authorization"),
            Some(&format!("Bearer {}", fake_openai_codex_jwt("acct-123")))
        );
        assert_eq!(
            captured[2].headers.get("authorization"),
            Some(&format!("Bearer {}", fake_openai_codex_jwt("acct-refreshed")))
        );
        assert_eq!(
            captured[3].headers.get("authorization"),
            Some(&format!("Bearer {}", fake_openai_codex_jwt("acct-refreshed")))
        );
        assert_eq!(
            captured[4].headers.get("authorization"),
            Some(&format!("Bearer {}", fake_openai_codex_jwt("acct-refreshed")))
        );
        assert_eq!(err.status_code(), Some(429));
        assert_eq!(err.to_string(), "provider error: still limited");
    }

    #[tokio::test]
    async fn ensure_entitlement_keeps_unknown_after_probe_failure() {
        let sleep_log = Arc::new(Mutex::new(Vec::new()));
        let server = MockHttpSequenceServer::start(vec![
            probe_json_response(500, json!({"error": {"message": "boom-1"}})),
            probe_json_response(503, json!({"error": {"message": "boom-2"}})),
            probe_json_response(429, json!({"error": {"message": "boom-3"}})),
            probe_json_response(500, json!({"error": {"message": "boom-4"}})),
        ])
        .await;

        let store = codex_store();
        let record = with_test_http_hooks(server.url.clone(), Some(Arc::clone(&sleep_log)), || async {
            ensure_entitlement(&store, "work", None).await
        })
        .await;

        assert!(matches!(record.state, EntitlementState::Unknown));
        assert!(record.last_error.as_deref().unwrap_or_default().contains("entitlement probe failed with HTTP 500"));
        assert_eq!(server.request_count.load(Ordering::SeqCst), 4);
        assert_eq!(*sleep_log.lock().expect("sleep log lock poisoned"), vec![
            Duration::from_secs(1),
            Duration::from_secs(2),
            Duration::from_secs(4)
        ]);
    }

    #[tokio::test]
    async fn openai_codex_provider_probes_before_first_normal_request() {
        let server =
            MockHttpSequenceServer::start(vec![probe_json_response(200, json!({"ok": true})), codex_sse_ok_response()])
                .await;
        let provider = test_provider(oauth_credential("acct-123"));
        let (tx, _rx) = mpsc::channel(8);

        with_test_http_hooks(server.url.clone(), None, || async {
            provider
                .complete(codex_request(Some("session-1")), tx)
                .await
                .expect("provider should probe then complete request");
        })
        .await;

        assert_eq!(server.request_count.load(Ordering::SeqCst), 2);
        let requests = server.requests.lock().expect("requests lock poisoned");
        let probe_body: Value = serde_json::from_str(&requests[0].body).expect("probe body json");
        let normal_body: Value = serde_json::from_str(&requests[1].body).expect("normal body json");
        assert_eq!(probe_body.get("stream"), Some(&json!(true)));
        assert_eq!(probe_body.get("model"), Some(&json!(OPENAI_CODEX_PROBE_MODEL)));
        assert!(probe_body.get("prompt_cache_key").is_none());
        assert_eq!(requests[0].headers.get("accept"), Some(&"text/event-stream".to_string()));
        assert!(requests[0].headers.get("session_id").is_none());
        assert_eq!(normal_body.get("stream"), Some(&json!(true)));
        assert_eq!(normal_body.get("model"), Some(&json!(OPENAI_CODEX_MODEL_IDS[0])));
        assert_eq!(normal_body.get("prompt_cache_key"), Some(&json!("session-1")));
        assert_eq!(requests[1].headers.get("session_id"), Some(&"session-1".to_string()));
    }

    #[tokio::test]
    async fn openai_codex_provider_fails_closed_without_sending_normal_request_when_not_entitled() {
        let server =
            MockHttpSequenceServer::start(vec![probe_json_response(403, json!({"error": {"message": "no plan"}}))])
                .await;
        let provider = test_provider(oauth_credential("acct-123"));
        let (tx, _rx) = mpsc::channel(8);

        let err = with_test_http_hooks(server.url.clone(), None, || async {
            provider
                .complete(codex_request(Some("session-1")), tx)
                .await
                .expect_err("not-entitled provider should fail closed")
        })
        .await;

        assert_eq!(server.request_count.load(Ordering::SeqCst), 1);
        assert_eq!(
            err.to_string(),
            "auth error: authenticated but not entitled for Codex use. ChatGPT Plus or Pro is required for openai-codex"
        );
        let requests = server.requests.lock().expect("requests lock poisoned");
        let probe_body: Value = serde_json::from_str(&requests[0].body).expect("probe body json");
        assert_eq!(probe_body.get("stream"), Some(&json!(true)));
        assert_eq!(requests[0].headers.get("accept"), Some(&"text/event-stream".to_string()));
    }

    #[tokio::test]
    async fn openai_codex_provider_fails_closed_without_sending_normal_request_when_probe_fails() {
        let sleep_log = Arc::new(Mutex::new(Vec::new()));
        let server = MockHttpSequenceServer::start(vec![
            probe_json_response(500, json!({"error": {"message": "boom-1"}})),
            probe_json_response(503, json!({"error": {"message": "boom-2"}})),
            probe_json_response(429, json!({"error": {"message": "boom-3"}})),
            probe_json_response(500, json!({"error": {"message": "boom-4"}})),
        ])
        .await;
        let provider = test_provider(oauth_credential("acct-123"));
        let (tx, _rx) = mpsc::channel(8);

        let err = with_test_http_hooks(server.url.clone(), Some(Arc::clone(&sleep_log)), || async {
            provider
                .complete(codex_request(Some("session-1")), tx)
                .await
                .expect_err("probe failure should fail closed")
        })
        .await;

        assert_eq!(server.request_count.load(Ordering::SeqCst), 4);
        assert_eq!(err.status_code(), Some(503));
        assert!(err.to_string().contains("provider error: openai-codex entitlement check failed"));
        let requests = server.requests.lock().expect("requests lock poisoned");
        assert!(requests.iter().all(|request| request.headers.get("session_id").is_none()));
        assert_eq!(*sleep_log.lock().expect("sleep log lock poisoned"), vec![
            Duration::from_secs(1),
            Duration::from_secs(2),
            Duration::from_secs(4)
        ]);
    }

    #[test]
    fn build_probe_request_preserves_contract_on_initial_transient_and_refresh_retry_paths() {
        let client = common::build_http_client(Duration::from_secs(30)).expect("client should build");
        let expected_body = build_probe_request_body();
        let initial =
            build_probe_request(&client, &oauth_credential("acct-123")).expect("initial request should build");
        let transient_retry =
            build_probe_request(&client, &oauth_credential("acct-123")).expect("retry request should build");
        let refresh_retry =
            build_probe_request(&client, &oauth_credential("acct-999")).expect("refresh retry request should build");

        for built in [&initial, &transient_retry, &refresh_retry] {
            assert_eq!(built.method(), reqwest::Method::POST);
            assert_eq!(built.url().as_str(), OPENAI_CODEX_RESPONSES_URL);
            assert_eq!(request_body_json(built), expected_body);
            assert_eq!(built.headers().get("OpenAI-Beta").unwrap(), OPENAI_CODEX_BETA_HEADER);
            assert_eq!(built.headers().get("originator").unwrap(), "pi");
            assert_eq!(built.headers().get("content-type").unwrap(), "application/json");
            assert_eq!(built.headers().get("accept").unwrap(), "text/event-stream");
            assert!(built.headers().get("session_id").is_none());
        }

        assert_eq!(
            header_subset(&initial, &["authorization", "chatgpt-account-id"]),
            header_subset(&transient_retry, &["authorization", "chatgpt-account-id"])
        );
        assert_eq!(
            header_subset(&initial, &["authorization", "chatgpt-account-id"]),
            BTreeMap::from([
                ("authorization".to_string(), format!("Bearer {}", fake_openai_codex_jwt("acct-123")),),
                ("chatgpt-account-id".to_string(), "acct-123".to_string()),
            ])
        );
        assert_eq!(
            header_subset(&refresh_retry, &["authorization", "chatgpt-account-id"]),
            BTreeMap::from([
                ("authorization".to_string(), format!("Bearer {}", fake_openai_codex_jwt("acct-999")),),
                ("chatgpt-account-id".to_string(), "acct-999".to_string()),
            ])
        );
    }

    #[tokio::test]
    async fn provider_reload_uses_layered_runtime_store() {
        let dir = tempfile::TempDir::new().expect("tempdir should exist");
        let seed_path = dir.path().join("seed.json");
        let runtime_path = dir.path().join("runtime.json");

        let mut seed = AuthStore::default();
        seed.set_credential(OPENAI_CODEX_PROVIDER, "work", StoredCredential::OAuth {
            access_token: fake_openai_codex_jwt("acct-123"),
            refresh_token: "refresh".to_string(),
            expires_at_ms: now_ms() + 1000,
            label: None,
        });
        seed.switch_account(OPENAI_CODEX_PROVIDER, "work");
        seed.save(&seed_path).expect("seed should save");

        let auth_paths = AuthStorePaths::layered(seed_path.clone(), runtime_path.clone());
        let manager = CredentialManager::with_refresh_fn(
            OPENAI_CODEX_PROVIDER.to_string(),
            seed.active_credential(OPENAI_CODEX_PROVIDER).expect("credential should exist").clone(),
            auth_paths,
            None,
            refresh_fn_for_codex(),
        );
        manager.reload_from_disk().await;
        assert!(runtime_path.parent().is_some());
    }
}
