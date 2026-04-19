use axum::http::StatusCode;
use serde_json::{Value, json};
use std::sync::Arc;
use tokio::sync::mpsc;

use super::anthropic::*;
use super::*;
use crate::model::Model;
use crate::provider::{CompletionRequest, Provider, Usage};
use crate::router::Router;
use crate::streaming::*;

// ── Mock provider (reused from OpenAI proxy tests) ──────────────────────

struct MockProvider {
    name: String,
    models: Vec<Model>,
}

#[async_trait::async_trait]
impl Provider for MockProvider {
    async fn complete(&self, request: CompletionRequest, tx: mpsc::Sender<StreamEvent>) -> crate::Result<()> {
        let _ = tx
            .send(StreamEvent::MessageStart {
                message: MessageMetadata {
                    id: "msg-test".into(),
                    model: request.model.clone(),
                    role: "assistant".into(),
                },
            })
            .await;
        let _ = tx
            .send(StreamEvent::ContentBlockStart {
                index: 0,
                content_block: ContentBlock::Text { text: String::new() },
            })
            .await;
        let _ = tx
            .send(StreamEvent::ContentBlockDelta {
                index: 0,
                delta: ContentDelta::TextDelta {
                    text: "Hello from anthropic proxy!".into(),
                },
            })
            .await;
        let _ = tx.send(StreamEvent::ContentBlockStop { index: 0 }).await;
        let _ = tx
            .send(StreamEvent::MessageDelta {
                stop_reason: Some("end_turn".into()),
                usage: Usage {
                    input_tokens: 10,
                    output_tokens: 5,
                    cache_creation_input_tokens: 100,
                    cache_read_input_tokens: 50,
                },
            })
            .await;
        let _ = tx.send(StreamEvent::MessageStop).await;
        Ok(())
    }

    fn models(&self) -> &[Model] {
        &self.models
    }

    fn name(&self) -> &str {
        &self.name
    }
}

fn make_test_router() -> Router {
    let mut router = Router::new("test-model");
    router.register_provider(Arc::new(MockProvider {
        name: "test".into(),
        models: vec![Model {
            id: "test-model".into(),
            name: "Test Model".into(),
            provider: "test".into(),
            max_input_tokens: 200_000,
            max_output_tokens: 16_384,
            supports_thinking: true,
            supports_images: true,
            supports_tools: true,
            input_cost_per_mtok: Some(3.0),
            output_cost_per_mtok: Some(15.0),
        }],
    }));
    router
}

fn make_test_app() -> AxumRouter {
    let state = Arc::new(ProxyState {
        router: Arc::new(make_test_router()),
        allowed_keys: vec![],
    });
    build_app(state)
}

fn make_authed_app() -> AxumRouter {
    let state = Arc::new(ProxyState {
        router: Arc::new(make_test_router()),
        allowed_keys: vec!["sk-test-key".into()],
    });
    build_app(state)
}

async fn request_raw(
    app: &AxumRouter,
    uri: &str,
    body: Value,
    auth: Option<&str>,
) -> (StatusCode, String) {
    use axum::body::Body;
    use axum::http::Request;
    use tower::ServiceExt;

    let body_bytes = serde_json::to_vec(&body).unwrap();
    let mut req = Request::builder()
        .method("POST")
        .uri(uri)
        .header("content-type", "application/json");

    if let Some(token) = auth {
        req = req.header("authorization", format!("Bearer {}", token));
    }

    let req = req.body(Body::from(body_bytes)).unwrap();
    let response = app.clone().oneshot(req).await.unwrap();
    let status = response.status();
    let body = axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap();
    (status, String::from_utf8_lossy(&body).to_string())
}

/// Parse SSE data lines from raw response body.
fn parse_sse_events(body: &str) -> Vec<(String, Value)> {
    let mut events = Vec::new();
    let mut current_event = String::new();
    let mut current_data = String::new();

    for line in body.lines() {
        if let Some(ev) = line.strip_prefix("event: ") {
            current_event = ev.to_string();
        } else if let Some(data) = line.strip_prefix("data: ") {
            current_data = data.to_string();
        } else if line.is_empty() && !current_event.is_empty() {
            if let Ok(v) = serde_json::from_str::<Value>(&current_data) {
                events.push((current_event.clone(), v));
            }
            current_event.clear();
            current_data.clear();
        }
    }

    // Flush last event if no trailing blank line
    if !current_event.is_empty() && !current_data.is_empty() {
        if let Ok(v) = serde_json::from_str::<Value>(&current_data) {
            events.push((current_event, v));
        }
    }

    events
}

// ── 5.1 Unit test: convert_anthropic_request ────────────────────────────

#[test]
fn convert_basic_request() {
    let req = AnthropicRequest {
        model: "claude-sonnet-4-5-20250514".into(),
        messages: vec![json!({"role": "user", "content": [{"type": "text", "text": "Hello"}]})],
        max_tokens: 1024,
        stream: Some(true),
        system: Some(vec![
            json!({"type": "text", "text": "Be helpful", "cache_control": {"type": "ephemeral"}}),
        ]),
        tools: Some(vec![AnthropicTool {
            name: "bash".into(),
            description: Some("Run a command".into()),
            input_schema: Some(json!({"type": "object", "properties": {"cmd": {"type": "string"}}})),
        }]),
        temperature: Some(0.7),
        thinking: Some(AnthropicThinking::Enabled { budget_tokens: 5000 }),
    };

    let cr = convert_anthropic_request(req);
    assert_eq!(cr.model, "claude-sonnet-4-5-20250514");
    assert_eq!(cr.max_tokens, Some(1024));
    assert_eq!(cr.temperature, Some(0.7));

    // System prompt: plain text extracted
    assert_eq!(cr.system_prompt.as_deref(), Some("Be helpful"));

    // Raw system blocks preserved in extra_params
    let raw = cr.extra_params.get("_anthropic_system").unwrap();
    assert!(raw.is_array());
    assert_eq!(raw[0]["cache_control"]["type"], "ephemeral");

    // Messages pass through
    assert_eq!(cr.messages.len(), 1);
    assert_eq!(cr.messages[0]["role"], "user");

    // Tools converted
    assert_eq!(cr.tools.len(), 1);
    assert_eq!(cr.tools[0].name, "bash");

    // Thinking
    let t = cr.thinking.unwrap();
    assert!(t.enabled);
    assert_eq!(t.budget_tokens, Some(5000));
}

#[test]
fn convert_request_no_system() {
    let req = AnthropicRequest {
        model: "test-model".into(),
        messages: vec![json!({"role": "user", "content": "hi"})],
        max_tokens: 256,
        stream: Some(true),
        system: None,
        tools: None,
        temperature: None,
        thinking: None,
    };

    let cr = convert_anthropic_request(req);
    assert!(cr.system_prompt.is_none());
    assert!(!cr.extra_params.contains_key("_anthropic_system"));
}

#[test]
fn convert_request_thinking_disabled() {
    let req = AnthropicRequest {
        model: "test-model".into(),
        messages: vec![],
        max_tokens: 256,
        stream: Some(true),
        system: None,
        tools: None,
        temperature: None,
        thinking: Some(AnthropicThinking::Disabled),
    };

    let cr = convert_anthropic_request(req);
    let t = cr.thinking.unwrap();
    assert!(!t.enabled);
}

// ── 5.2 Unit test: AnthropicSseConverter ────────────────────────────────

#[test]
fn converter_message_start() {
    let mut conv = AnthropicSseConverter::new();
    let events = conv.convert(StreamEvent::MessageStart {
        message: MessageMetadata {
            id: "msg-123".into(),
            model: "claude-test".into(),
            role: "assistant".into(),
        },
    });
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].0, "message_start");
    let v: Value = serde_json::from_str(&events[0].1).unwrap();
    assert_eq!(v["type"], "message_start");
    assert_eq!(v["message"]["id"], "msg-123");
    assert_eq!(v["message"]["model"], "claude-test");
}

#[test]
fn converter_text_block() {
    let mut conv = AnthropicSseConverter::new();

    // content_block_start
    let events = conv.convert(StreamEvent::ContentBlockStart {
        index: 0,
        content_block: ContentBlock::Text { text: String::new() },
    });
    assert_eq!(events[0].0, "content_block_start");
    let v: Value = serde_json::from_str(&events[0].1).unwrap();
    assert_eq!(v["content_block"]["type"], "text");

    // content_block_delta
    let events = conv.convert(StreamEvent::ContentBlockDelta {
        index: 0,
        delta: ContentDelta::TextDelta { text: "Hello".into() },
    });
    let v: Value = serde_json::from_str(&events[0].1).unwrap();
    assert_eq!(v["delta"]["type"], "text_delta");
    assert_eq!(v["delta"]["text"], "Hello");

    // content_block_stop
    let events = conv.convert(StreamEvent::ContentBlockStop { index: 0 });
    let v: Value = serde_json::from_str(&events[0].1).unwrap();
    assert_eq!(v["type"], "content_block_stop");
    assert_eq!(v["index"], 0);
}

#[test]
fn converter_tool_use() {
    let mut conv = AnthropicSseConverter::new();

    let events = conv.convert(StreamEvent::ContentBlockStart {
        index: 1,
        content_block: ContentBlock::ToolUse {
            id: "toolu_123".into(),
            name: "bash".into(),
            input: json!({}),
        },
    });
    let v: Value = serde_json::from_str(&events[0].1).unwrap();
    assert_eq!(v["content_block"]["type"], "tool_use");
    assert_eq!(v["content_block"]["id"], "toolu_123");
    assert_eq!(v["content_block"]["name"], "bash");

    let events = conv.convert(StreamEvent::ContentBlockDelta {
        index: 1,
        delta: ContentDelta::InputJsonDelta {
            partial_json: r#"{"cmd":"ls"}"#.into(),
        },
    });
    let v: Value = serde_json::from_str(&events[0].1).unwrap();
    assert_eq!(v["delta"]["type"], "input_json_delta");
    assert_eq!(v["delta"]["partial_json"], r#"{"cmd":"ls"}"#);
}

#[test]
fn converter_thinking_with_signature() {
    let mut conv = AnthropicSseConverter::new();

    let events = conv.convert(StreamEvent::ContentBlockStart {
        index: 0,
        content_block: ContentBlock::Thinking {
            thinking: String::new(),
            signature: String::new(),
        },
    });
    let v: Value = serde_json::from_str(&events[0].1).unwrap();
    assert_eq!(v["content_block"]["type"], "thinking");

    let events = conv.convert(StreamEvent::ContentBlockDelta {
        index: 0,
        delta: ContentDelta::ThinkingDelta {
            thinking: "Let me think...".into(),
        },
    });
    let v: Value = serde_json::from_str(&events[0].1).unwrap();
    assert_eq!(v["delta"]["type"], "thinking_delta");
    assert_eq!(v["delta"]["thinking"], "Let me think...");

    let events = conv.convert(StreamEvent::ContentBlockDelta {
        index: 0,
        delta: ContentDelta::SignatureDelta {
            signature: "opaque-sig-abc".into(),
        },
    });
    let v: Value = serde_json::from_str(&events[0].1).unwrap();
    assert_eq!(v["delta"]["type"], "signature_delta");
    assert_eq!(v["delta"]["signature"], "opaque-sig-abc");
}

#[test]
fn converter_message_delta_with_cache_tokens() {
    let mut conv = AnthropicSseConverter::new();
    let events = conv.convert(StreamEvent::MessageDelta {
        stop_reason: Some("end_turn".into()),
        usage: Usage {
            input_tokens: 100,
            output_tokens: 50,
            cache_creation_input_tokens: 200,
            cache_read_input_tokens: 150,
        },
    });
    assert_eq!(events.len(), 1);
    let v: Value = serde_json::from_str(&events[0].1).unwrap();
    assert_eq!(v["type"], "message_delta");
    assert_eq!(v["delta"]["stop_reason"], "end_turn");
    assert_eq!(v["usage"]["input_tokens"], 100);
    assert_eq!(v["usage"]["output_tokens"], 50);
    assert_eq!(v["usage"]["cache_creation_input_tokens"], 200);
    assert_eq!(v["usage"]["cache_read_input_tokens"], 150);
}

#[test]
fn converter_error() {
    let mut conv = AnthropicSseConverter::new();
    let events = conv.convert(StreamEvent::Error {
        error: "something broke".into(),
    });
    let v: Value = serde_json::from_str(&events[0].1).unwrap();
    assert_eq!(v["type"], "error");
    assert_eq!(v["error"]["type"], "server_error");
    assert_eq!(v["error"]["message"], "something broke");
}

#[test]
fn converter_message_stop() {
    let mut conv = AnthropicSseConverter::new();
    let events = conv.convert(StreamEvent::MessageStop);
    let v: Value = serde_json::from_str(&events[0].1).unwrap();
    assert_eq!(v["type"], "message_stop");
}

// ── 5.3 Integration test: full SSE stream ───────────────────────────────

#[tokio::test]
async fn anthropic_endpoint_full_stream() {
    let app = make_test_app();
    let body = json!({
        "model": "test-model",
        "messages": [{"role": "user", "content": [{"type": "text", "text": "Hello"}]}],
        "max_tokens": 1024,
        "stream": true,
    });

    let (status, resp_body) = request_raw(&app, "/v1/messages", body, None).await;
    assert_eq!(status, StatusCode::OK);

    let events = parse_sse_events(&resp_body);
    assert!(!events.is_empty(), "Should have SSE events");

    // Check event sequence
    let event_types: Vec<&str> = events.iter().map(|(t, _)| t.as_str()).collect();
    assert_eq!(event_types[0], "message_start");
    assert!(event_types.contains(&"content_block_start"));
    assert!(event_types.contains(&"content_block_delta"));
    assert!(event_types.contains(&"content_block_stop"));
    assert!(event_types.contains(&"message_delta"));
    assert_eq!(*event_types.last().unwrap(), "message_stop");

    // Check message_start has correct structure
    let msg_start = &events[0].1;
    assert_eq!(msg_start["type"], "message_start");
    assert_eq!(msg_start["message"]["model"], "test-model");
    assert_eq!(msg_start["message"]["role"], "assistant");

    // Check content contains our text
    let has_text = events.iter().any(|(t, v)| {
        t == "content_block_delta" && v["delta"]["text"].as_str() == Some("Hello from anthropic proxy!")
    });
    assert!(has_text, "Should contain text delta");

    // Check message_delta has usage with cache tokens
    let msg_delta = events.iter().find(|(t, _)| t == "message_delta").unwrap();
    assert_eq!(msg_delta.1["delta"]["stop_reason"], "end_turn");
    assert_eq!(msg_delta.1["usage"]["cache_creation_input_tokens"], 100);
    assert_eq!(msg_delta.1["usage"]["cache_read_input_tokens"], 50);
}

#[tokio::test]
async fn anthropic_endpoint_bare_route() {
    // /messages (without /v1 prefix) should also work
    let app = make_test_app();
    let body = json!({
        "model": "test-model",
        "messages": [{"role": "user", "content": "hi"}],
        "max_tokens": 256,
        "stream": true,
    });

    let (status, _) = request_raw(&app, "/messages", body, None).await;
    assert_eq!(status, StatusCode::OK);
}

// ── 5.4 Integration test: error cases ───────────────────────────────────

#[tokio::test]
async fn anthropic_endpoint_rejects_non_streaming() {
    let app = make_test_app();
    let body = json!({
        "model": "test-model",
        "messages": [{"role": "user", "content": "hi"}],
        "max_tokens": 256,
        "stream": false,
    });

    let (status, resp) = request_raw(&app, "/v1/messages", body, None).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let v: Value = serde_json::from_str(&resp).unwrap();
    assert_eq!(v["type"], "error");
    assert_eq!(v["error"]["type"], "invalid_request_error");
}

#[tokio::test]
async fn anthropic_endpoint_rejects_missing_stream() {
    let app = make_test_app();
    let body = json!({
        "model": "test-model",
        "messages": [{"role": "user", "content": "hi"}],
        "max_tokens": 256,
    });

    let (status, resp) = request_raw(&app, "/v1/messages", body, None).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let v: Value = serde_json::from_str(&resp).unwrap();
    assert_eq!(v["error"]["type"], "invalid_request_error");
}

#[tokio::test]
async fn anthropic_endpoint_auth_required_no_key() {
    let app = make_authed_app();
    let body = json!({
        "model": "test-model",
        "messages": [{"role": "user", "content": "hi"}],
        "max_tokens": 256,
        "stream": true,
    });

    let (status, resp) = request_raw(&app, "/v1/messages", body, None).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    let v: Value = serde_json::from_str(&resp).unwrap();
    assert_eq!(v["type"], "error");
    assert_eq!(v["error"]["type"], "authentication_error");
}

#[tokio::test]
async fn anthropic_endpoint_auth_wrong_key() {
    let app = make_authed_app();
    let body = json!({
        "model": "test-model",
        "messages": [{"role": "user", "content": "hi"}],
        "max_tokens": 256,
        "stream": true,
    });

    let (status, resp) = request_raw(&app, "/v1/messages", body, Some("sk-wrong")).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let v: Value = serde_json::from_str(&resp).unwrap();
    assert_eq!(v["error"]["type"], "authentication_error");
}

#[tokio::test]
async fn anthropic_endpoint_auth_correct_key() {
    let app = make_authed_app();
    let body = json!({
        "model": "test-model",
        "messages": [{"role": "user", "content": "hi"}],
        "max_tokens": 256,
        "stream": true,
    });

    let (status, _) = request_raw(&app, "/v1/messages", body, Some("sk-test-key")).await;
    assert_eq!(status, StatusCode::OK);
}

#[tokio::test]
async fn anthropic_endpoint_no_auth_when_unconfigured() {
    let app = make_test_app(); // no allowed_keys
    let body = json!({
        "model": "test-model",
        "messages": [{"role": "user", "content": "hi"}],
        "max_tokens": 256,
        "stream": true,
    });

    let (status, _) = request_raw(&app, "/v1/messages", body, None).await;
    assert_eq!(status, StatusCode::OK);
}

#[tokio::test]
async fn anthropic_endpoint_rejects_missing_model() {
    let app = make_test_app();

    let body = json!({
        "messages": [{"role": "user", "content": "hi"}],
        "max_tokens": 256,
        "stream": true,
    });
    let (status, resp) = request_raw(&app, "/v1/messages", body, None).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let v: Value = serde_json::from_str(&resp).unwrap();
    assert_eq!(v["error"]["type"], "invalid_request_error");
}

#[tokio::test]
async fn anthropic_endpoint_rejects_missing_messages() {
    let app = make_test_app();

    let body = json!({
        "model": "test-model",
        "max_tokens": 256,
        "stream": true,
    });
    let (status, resp) = request_raw(&app, "/v1/messages", body, None).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let v: Value = serde_json::from_str(&resp).unwrap();
    assert_eq!(v["error"]["type"], "invalid_request_error");
}

// ── 5.5 Raw passthrough tests ────────────────────────────────────────

#[test]
fn raw_passthrough_preserves_full_body() {
    use crate::backends::anthropic::build_request_body_for_test;

    // A realistic clankers request with all the fields the old handler dropped
    let raw = json!({
        "model": "claude-sonnet-4-5-20250514",
        "messages": [
            {"role": "user", "content": [{"type": "text", "text": "hello"}]}
        ],
        "max_tokens": 16384,
        "stream": true,
        "system": [
            {
                "type": "text",
                "text": "You are a helpful assistant.",
                "cache_control": {"type": "ephemeral"}
            },
            {
                "type": "text",
                "text": "Additional context.",
                "cache_control": {"type": "ephemeral", "ttl": "1h"}
            }
        ],
        "tools": [
            {
                "name": "bash",
                "description": "Run a command",
                "input_schema": {"type": "object", "properties": {"cmd": {"type": "string"}}},
                "cache_control": {"type": "ephemeral"}
            },
            {
                "name": "read",
                "description": "Read a file",
                "input_schema": {"type": "object"},
                "cache_control": {"type": "ephemeral"}
            }
        ],
        "tool_choice": {"type": "auto"},
        "metadata": {"user_id": "test-user"},
        "thinking": {"type": "enabled", "budget_tokens": 10000},
        "temperature": 1.0
    });

    let cr = convert_raw_to_completion_request(raw.clone());
    assert_eq!(cr.model, "claude-sonnet-4-5-20250514");
    assert_eq!(cr.system_prompt.as_deref(), Some("You are a helpful assistant.\n\nAdditional context."));

    // Build the request body the Anthropic backend would send
    let body = build_request_body_for_test(&cr, false).unwrap();

    // Every field from the original request must be preserved
    assert_eq!(body["model"], "claude-sonnet-4-5-20250514");
    assert_eq!(body["max_tokens"], 16384);
    assert_eq!(body["stream"], true);

    // temperature must be stripped when thinking is enabled (Anthropic rejects it)
    assert!(body.get("temperature").is_none() || body["temperature"].is_null(),
        "temperature must be omitted when thinking is enabled");

    // System blocks with cache_control intact
    let system = body["system"].as_array().unwrap();
    assert_eq!(system.len(), 2);
    assert_eq!(system[0]["cache_control"]["type"], "ephemeral");
    assert_eq!(system[1]["cache_control"]["ttl"], "1h");

    // Tools with INDIVIDUAL cache_control (the old handler dropped these)
    let tools = body["tools"].as_array().unwrap();
    assert_eq!(tools.len(), 2);
    assert_eq!(tools[0]["cache_control"]["type"], "ephemeral");
    assert_eq!(tools[1]["cache_control"]["type"], "ephemeral");

    // Fields the old handler dropped entirely
    assert_eq!(body["tool_choice"]["type"], "auto");
    assert_eq!(body["metadata"]["user_id"], "test-user");
    assert_eq!(body["thinking"]["type"], "enabled");
    assert_eq!(body["thinking"]["budget_tokens"], 10000);
}

#[test]
fn raw_passthrough_updates_model_for_fallback() {
    use crate::backends::anthropic::build_request_body_for_test;

    let raw = json!({
        "model": "claude-sonnet-4-5-20250514",
        "messages": [{"role": "user", "content": "hi"}],
        "max_tokens": 1024,
        "stream": true,
    });

    let mut cr = convert_raw_to_completion_request(raw);
    // Router fallback changes the model
    cr.model = "claude-haiku-4-5-20250514".to_string();

    let body = build_request_body_for_test(&cr, false).unwrap();
    assert_eq!(body["model"], "claude-haiku-4-5-20250514");
    assert_eq!(body["stream"], true);
}

#[test]
fn non_proxy_request_still_reconstructs() {
    use crate::backends::anthropic::build_request_body_for_test;

    // Regular request (not from proxy) — no _anthropic_raw_body
    let cr = CompletionRequest {
        model: "test-model".into(),
        messages: vec![json!({"role": "user", "content": "hi"})],
        system_prompt: Some("Be helpful".into()),
        max_tokens: Some(1024),
        temperature: None,
        tools: vec![],
        thinking: None,
        no_cache: false,
        cache_ttl: None,
        extra_params: Default::default(),
    };

    let body = build_request_body_for_test(&cr, false).unwrap();
    let system = body["system"].as_array().unwrap();
    assert_eq!(system.len(), 1);
    assert_eq!(system[0]["text"], "Be helpful");
    assert!(system[0].get("cache_control").is_some());
}

// ── 5.6 Legacy typed conversion tests (convert_anthropic_request) ─────

#[test]
fn system_cache_control_round_trip_via_typed_path() {
    use crate::backends::anthropic::build_request_body_for_test;

    let system_blocks = vec![
        json!({
            "type": "text",
            "text": "You are a helpful assistant.",
            "cache_control": {"type": "ephemeral"}
        }),
        json!({
            "type": "text",
            "text": "Additional context here.",
            "cache_control": {"type": "ephemeral", "ttl": "1h"}
        }),
    ];

    let req = AnthropicRequest {
        model: "test-model".into(),
        messages: vec![json!({"role": "user", "content": "hi"})],
        max_tokens: 1024,
        stream: Some(true),
        system: Some(system_blocks.clone()),
        tools: None,
        temperature: None,
        thinking: None,
    };

    let cr = convert_anthropic_request(req);

    let raw = cr.extra_params.get("_anthropic_system").unwrap();
    assert_eq!(raw[0]["cache_control"]["type"], "ephemeral");
    assert_eq!(raw[1]["cache_control"]["ttl"], "1h");

    let body = build_request_body_for_test(&cr, false).unwrap();
    let system = body["system"].as_array().unwrap();
    assert_eq!(system.len(), 2);
    assert_eq!(system[0]["text"], "You are a helpful assistant.");
    assert_eq!(system[0]["cache_control"]["type"], "ephemeral");
    assert_eq!(system[1]["cache_control"]["ttl"], "1h");
}

// ── 5.7 Raw passthrough: temperature + thinking interaction ────────────

#[test]
fn raw_passthrough_strips_temperature_when_thinking_enabled() {
    use crate::backends::anthropic::build_request_body_for_test;

    let raw = json!({
        "model": "claude-sonnet-4-20250514",
        "messages": [{"role": "user", "content": "hi"}],
        "max_tokens": 16384,
        "stream": true,
        "temperature": 0.7,
        "thinking": {"type": "enabled", "budget_tokens": 10000}
    });

    let cr = convert_raw_to_completion_request(raw);
    let body = build_request_body_for_test(&cr, false).unwrap();

    // Anthropic rejects temperature when thinking is enabled
    assert!(body.get("temperature").is_none(),
        "temperature must be stripped when thinking is enabled");
    // thinking itself is preserved
    assert_eq!(body["thinking"]["type"], "enabled");
    assert_eq!(body["thinking"]["budget_tokens"], 10000);
}

#[test]
fn raw_passthrough_preserves_temperature_without_thinking() {
    use crate::backends::anthropic::build_request_body_for_test;

    let raw = json!({
        "model": "claude-sonnet-4-20250514",
        "messages": [{"role": "user", "content": "hi"}],
        "max_tokens": 1024,
        "stream": true,
        "temperature": 0.7
    });

    let cr = convert_raw_to_completion_request(raw);
    let body = build_request_body_for_test(&cr, false).unwrap();

    // Without thinking, temperature passes through
    assert_eq!(body["temperature"], 0.7);
}

#[test]
fn raw_passthrough_no_temperature_no_thinking() {
    use crate::backends::anthropic::build_request_body_for_test;

    let raw = json!({
        "model": "claude-sonnet-4-20250514",
        "messages": [{"role": "user", "content": "hi"}],
        "max_tokens": 1024,
        "stream": true
    });

    let cr = convert_raw_to_completion_request(raw);
    let body = build_request_body_for_test(&cr, false).unwrap();

    assert!(body.get("temperature").is_none());
    assert!(body.get("thinking").is_none());
}
