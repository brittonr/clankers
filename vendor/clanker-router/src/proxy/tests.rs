use async_trait::async_trait;

use super::*;
use crate::model::Model;
use crate::provider::Provider;

struct MockProvider {
    name: String,
    models: Vec<Model>,
}

#[async_trait]
impl Provider for MockProvider {
    async fn complete(&self, request: CompletionRequest, tx: mpsc::Sender<StreamEvent>) -> crate::Result<()> {
        use crate::streaming::ContentBlock;
        use crate::streaming::ContentDelta;
        use crate::streaming::MessageMetadata;

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
                    text: "Hello from proxy!".into(),
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
                    ..Default::default()
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
            max_input_tokens: 128_000,
            max_output_tokens: 16_384,
            supports_thinking: false,
            supports_images: true,
            supports_tools: true,
            input_cost_per_mtok: Some(1.0),
            output_cost_per_mtok: Some(2.0),
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

// ── Helper to drive axum in-process ─────────────────────────────

async fn request(
    app: &AxumRouter,
    method: &str,
    uri: &str,
    body: Option<Value>,
    auth: Option<&str>,
) -> (StatusCode, String) {
    use axum::body::Body;
    use axum::http::Request;
    use tower::ServiceExt;

    let body_bytes = body.map(|b| serde_json::to_vec(&b).unwrap()).unwrap_or_default();

    let mut req = Request::builder().method(method).uri(uri).header("content-type", "application/json");

    if let Some(token) = auth {
        req = req.header("authorization", format!("Bearer {}", token));
    }

    let req = req.body(Body::from(body_bytes)).unwrap();

    let response = app.clone().oneshot(req).await.unwrap();
    let status = response.status();
    let body = axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap();
    (status, String::from_utf8_lossy(&body).to_string())
}

// ── Tests ───────────────────────────────────────────────────────

#[tokio::test]
async fn test_health() {
    let app = make_test_app();
    let (status, body) = request(&app, "GET", "/health", None, None).await;
    assert_eq!(status, StatusCode::OK);
    let v: Value = serde_json::from_str(&body).unwrap();
    assert_eq!(v["status"], "ok");
}

#[tokio::test]
async fn test_list_models() {
    let app = make_test_app();
    let (status, body) = request(&app, "GET", "/v1/models", None, None).await;
    assert_eq!(status, StatusCode::OK);
    let v: Value = serde_json::from_str(&body).unwrap();
    assert_eq!(v["object"], "list");
    assert_eq!(v["data"][0]["id"], "test-model");
    assert_eq!(v["data"][0]["owned_by"], "test");
}

#[tokio::test]
async fn test_non_streaming_completion() {
    let app = make_test_app();
    let body = json!({
        "model": "test-model",
        "messages": [{"role": "user", "content": "hello"}],
    });
    let (status, resp) = request(&app, "POST", "/v1/chat/completions", Some(body), None).await;
    assert_eq!(status, StatusCode::OK);
    let v: Value = serde_json::from_str(&resp).unwrap();
    assert_eq!(v["object"], "chat.completion");
    assert_eq!(v["choices"][0]["message"]["content"], "Hello from proxy!");
    assert_eq!(v["choices"][0]["finish_reason"], "stop");
    assert_eq!(v["usage"]["prompt_tokens"], 10);
    assert_eq!(v["usage"]["completion_tokens"], 5);
    assert_eq!(v["usage"]["total_tokens"], 15);
}

#[tokio::test]
async fn test_auth_required_no_key() {
    let app = make_authed_app();
    let (status, body) = request(&app, "GET", "/v1/models", None, None).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    let v: Value = serde_json::from_str(&body).unwrap();
    assert!(v["error"]["message"].as_str().unwrap().contains("Missing"));
}

#[tokio::test]
async fn test_auth_required_wrong_key() {
    let app = make_authed_app();
    let (status, body) = request(&app, "GET", "/v1/models", None, Some("sk-wrong")).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let v: Value = serde_json::from_str(&body).unwrap();
    assert!(v["error"]["message"].as_str().unwrap().contains("Invalid"));
}

#[tokio::test]
async fn test_auth_required_correct_key() {
    let app = make_authed_app();
    let (status, body) = request(&app, "GET", "/v1/models", None, Some("sk-test-key")).await;
    assert_eq!(status, StatusCode::OK);
    let v: Value = serde_json::from_str(&body).unwrap();
    assert_eq!(v["object"], "list");
}

#[tokio::test]
async fn test_no_auth_when_no_keys_configured() {
    let app = make_test_app(); // no allowed_keys
    let (status, _) = request(&app, "GET", "/v1/models", None, None).await;
    assert_eq!(status, StatusCode::OK);
}

#[tokio::test]
async fn test_convert_request_extracts_system_prompt() {
    let req = ChatCompletionRequest {
        model: "gpt-4o".into(),
        messages: vec![
            OpenAIMessage {
                role: "system".into(),
                content: Some(Value::String("Be helpful".into())),
                tool_calls: None,
                tool_call_id: None,
                name: None,
            },
            OpenAIMessage {
                role: "user".into(),
                content: Some(Value::String("hi".into())),
                tool_calls: None,
                tool_call_id: None,
                name: None,
            },
        ],
        stream: None,
        max_tokens: None,
        max_completion_tokens: None,
        temperature: None,
        tools: None,
        stream_options: None,
        extra: Default::default(),
    };

    let converted = convert_request(req);
    assert_eq!(converted.system_prompt.as_deref(), Some("Be helpful"));
    assert_eq!(converted.messages.len(), 1); // system removed
    assert_eq!(converted.messages[0]["role"], "user");
}

#[tokio::test]
async fn test_convert_request_with_tools() {
    let req = ChatCompletionRequest {
        model: "gpt-4o".into(),
        messages: vec![],
        stream: None,
        max_tokens: None,
        max_completion_tokens: Some(4096),
        temperature: Some(0.5),
        tools: Some(vec![OpenAITool {
            function: Some(OpenAIFunction {
                name: "bash".into(),
                description: Some("Run a command".into()),
                parameters: Some(json!({"type": "object"})),
            }),
        }]),
        stream_options: None,
        extra: Default::default(),
    };

    let converted = convert_request(req);
    assert_eq!(converted.max_tokens, Some(4096));
    assert_eq!(converted.temperature, Some(0.5));
    assert_eq!(converted.tools.len(), 1);
    assert_eq!(converted.tools[0].name, "bash");
}

#[tokio::test]
async fn test_chunk_converter_text() {
    use crate::streaming::MessageMetadata;

    let mut conv = ChunkConverter::new(true);

    // MessageStart
    let chunks = conv.convert(StreamEvent::MessageStart {
        message: MessageMetadata {
            id: "msg-1".into(),
            model: "gpt-4o".into(),
            role: "assistant".into(),
        },
    });
    assert_eq!(chunks.len(), 1);
    let v: Value = serde_json::from_str(&chunks[0]).unwrap();
    assert_eq!(v["choices"][0]["delta"]["role"], "assistant");

    // Text delta
    let chunks = conv.convert(StreamEvent::ContentBlockDelta {
        index: 0,
        delta: ContentDelta::TextDelta { text: "Hello".into() },
    });
    assert_eq!(chunks.len(), 1);
    let v: Value = serde_json::from_str(&chunks[0]).unwrap();
    assert_eq!(v["choices"][0]["delta"]["content"], "Hello");

    // MessageDelta with stop + usage
    let chunks = conv.convert(StreamEvent::MessageDelta {
        stop_reason: Some("end_turn".into()),
        usage: Usage {
            input_tokens: 10,
            output_tokens: 5,
            ..Default::default()
        },
    });
    assert_eq!(chunks.len(), 2); // finish_reason chunk + usage chunk
    let v: Value = serde_json::from_str(&chunks[0]).unwrap();
    assert_eq!(v["choices"][0]["finish_reason"], "stop");
    let v: Value = serde_json::from_str(&chunks[1]).unwrap();
    assert_eq!(v["usage"]["prompt_tokens"], 10);
    assert_eq!(v["usage"]["completion_tokens"], 5);
}

#[tokio::test]
async fn test_chunk_converter_tool_calls() {
    let mut conv = ChunkConverter::new(false);
    conv.model = "test".into();

    // Tool start
    let chunks = conv.convert(StreamEvent::ContentBlockStart {
        index: 1,
        content_block: ContentBlock::ToolUse {
            id: "call_1".into(),
            name: "bash".into(),
            input: json!({}),
        },
    });
    assert_eq!(chunks.len(), 1);
    let v: Value = serde_json::from_str(&chunks[0]).unwrap();
    assert_eq!(v["choices"][0]["delta"]["tool_calls"][0]["id"], "call_1");
    assert_eq!(v["choices"][0]["delta"]["tool_calls"][0]["function"]["name"], "bash");

    // Tool argument delta
    let chunks = conv.convert(StreamEvent::ContentBlockDelta {
        index: 1,
        delta: ContentDelta::InputJsonDelta {
            partial_json: r#"{"cmd":"ls"}"#.into(),
        },
    });
    assert_eq!(chunks.len(), 1);
    let v: Value = serde_json::from_str(&chunks[0]).unwrap();
    assert_eq!(v["choices"][0]["delta"]["tool_calls"][0]["function"]["arguments"], r#"{"cmd":"ls"}"#);
}

#[tokio::test]
async fn test_v1_prefix_and_bare_routes() {
    let app = make_test_app();

    // Both /v1/models and /models should work
    let (s1, _) = request(&app, "GET", "/v1/models", None, None).await;
    let (s2, _) = request(&app, "GET", "/models", None, None).await;
    assert_eq!(s1, StatusCode::OK);
    assert_eq!(s2, StatusCode::OK);
}

#[tokio::test]
async fn test_streaming_completion() {
    let app = make_test_app();
    let body = json!({
        "model": "test-model",
        "messages": [{"role": "user", "content": "hello"}],
        "stream": true,
        "stream_options": { "include_usage": true },
    });

    // Send request via axum's in-process test infra
    use axum::body::Body;
    use axum::http::Request;
    use tower::ServiceExt;

    let req = Request::builder()
        .method("POST")
        .uri("/v1/chat/completions")
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_vec(&body).unwrap()))
        .unwrap();

    let response = app.clone().oneshot(req).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    // Read full body and parse SSE events
    let body = axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let text = String::from_utf8_lossy(&body);

    // Should contain data: lines and [DONE] sentinel
    let data_lines: Vec<&str> = text.lines().filter(|l| l.starts_with("data: ")).collect();
    assert!(
        data_lines.len() >= 3,
        "Expected at least 3 SSE data lines, got {}: {:?}",
        data_lines.len(),
        data_lines,
    );

    // Last data line should be [DONE]
    assert_eq!(*data_lines.last().unwrap(), "data: [DONE]");

    // First non-DONE chunk should have role: assistant
    let first_data = data_lines[0].strip_prefix("data: ").unwrap();
    let v: Value = serde_json::from_str(first_data).unwrap();
    assert_eq!(v["choices"][0]["delta"]["role"], "assistant");

    // Should contain content "Hello from proxy!"
    let has_content = data_lines.iter().any(|line| {
        let data = line.strip_prefix("data: ").unwrap_or("");
        if data == "[DONE]" {
            return false;
        }
        serde_json::from_str::<Value>(data)
            .ok()
            .and_then(|v| v["choices"][0]["delta"]["content"].as_str().map(String::from))
            .map(|c| c.contains("Hello from proxy!"))
            .unwrap_or(false)
    });
    assert!(has_content, "Stream should contain 'Hello from proxy!'");

    // Should contain usage with include_usage=true
    let has_usage = data_lines.iter().any(|line| {
        let data = line.strip_prefix("data: ").unwrap_or("");
        if data == "[DONE]" {
            return false;
        }
        serde_json::from_str::<Value>(data)
            .ok()
            .map(|v| v["usage"]["prompt_tokens"].as_u64().is_some())
            .unwrap_or(false)
    });
    assert!(has_usage, "Stream should contain usage chunk");
}

#[tokio::test]
async fn test_chunk_converter_thinking_delta() {
    let mut conv = ChunkConverter::new(false);
    conv.model = "claude-sonnet".into();

    // Thinking delta should produce a chunk with reasoning_content
    let chunks = conv.convert(StreamEvent::ContentBlockDelta {
        index: 0,
        delta: ContentDelta::ThinkingDelta {
            thinking: "Let me think about this...".into(),
        },
    });
    assert_eq!(chunks.len(), 1);
    let v: Value = serde_json::from_str(&chunks[0]).unwrap();
    assert_eq!(v["choices"][0]["delta"]["reasoning_content"], "Let me think about this...");
    // content should be absent (null)
    assert!(v["choices"][0]["delta"]["content"].is_null());
}
