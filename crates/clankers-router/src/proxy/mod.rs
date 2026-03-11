//! OpenAI-compatible HTTP proxy server + iroh p2p tunnel.
//!
//! Exposes the router as an OpenAI-compatible API so any tool that supports
//! `OPENAI_BASE_URL` (Cursor, aider, Continue, etc.) can use clankers-router
//! as a drop-in replacement.
//!
//! # Endpoints
//!
//! - `POST /v1/chat/completions` — streaming + non-streaming completions
//! - `GET  /v1/models`           — list available models
//! - `GET  /health`              — health check
//!
//! # Transport
//!
//! - **TCP** — standard HTTP on a configurable port (default :4000)
//! - **iroh QUIC** — the same HTTP API tunneled over iroh p2p connections, accessible by node ID
//!   from anywhere without port forwarding
//!
//! # Authentication
//!
//! Accepts `Authorization: Bearer <key>` headers. The proxy validates against
//! a configured set of allowed keys (or allows all if none are configured).

#[cfg(feature = "rpc")]
pub mod iroh_tunnel;

use std::convert::Infallible;
use std::net::SocketAddr;
use std::sync::Arc;

use axum::Router as AxumRouter;
use axum::extract::State;
use axum::http::HeaderMap;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::response::Json;
use axum::response::Response;
use axum::response::sse::Event as SseEvent;
use axum::response::sse::KeepAlive;
use axum::response::sse::Sse;
use axum::routing::get;
use axum::routing::post;
use serde::Deserialize;
use serde::Serialize;
use serde_json::Value;
use serde_json::json;
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;
use tower_http::cors::CorsLayer;
use tracing::debug;
use tracing::info;
use tracing::warn;

use crate::Router;
use crate::provider::CompletionRequest;
use crate::provider::ToolDefinition;
use crate::provider::Usage;
use crate::streaming::ContentBlock;
use crate::streaming::ContentDelta;
use crate::streaming::StreamEvent;

// ── Shared state ────────────────────────────────────────────────────────

/// Shared proxy state passed to all handlers via axum.
pub struct ProxyState {
    /// The underlying model router (shared with RPC handler when co-hosted).
    pub router: Arc<Router>,
    /// Allowed bearer tokens. Empty = allow all (no auth required).
    pub allowed_keys: Vec<String>,
}

// ── OpenAI request types ────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
struct ChatCompletionRequest {
    model: String,
    messages: Vec<OpenAIMessage>,
    #[serde(default)]
    stream: Option<bool>,
    #[serde(default)]
    max_tokens: Option<usize>,
    #[serde(default)]
    max_completion_tokens: Option<usize>,
    #[serde(default)]
    temperature: Option<f64>,
    #[serde(default)]
    tools: Option<Vec<OpenAITool>>,
    #[serde(default)]
    stream_options: Option<StreamOptions>,
    /// All other parameters (response_format, seed, top_p, frequency_penalty,
    /// presence_penalty, logprobs, top_logprobs, n, stop, etc.) forwarded
    /// verbatim to the upstream provider.
    #[serde(flatten)]
    extra: std::collections::HashMap<String, Value>,
}

#[derive(Debug, Deserialize)]
struct OpenAIMessage {
    role: String,
    #[serde(default)]
    content: Option<Value>,
    #[serde(default)]
    tool_calls: Option<Vec<Value>>,
    #[serde(default)]
    tool_call_id: Option<String>,
    #[serde(default)]
    name: Option<String>,
}

#[derive(Debug, Deserialize)]
struct OpenAITool {
    #[serde(default)]
    function: Option<OpenAIFunction>,
}

#[derive(Debug, Deserialize)]
struct OpenAIFunction {
    name: String,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    parameters: Option<Value>,
}

#[derive(Debug, Deserialize)]
struct StreamOptions {
    #[serde(default)]
    include_usage: Option<bool>,
}

// ── OpenAI response types ───────────────────────────────────────────────

#[derive(Debug, Serialize)]
struct ChatCompletionChunk {
    id: String,
    object: &'static str,
    created: i64,
    model: String,
    choices: Vec<ChunkChoice>,
    #[serde(skip_serializing_if = "Option::is_none")]
    usage: Option<ChunkUsage>,
}

#[derive(Debug, Serialize)]
struct ChunkChoice {
    index: usize,
    delta: ChunkDelta,
    #[serde(skip_serializing_if = "Option::is_none")]
    finish_reason: Option<String>,
}

#[derive(Debug, Serialize)]
struct ChunkDelta {
    #[serde(skip_serializing_if = "Option::is_none")]
    role: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    content: Option<String>,
    /// Reasoning content (extended thinking) — uses OpenAI's `reasoning_content` field
    /// so downstream tools (Cursor, Continue, etc.) that support it can display thinking.
    #[serde(skip_serializing_if = "Option::is_none")]
    reasoning_content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_calls: Option<Vec<ChunkToolCall>>,
}

#[derive(Debug, Serialize)]
struct ChunkToolCall {
    index: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "type")]
    call_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    function: Option<ChunkFunction>,
}

#[derive(Debug, Serialize)]
struct ChunkFunction {
    #[serde(skip_serializing_if = "Option::is_none")]
    name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    arguments: Option<String>,
}

#[derive(Debug, Serialize)]
struct ChunkUsage {
    prompt_tokens: usize,
    completion_tokens: usize,
    total_tokens: usize,
}

/// Non-streaming response.
#[derive(Debug, Serialize)]
struct ChatCompletionResponse {
    id: String,
    object: &'static str,
    created: i64,
    model: String,
    choices: Vec<ResponseChoice>,
    usage: ChunkUsage,
}

#[derive(Debug, Serialize)]
struct ResponseChoice {
    index: usize,
    message: ResponseMessage,
    finish_reason: String,
}

#[derive(Debug, Serialize)]
struct ResponseMessage {
    role: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_calls: Option<Vec<ResponseToolCall>>,
}

#[derive(Debug, Serialize)]
struct ResponseToolCall {
    id: String,
    #[serde(rename = "type")]
    call_type: String,
    function: ResponseFunction,
}

#[derive(Debug, Serialize)]
struct ResponseFunction {
    name: String,
    arguments: String,
}

/// OpenAI model list response.
#[derive(Debug, Serialize)]
struct ModelListResponse {
    object: &'static str,
    data: Vec<ModelObject>,
}

#[derive(Debug, Serialize)]
struct ModelObject {
    id: String,
    object: &'static str,
    created: i64,
    owned_by: String,
}

// ── Error response ──────────────────────────────────────────────────────

#[derive(Debug, Serialize)]
struct ErrorResponse {
    error: ErrorBody,
}

#[derive(Debug, Serialize)]
struct ErrorBody {
    message: String,
    #[serde(rename = "type")]
    error_type: String,
    code: Option<String>,
}

fn error_response(status: StatusCode, message: &str, error_type: &str) -> Response {
    let body = ErrorResponse {
        error: ErrorBody {
            message: message.to_string(),
            error_type: error_type.to_string(),
            code: None,
        },
    };
    (status, Json(body)).into_response()
}

// ── Auth middleware ──────────────────────────────────────────────────────

fn check_auth(state: &ProxyState, headers: &HeaderMap) -> Result<(), Response> {
    if state.allowed_keys.is_empty() {
        return Ok(()); // no auth required
    }

    let auth_header = headers.get("authorization").and_then(|v| v.to_str().ok()).unwrap_or("");

    let token = auth_header.strip_prefix("Bearer ").unwrap_or(auth_header);

    if token.is_empty() {
        return Err(error_response(
            StatusCode::UNAUTHORIZED,
            "Missing API key. Pass it via Authorization: Bearer <key>",
            "authentication_error",
        ));
    }

    if !state.allowed_keys.iter().any(|k| k == token) {
        return Err(error_response(StatusCode::FORBIDDEN, "Invalid API key", "authentication_error"));
    }

    Ok(())
}

// ── Request conversion ──────────────────────────────────────────────────

fn convert_request(req: ChatCompletionRequest) -> CompletionRequest {
    // Convert messages back to raw JSON (our router uses serde_json::Value)
    let messages: Vec<Value> = req
        .messages
        .into_iter()
        .map(|m| {
            let mut obj = serde_json::Map::new();
            obj.insert("role".into(), Value::String(m.role));
            if let Some(content) = m.content {
                obj.insert("content".into(), content);
            }
            if let Some(tool_calls) = m.tool_calls {
                obj.insert("tool_calls".into(), Value::Array(tool_calls));
            }
            if let Some(tool_call_id) = m.tool_call_id {
                obj.insert("tool_call_id".into(), Value::String(tool_call_id));
            }
            if let Some(name) = m.name {
                obj.insert("name".into(), Value::String(name));
            }
            Value::Object(obj)
        })
        .collect();

    // Extract system prompt from messages (OpenAI puts it in messages,
    // our router uses a separate field)
    let (system_prompt, messages) = extract_system_prompt(messages);

    // Convert tools
    let tools: Vec<ToolDefinition> = req
        .tools
        .unwrap_or_default()
        .into_iter()
        .filter_map(|t| {
            let func = t.function?;
            Some(ToolDefinition {
                name: func.name,
                description: func.description.unwrap_or_default(),
                input_schema: func.parameters.unwrap_or(json!({"type": "object"})),
            })
        })
        .collect();

    // max_completion_tokens is the newer OpenAI field, max_tokens is legacy
    let max_tokens = req.max_completion_tokens.or(req.max_tokens);

    CompletionRequest {
        model: req.model,
        messages,
        system_prompt,
        max_tokens,
        temperature: req.temperature,
        tools,
        thinking: None,
        extra_params: req.extra,
    }
}

/// Pull out system messages and merge them into a single system prompt.
fn extract_system_prompt(messages: Vec<Value>) -> (Option<String>, Vec<Value>) {
    let mut system_parts = Vec::new();
    let mut other_messages = Vec::new();

    for msg in messages {
        if msg.get("role").and_then(|r| r.as_str()) == Some("system") {
            if let Some(content) = msg.get("content").and_then(|c| c.as_str()) {
                system_parts.push(content.to_string());
            }
        } else {
            other_messages.push(msg);
        }
    }

    let system_prompt = if system_parts.is_empty() {
        None
    } else {
        Some(system_parts.join("\n\n"))
    };

    (system_prompt, other_messages)
}

// ── Streaming response conversion ───────────────────────────────────────

/// State machine for converting our StreamEvents into OpenAI SSE chunks.
struct ChunkConverter {
    id: String,
    model: String,
    created: i64,
    include_usage: bool,
    /// Track tool call indices for mapping our block indices to OpenAI's
    /// tool_call array indices (0-based, text block excluded).
    tool_call_index: usize,
}

impl ChunkConverter {
    fn new(include_usage: bool) -> Self {
        Self {
            id: format!("chatcmpl-{}", uuid::Uuid::new_v4().simple()),
            model: String::new(),
            created: chrono::Utc::now().timestamp(),
            include_usage,
            tool_call_index: 0,
        }
    }

    /// Convert a StreamEvent into zero or more OpenAI SSE chunks (as JSON strings).
    fn convert(&mut self, event: StreamEvent) -> Vec<String> {
        let mut chunks = Vec::new();

        match event {
            StreamEvent::MessageStart { message } => {
                self.model = message.model.clone();
                // Send initial chunk with role
                let chunk = ChatCompletionChunk {
                    id: self.id.clone(),
                    object: "chat.completion.chunk",
                    created: self.created,
                    model: message.model,
                    choices: vec![ChunkChoice {
                        index: 0,
                        delta: ChunkDelta {
                            role: Some("assistant".to_string()),
                            content: None,
                            reasoning_content: None,
                            tool_calls: None,
                        },
                        finish_reason: None,
                    }],
                    usage: None,
                };
                if let Ok(json) = serde_json::to_string(&chunk) {
                    chunks.push(json);
                }
            }

            StreamEvent::ContentBlockStart {
                content_block: ContentBlock::ToolUse { id, name, .. },
                ..
            } => {
                let idx = self.tool_call_index;
                self.tool_call_index += 1;
                let chunk = ChatCompletionChunk {
                    id: self.id.clone(),
                    object: "chat.completion.chunk",
                    created: self.created,
                    model: self.model.clone(),
                    choices: vec![ChunkChoice {
                        index: 0,
                        delta: ChunkDelta {
                            role: None,
                            content: None,
                            reasoning_content: None,
                            tool_calls: Some(vec![ChunkToolCall {
                                index: idx,
                                id: Some(id),
                                call_type: Some("function".to_string()),
                                function: Some(ChunkFunction {
                                    name: Some(name),
                                    arguments: Some(String::new()),
                                }),
                            }]),
                        },
                        finish_reason: None,
                    }],
                    usage: None,
                };
                if let Ok(json) = serde_json::to_string(&chunk) {
                    chunks.push(json);
                }
            }

            StreamEvent::ContentBlockDelta { delta, .. } => match delta {
                ContentDelta::TextDelta { text } => {
                    let chunk = ChatCompletionChunk {
                        id: self.id.clone(),
                        object: "chat.completion.chunk",
                        created: self.created,
                        model: self.model.clone(),
                        choices: vec![ChunkChoice {
                            index: 0,
                            delta: ChunkDelta {
                                role: None,
                                content: Some(text),
                                reasoning_content: None,
                                tool_calls: None,
                            },
                            finish_reason: None,
                        }],
                        usage: None,
                    };
                    if let Ok(json) = serde_json::to_string(&chunk) {
                        chunks.push(json);
                    }
                }
                ContentDelta::ThinkingDelta { thinking } => {
                    // Emit as `reasoning_content` (OpenAI's field for o3/o3-mini
                    // reasoning tokens). Tools that support this field will render it.
                    let chunk = ChatCompletionChunk {
                        id: self.id.clone(),
                        object: "chat.completion.chunk",
                        created: self.created,
                        model: self.model.clone(),
                        choices: vec![ChunkChoice {
                            index: 0,
                            delta: ChunkDelta {
                                role: None,
                                content: None,
                                reasoning_content: Some(thinking),
                                tool_calls: None,
                            },
                            finish_reason: None,
                        }],
                        usage: None,
                    };
                    if let Ok(json) = serde_json::to_string(&chunk) {
                        chunks.push(json);
                    }
                }
                ContentDelta::SignatureDelta { .. } => {
                    // Signature deltas are internal to the Anthropic protocol;
                    // they have no OpenAI equivalent, so we skip them here.
                }
                ContentDelta::InputJsonDelta { partial_json } => {
                    // Tool call argument streaming — the tool_call_index
                    // was incremented when we saw ContentBlockStart,
                    // so the current tool is tool_call_index - 1.
                    let idx = self.tool_call_index.saturating_sub(1);
                    let chunk = ChatCompletionChunk {
                        id: self.id.clone(),
                        object: "chat.completion.chunk",
                        created: self.created,
                        model: self.model.clone(),
                        choices: vec![ChunkChoice {
                            index: 0,
                            delta: ChunkDelta {
                                role: None,
                                content: None,
                                reasoning_content: None,
                                tool_calls: Some(vec![ChunkToolCall {
                                    index: idx,
                                    id: None,
                                    call_type: None,
                                    function: Some(ChunkFunction {
                                        name: None,
                                        arguments: Some(partial_json),
                                    }),
                                }]),
                            },
                            finish_reason: None,
                        }],
                        usage: None,
                    };
                    if let Ok(json) = serde_json::to_string(&chunk) {
                        chunks.push(json);
                    }
                }
            },

            StreamEvent::MessageDelta { stop_reason, usage } => {
                // Map our stop reasons to OpenAI's
                let finish_reason = stop_reason.as_deref().map(|r| match r {
                    "end_turn" => "stop".to_string(),
                    "tool_use" => "tool_calls".to_string(),
                    "max_tokens" => "length".to_string(),
                    other => other.to_string(),
                });

                if finish_reason.is_some() {
                    let chunk = ChatCompletionChunk {
                        id: self.id.clone(),
                        object: "chat.completion.chunk",
                        created: self.created,
                        model: self.model.clone(),
                        choices: vec![ChunkChoice {
                            index: 0,
                            delta: ChunkDelta {
                                role: None,
                                content: None,
                                reasoning_content: None,
                                tool_calls: None,
                            },
                            finish_reason,
                        }],
                        usage: None,
                    };
                    if let Ok(json) = serde_json::to_string(&chunk) {
                        chunks.push(json);
                    }
                }

                // Send usage in a separate chunk if requested
                if self.include_usage && (usage.input_tokens > 0 || usage.output_tokens > 0) {
                    let chunk = ChatCompletionChunk {
                        id: self.id.clone(),
                        object: "chat.completion.chunk",
                        created: self.created,
                        model: self.model.clone(),
                        choices: vec![],
                        usage: Some(ChunkUsage {
                            prompt_tokens: usage.input_tokens,
                            completion_tokens: usage.output_tokens,
                            total_tokens: usage.total_tokens(),
                        }),
                    };
                    if let Ok(json) = serde_json::to_string(&chunk) {
                        chunks.push(json);
                    }
                }
            }

            // These don't produce OpenAI chunks
            StreamEvent::ContentBlockStart { .. } | StreamEvent::ContentBlockStop { .. } | StreamEvent::MessageStop => {
            }

            StreamEvent::Error { error } => {
                warn!("Stream error forwarded to client: {error}");
            }
        }

        chunks
    }
}

// ── Handlers ────────────────────────────────────────────────────────────

/// POST /v1/chat/completions
async fn chat_completions(
    State(state): State<Arc<ProxyState>>,
    headers: HeaderMap,
    Json(body): Json<ChatCompletionRequest>,
) -> Response {
    if let Err(resp) = check_auth(&state, &headers) {
        return resp;
    }

    let streaming = body.stream.unwrap_or(false);
    let include_usage = body.stream_options.as_ref().and_then(|o| o.include_usage).unwrap_or(false);
    let requested_model = body.model.clone();

    let request = convert_request(body);

    debug!(
        "proxy: {} completion for model={} messages={}",
        if streaming { "streaming" } else { "non-streaming" },
        request.model,
        request.messages.len(),
    );

    if streaming {
        handle_streaming(state, request, include_usage).await
    } else {
        handle_non_streaming(state, request, requested_model).await
    }
}

async fn handle_streaming(state: Arc<ProxyState>, request: CompletionRequest, include_usage: bool) -> Response {
    let (event_tx, mut event_rx) = mpsc::channel::<StreamEvent>(64);
    let (sse_tx, sse_rx) = mpsc::channel::<Result<SseEvent, Infallible>>(64);

    // Spawn the router completion
    let router_handle = {
        let state = Arc::clone(&state);
        tokio::spawn(async move { state.router.complete(request, event_tx).await })
    };

    // Spawn the event converter
    tokio::spawn(async move {
        let mut converter = ChunkConverter::new(include_usage);

        while let Some(event) = event_rx.recv().await {
            let jsons = converter.convert(event);
            for json_str in jsons {
                let sse = SseEvent::default().data(json_str);
                if sse_tx.send(Ok(sse)).await.is_err() {
                    return;
                }
            }
        }

        // Wait for completion result to check for errors
        match router_handle.await {
            Ok(Err(e)) => {
                warn!("proxy: completion error: {e}");
                // Send error as a final SSE event before [DONE]
                let err_json = json!({
                    "error": {
                        "message": e.to_string(),
                        "type": "server_error",
                    }
                });
                let sse = SseEvent::default().data(err_json.to_string());
                let _ = sse_tx.send(Ok(sse)).await;
            }
            Err(e) => {
                warn!("proxy: completion task panicked: {e}");
            }
            Ok(Ok(())) => {}
        }

        // Send [DONE] sentinel
        let done = SseEvent::default().data("[DONE]");
        let _ = sse_tx.send(Ok(done)).await;
    });

    let stream = ReceiverStream::new(sse_rx);
    Sse::new(stream).keep_alive(KeepAlive::default()).into_response()
}

async fn handle_non_streaming(state: Arc<ProxyState>, request: CompletionRequest, requested_model: String) -> Response {
    let (tx, mut rx) = mpsc::channel::<StreamEvent>(64);

    let complete_handle = {
        let state = Arc::clone(&state);
        tokio::spawn(async move { state.router.complete(request, tx).await })
    };

    // Collect all events
    let mut msg_id = String::new();
    let mut model = requested_model;
    let mut text_content = String::new();
    let mut tool_calls: Vec<ResponseToolCall> = Vec::new();
    let mut current_tool: Option<(String, String, String)> = None; // (id, name, args)
    let mut usage = Usage::default();
    let mut stop_reason = "stop".to_string();

    while let Some(event) = rx.recv().await {
        match event {
            StreamEvent::MessageStart { message } => {
                msg_id = message.id;
                model = message.model;
            }
            StreamEvent::ContentBlockStart {
                content_block: ContentBlock::ToolUse { id, name, .. },
                ..
            } => {
                // Flush previous tool if any
                if let Some((tid, tname, targs)) = current_tool.take() {
                    tool_calls.push(ResponseToolCall {
                        id: tid,
                        call_type: "function".to_string(),
                        function: ResponseFunction {
                            name: tname,
                            arguments: targs,
                        },
                    });
                }
                current_tool = Some((id, name, String::new()));
            }
            StreamEvent::ContentBlockDelta { delta, .. } => match delta {
                ContentDelta::TextDelta { text } => {
                    text_content.push_str(&text);
                }
                ContentDelta::InputJsonDelta { partial_json } => {
                    if let Some((_, _, ref mut args)) = current_tool {
                        args.push_str(&partial_json);
                    }
                }
                ContentDelta::ThinkingDelta { .. } => {}
                ContentDelta::SignatureDelta { .. } => {}
            },
            StreamEvent::MessageDelta {
                stop_reason: sr,
                usage: u,
            } => {
                if let Some(r) = sr {
                    stop_reason = match r.as_str() {
                        "end_turn" => "stop".to_string(),
                        "tool_use" => "tool_calls".to_string(),
                        "max_tokens" => "length".to_string(),
                        other => other.to_string(),
                    };
                }
                usage.input_tokens += u.input_tokens;
                usage.output_tokens += u.output_tokens;
            }
            _ => {}
        }
    }

    // Flush last tool
    if let Some((tid, tname, targs)) = current_tool.take() {
        tool_calls.push(ResponseToolCall {
            id: tid,
            call_type: "function".to_string(),
            function: ResponseFunction {
                name: tname,
                arguments: targs,
            },
        });
    }

    // Check for completion errors
    if let Err(e) = complete_handle.await.unwrap_or(Ok(())) {
        return error_response(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string(), "server_error");
    }

    let response = ChatCompletionResponse {
        id: if msg_id.is_empty() {
            format!("chatcmpl-{}", uuid::Uuid::new_v4().simple())
        } else {
            msg_id
        },
        object: "chat.completion",
        created: chrono::Utc::now().timestamp(),
        model,
        choices: vec![ResponseChoice {
            index: 0,
            message: ResponseMessage {
                role: "assistant".to_string(),
                content: if text_content.is_empty() {
                    None
                } else {
                    Some(text_content)
                },
                tool_calls: if tool_calls.is_empty() { None } else { Some(tool_calls) },
            },
            finish_reason: stop_reason,
        }],
        usage: ChunkUsage {
            prompt_tokens: usage.input_tokens,
            completion_tokens: usage.output_tokens,
            total_tokens: usage.total_tokens(),
        },
    };

    Json(response).into_response()
}

/// GET /v1/models
async fn list_models(State(state): State<Arc<ProxyState>>, headers: HeaderMap) -> Response {
    if let Err(resp) = check_auth(&state, &headers) {
        return resp;
    }

    let models: Vec<ModelObject> = state
        .router
        .list_models()
        .iter()
        .map(|m| ModelObject {
            id: m.id.clone(),
            object: "model",
            created: 0,
            owned_by: m.provider.clone(),
        })
        .collect();

    Json(ModelListResponse {
        object: "list",
        data: models,
    })
    .into_response()
}

/// GET /health
async fn health() -> Json<Value> {
    Json(json!({
        "status": "ok",
    }))
}

// ── Server construction ─────────────────────────────────────────────────

/// Configuration for the proxy server.
pub struct ProxyConfig {
    /// Address to bind to (default: 127.0.0.1:4000).
    pub bind_addr: SocketAddr,
    /// Allowed API keys. Empty = no auth required.
    pub allowed_keys: Vec<String>,
}

impl Default for ProxyConfig {
    fn default() -> Self {
        Self {
            bind_addr: SocketAddr::from(([127, 0, 0, 1], 4000)),
            allowed_keys: Vec::new(),
        }
    }
}

/// Build the axum router (for use in tests or custom server setups).
pub fn build_app(state: Arc<ProxyState>) -> AxumRouter {
    AxumRouter::new()
        .route("/v1/chat/completions", post(chat_completions))
        .route("/v1/models", get(list_models))
        .route("/health", get(health))
        // Also handle without /v1 prefix for flexibility
        .route("/chat/completions", post(chat_completions))
        .route("/models", get(list_models))
        .layer(CorsLayer::permissive())
        .with_state(state)
}

/// Start the OpenAI-compatible proxy server.
///
/// This blocks until the server is shut down.
///
/// Accepts an `Arc<Router>` so the same router instance can be shared
/// with the RPC handler when both are co-hosted in `run_serve()`.
pub async fn serve(router: Arc<Router>, config: ProxyConfig) -> crate::Result<()> {
    let state = Arc::new(ProxyState {
        router,
        allowed_keys: config.allowed_keys,
    });

    let app = build_app(state);
    let listener = tokio::net::TcpListener::bind(config.bind_addr).await.map_err(|e| crate::Error::Config {
        message: format!("Failed to bind proxy to {}: {e}", config.bind_addr),
    })?;

    info!("OpenAI-compatible proxy listening on http://{}", config.bind_addr);
    info!("Set OPENAI_BASE_URL=http://{}/v1 to use", config.bind_addr);

    axum::serve(listener, app).await.map_err(|e| crate::Error::Config {
        message: format!("Proxy server error: {e}"),
    })
}

// ── Tests ───────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests;
