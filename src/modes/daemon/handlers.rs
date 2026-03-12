//! Request and connection handlers for iroh and RPC protocols.

use std::sync::Arc;

use clankers_auth::Capability;
use clankers_auth::CapabilityToken;
use serde_json::json;
use tokio::sync::RwLock;
use tracing::info;
use tracing::warn;

use super::session_store::SessionKey;
use super::session_store::SessionStore;
use crate::agent::events::AgentEvent;
use crate::modes::rpc::iroh;
use crate::modes::rpc::iroh::write_frame;
use crate::modes::rpc::protocol::Request;
use crate::modes::rpc::protocol::Response;
use crate::provider::streaming::ContentDelta;

// ── iroh connection handler ─────────────────────────────────────────────────

// NOTE: The accept loop in mod.rs now handles ALPN dispatch directly,
// using handle_iroh_connection_from_conn() for rpc/1 and chat/1, and
// quic_bridge::handle_daemon_quic_connection() for daemon/1.

/// Handle a clankers/chat/1 connection: bidirectional conversational stream.
///
/// Wire format per stream (same framing as rpc/1):
///   Client → Server: `{ "text": "...", "session_hint": "..." }`
///   Server → Client: N × notification frames (text deltas, tool events)
///   Server → Client: 1 × final response frame
///
/// Optional auth frame (first frame on connection):
///   `{ "type": "auth", "token": "<base64>" }`
///   If present, the token is verified and capabilities are used for the session.
///   If absent, falls back to allowlist (backwards compatible).
pub(crate) async fn handle_chat_connection(
    conn: ::iroh::endpoint::Connection,
    store: Arc<RwLock<SessionStore>>,
    peer_id: &str,
) {
    let key = SessionKey::Iroh(peer_id.to_string());
    let mut auth_capabilities: Option<Vec<Capability>> = None;
    let mut first_frame = true;

    loop {
        let (send, mut recv) = match conn.accept_bi().await {
            Ok(streams) => streams,
            Err(_) => break, // connection closed
        };

        let data = match iroh::read_frame(&mut recv).await {
            Ok(d) => d,
            Err(_) => break,
        };

        let request: serde_json::Value = match serde_json::from_slice(&data) {
            Ok(v) => v,
            Err(_) => continue,
        };

        // Check for optional auth frame (first frame only)
        if first_frame {
            first_frame = false;
            if request.get("type").and_then(|v| v.as_str()) == Some("auth") {
                if let Some(token_b64) = request.get("token").and_then(|v| v.as_str()) {
                    let store_guard = store.read().await;
                    if let Some(ref auth) = store_guard.auth {
                        match CapabilityToken::from_base64(token_b64) {
                            Ok(token) => match auth.verify_token(&token) {
                                Ok(caps) => {
                                    info!("[{}] authenticated with {} capabilities", key, caps.len());
                                    auth_capabilities = Some(caps);
                                    // Store token for this peer
                                    auth.store_token(peer_id, &token);
                                }
                                Err(e) => {
                                    warn!("[{}] token verification failed: {e}", key);
                                    let err = json!({ "type": "error", "message": format!("Token rejected: {e}") });
                                    let mut send = send;
                                    let _ = write_frame(&mut send, &serde_json::to_vec(&err).unwrap_or_default()).await;
                                    let _ = send.finish();
                                }
                            },
                            Err(e) => {
                                warn!("[{}] invalid token encoding: {e}", key);
                            }
                        }
                    }
                }
                continue; // auth frame is consumed, not treated as a prompt
            }
        }

        let text = match request.get("text").and_then(|v| v.as_str()) {
            Some(t) => t.to_string(),
            None => continue,
        };

        info!("[{}] prompt: {}", key, &text[..80.min(text.len())]);

        // Run the prompt in the session
        let store_clone = Arc::clone(&store);
        let key_clone = key.clone();
        let caps = auth_capabilities.clone();
        tokio::spawn(async move {
            run_session_prompt(store_clone, key_clone, text, send, caps.as_deref()).await;
        });
    }
}

/// Handle a clankers/rpc/1 connection using the existing server code.
pub(crate) async fn handle_rpc_v1_connection(conn: ::iroh::endpoint::Connection, state: Arc<iroh::ServerState>) {
    loop {
        let (send, mut recv) = match conn.accept_bi().await {
            Ok(streams) => streams,
            Err(_) => break,
        };

        let data = match iroh::read_frame(&mut recv).await {
            Ok(d) => d,
            Err(_) => break,
        };

        let request: Request = match serde_json::from_slice(&data) {
            Ok(r) => r,
            Err(_) => continue,
        };

        let state = Arc::clone(&state);
        match request.method.as_str() {
            "prompt" => {
                tokio::spawn(async move {
                    iroh::handle_prompt_streaming_pub(&request, &state, send).await;
                });
            }
            _ => {
                let response = dispatch_rpc(&request, &state);
                let mut send = send;
                let _ = write_frame(&mut send, &serde_json::to_vec(&response).unwrap_or_default()).await;
                let _ = send.finish();
            }
        }
    }
}

/// Dispatch a non-streaming RPC request.
pub(crate) fn dispatch_rpc(request: &Request, state: &iroh::ServerState) -> Response {
    match request.method.as_str() {
        "ping" => Response::success(json!("pong")),
        "version" => Response::success(json!({
            "version": env!("CARGO_PKG_VERSION"),
            "name": "clankers",
            "mode": "daemon",
        })),
        "status" => {
            let mut result = json!({
                "status": "running",
                "mode": "daemon",
                "version": env!("CARGO_PKG_VERSION"),
                "accepts_prompts": true,
                "tags": state.meta.tags,
            });
            if let Some(ref ctx) = state.agent {
                let tool_names: Vec<&str> = ctx.tools.iter().map(|t| t.definition().name.as_str()).collect();
                result["tools"] = json!(tool_names);
                result["model"] = json!(ctx.model);
            }
            Response::success(result)
        }
        _ => Response::error(format!("Method not found: {}", request.method)),
    }
}

// ── Prompt execution ────────────────────────────────────────────────────────

/// Run a prompt against a persistent session, streaming events back.
pub(crate) async fn run_session_prompt(
    store: Arc<RwLock<SessionStore>>,
    key: SessionKey,
    text: String,
    mut send: ::iroh::endpoint::SendStream,
    capabilities: Option<&[Capability]>,
) {
    // Serialize prompts per session to prevent concurrent prompts from
    // racing on conversation history. Acquire the per-session lock before
    // touching session state; hold it until messages are written back.
    let prompt_lock = {
        let mut store = store.write().await;
        store.prompt_lock(&key)
    };
    let _prompt_guard = prompt_lock.lock().await;

    // Acquire the session (briefly hold write lock to get/create)
    let (mut agent, history) = {
        let mut store = store.write().await;
        let session = store.get_or_create(&key, capabilities);
        session.turn_count += 1;
        let messages = session.agent.messages().to_vec();
        let tools = session.session_tools.clone();
        let provider = Arc::clone(&store.provider);
        let settings = store.settings.clone();
        let model = store.model.clone();
        let system_prompt = store.system_prompt.clone();
        let agent = crate::agent::builder::AgentBuilder::new(provider, settings.clone(), model, system_prompt)
            .with_tools(tools)
            .build();
        (agent, messages)
    };

    // Seed the agent with conversation history
    agent.seed_messages(history);

    let mut rx = agent.subscribe();

    // Stream events to the sender
    let streamer = tokio::spawn(async move {
        let mut collected = String::new();

        while let Ok(event) = rx.recv().await {
            let frame = match event {
                AgentEvent::MessageUpdate {
                    delta: ContentDelta::TextDelta { ref text },
                    ..
                } => {
                    collected.push_str(text);
                    Some(json!({
                        "type": "text_delta",
                        "text": text,
                    }))
                }
                AgentEvent::ToolCall {
                    ref tool_name,
                    ref call_id,
                    ref input,
                } => Some(json!({
                    "type": "tool_call",
                    "tool_name": tool_name,
                    "call_id": call_id,
                    "input": input,
                })),
                AgentEvent::ToolExecutionEnd {
                    ref call_id,
                    ref result,
                    is_error,
                } => Some(json!({
                    "type": "tool_result",
                    "call_id": call_id,
                    "is_error": is_error,
                    "content": format!("{:?}", result),
                })),
                AgentEvent::AgentEnd { .. } => break,
                _ => None,
            };

            if let Some(frame) = frame {
                let bytes = serde_json::to_vec(&frame).unwrap_or_default();
                if write_frame(&mut send, &bytes).await.is_err() {
                    break;
                }
            }
        }

        (send, collected)
    });

    // Run the agent
    let result = agent.prompt(&text).await;

    let (mut send, collected) = match streamer.await {
        Ok(r) => r,
        Err(_) => return,
    };

    // Send final response
    let final_frame = match result {
        Ok(()) => json!({
            "type": "done",
            "text": collected,
        }),
        Err(e) => json!({
            "type": "error",
            "message": format!("{e}"),
        }),
    };
    let _ = write_frame(&mut send, &serde_json::to_vec(&final_frame).unwrap_or_default()).await;
    let _ = send.finish();

    // Save updated messages back to the session store
    let messages = agent.messages().to_vec();
    {
        let mut store = store.write().await;
        if let Some(session) = store.sessions.get_mut(&key) {
            session.agent.seed_messages(messages);
        }
    }

    info!("[{}] prompt complete", key);
}
