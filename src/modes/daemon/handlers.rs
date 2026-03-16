//! Request and connection handlers for iroh and RPC protocols.
//!
//! Chat/1 sessions are backed by actor processes via `get_or_create_keyed_session`
//! and `prompt_and_collect`. RPC/1 uses the existing `ServerState` handler.

use std::sync::Arc;

use clanker_actor::ProcessRegistry;
use clankers_controller::transport::DaemonState;
use clankers_protocol::DaemonEvent;
use clankers_protocol::SessionKey;
use serde_json::json;
use tokio::sync::Mutex;
use tracing::info;
use tracing::warn;

use super::agent_process::get_or_create_keyed_session;
use super::session_store::AuthLayer;
use super::socket_bridge::SessionFactory;
use crate::modes::rpc::iroh;
use crate::modes::rpc::iroh::write_frame;
use crate::modes::rpc::protocol::Request;
use crate::modes::rpc::protocol::Response;

// ── Chat/1 handler (actor-backed) ───────────────────────────────────────────

/// Handle a clankers/chat/1 connection: bidirectional conversational stream.
///
/// Each prompt creates or reuses an actor session keyed by the peer's public
/// key. Responses are streamed back as JSON frames compatible with the legacy
/// protocol (`text_delta`, `tool_call`, `tool_result`, `done`/`error`).
pub(crate) async fn handle_chat_connection(
    conn: ::iroh::endpoint::Connection,
    state: Arc<Mutex<DaemonState>>,
    registry: ProcessRegistry,
    factory: Arc<SessionFactory>,
    auth: Option<Arc<AuthLayer>>,
    peer_id: &str,
) {
    let key = SessionKey::Iroh(peer_id.to_string());
    let mut first_frame = true;
    // When auth layer exists, peers must authenticate before sending prompts.
    // Verified capabilities are stored here and passed to sessions.
    let auth_required = auth.is_some();
    let mut authenticated = !auth_required; // open access when no auth layer
    let mut verified_capabilities: Option<Vec<clankers_ucan::Capability>> = None;

    loop {
        let (send, mut recv) = match conn.accept_bi().await {
            Ok(streams) => streams,
            Err(_) => break,
        };

        let data = match iroh::read_frame(&mut recv).await {
            Ok(d) => d,
            Err(_) => break,
        };

        let request: serde_json::Value = match serde_json::from_slice(&data) {
            Ok(v) => v,
            Err(_) => continue,
        };

        // Check for auth frame (first frame only)
        if first_frame {
            first_frame = false;
            if request.get("type").and_then(|v| v.as_str()) == Some("auth") {
                if let Some(token_b64) = request.get("token").and_then(|v| v.as_str())
                    && let Some(ref auth) = auth
                {
                    match clankers_ucan::Credential::from_base64(token_b64) {
                        Ok(cred) => match auth.verify_credential(&cred) {
                            Ok(caps) => {
                                info!("[{}] authenticated with {} capabilities", key, caps.len());
                                authenticated = true;
                                verified_capabilities = Some(caps);
                                auth.store_credential(peer_id, &cred);
                            }
                            Err(e) => {
                                warn!("[{}] credential verification failed: {e}", key);
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
                continue;
            }
        }

        // Reject prompts from unauthenticated peers when auth is required
        if !authenticated {
            warn!("[{}] rejected prompt from unauthenticated peer", key);
            let err = json!({
                "type": "error",
                "message": "Authentication required. Send an auth frame with a valid token first."
            });
            let mut send = send;
            let _ = write_frame(&mut send, &serde_json::to_vec(&err).unwrap_or_default()).await;
            let _ = send.finish();
            continue;
        }

        let text = match request.get("text").and_then(|v| v.as_str()) {
            Some(t) => t.to_string(),
            None => continue,
        };

        info!("[{}] prompt: {}", key, &text[..80.min(text.len())]);

        // Get or create an actor session for this peer (with capability enforcement)
        let state = Arc::clone(&state);
        let registry = registry.clone();
        let factory = Arc::clone(&factory);
        let key = key.clone();
        let caps = verified_capabilities.clone();
        tokio::spawn(async move {
            run_chat_prompt(state, registry, factory, key, text, send, caps).await;
        });
    }
}

/// Run a single chat/1 prompt via the actor session, streaming back events.
async fn run_chat_prompt(
    state: Arc<Mutex<DaemonState>>,
    registry: ProcessRegistry,
    factory: Arc<SessionFactory>,
    key: SessionKey,
    text: String,
    mut send: ::iroh::endpoint::SendStream,
    capabilities: Option<Vec<clankers_ucan::Capability>>,
) {
    let (_session_id, cmd_tx, event_tx) =
        get_or_create_keyed_session(&state, &registry, &factory, &key, capabilities).await;

    // Subscribe before sending the prompt so we don't miss events
    let mut event_rx = event_tx.subscribe();

    if cmd_tx
        .send(clankers_protocol::SessionCommand::Prompt {
            text: text.clone(),
            images: vec![],
        })
        .is_err()
    {
        let err = json!({ "type": "error", "message": "session disconnected" });
        let _ = write_frame(&mut send, &serde_json::to_vec(&err).unwrap_or_default()).await;
        let _ = send.finish();
        return;
    }

    // Stream DaemonEvents back as legacy chat/1 JSON frames
    let mut collected = String::new();
    loop {
        match event_rx.recv().await {
            Ok(DaemonEvent::TextDelta { ref text, .. }) => {
                collected.push_str(text);
                let frame = json!({ "type": "text_delta", "text": text });
                let bytes = serde_json::to_vec(&frame).unwrap_or_default();
                if write_frame(&mut send, &bytes).await.is_err() {
                    break;
                }
            }
            Ok(DaemonEvent::ToolCall {
                ref tool_name,
                ref call_id,
                ref input,
            }) => {
                let frame = json!({
                    "type": "tool_call",
                    "tool_name": tool_name,
                    "call_id": call_id,
                    "input": input,
                });
                let bytes = serde_json::to_vec(&frame).unwrap_or_default();
                let _ = write_frame(&mut send, &bytes).await;
            }
            Ok(DaemonEvent::ToolDone {
                ref call_id,
                ref text,
                is_error,
                ..
            }) => {
                let frame = json!({
                    "type": "tool_result",
                    "call_id": call_id,
                    "is_error": is_error,
                    "content": text,
                });
                let bytes = serde_json::to_vec(&frame).unwrap_or_default();
                let _ = write_frame(&mut send, &bytes).await;
            }
            Ok(DaemonEvent::AgentEnd | DaemonEvent::PromptDone { .. }) => break,
            Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
            Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                warn!("[{}] chat/1 client lagged, skipped {n} events", key);
            }
            _ => {}
        }
    }

    // Send final frame
    let final_frame = json!({ "type": "done", "text": collected });
    let _ = write_frame(&mut send, &serde_json::to_vec(&final_frame).unwrap_or_default()).await;
    let _ = send.finish();

    // Update last_active
    {
        let mut st = state.lock().await;
        if let Some(handle) = st.session_by_key(&key).map(|h| h.session_id.clone())
            && let Some(h) = st.sessions.get_mut(&handle)
        {
            h.turn_count += 1;
            h.last_active = chrono::Utc::now().to_rfc3339();
        }
    }

    info!("[{}] prompt complete", key);
}

// ── RPC/1 handler (unchanged) ───────────────────────────────────────────────

/// Handle a clankers/rpc/1 connection using the existing server code.
pub(crate) async fn handle_rpc_v1_connection(
    conn: ::iroh::endpoint::Connection,
    state: Arc<iroh::ServerState>,
    auth: Option<Arc<super::session_store::AuthLayer>>,
) {
    let peer_id = conn.remote_id().to_string();
    let mut first_frame = true;
    let auth_required = auth.is_some();
    let mut authenticated = !auth_required;

    loop {
        let (send, mut recv) = match conn.accept_bi().await {
            Ok(streams) => streams,
            Err(_) => break,
        };

        let data = match iroh::read_frame(&mut recv).await {
            Ok(d) => d,
            Err(_) => break,
        };

        let request: serde_json::Value = match serde_json::from_slice(&data) {
            Ok(v) => v,
            Err(_) => continue,
        };

        // Accept auth frame on first message
        if first_frame {
            first_frame = false;
            if request.get("type").and_then(|v| v.as_str()) == Some("auth") {
                if let Some(token_b64) = request.get("token").and_then(|v| v.as_str())
                    && let Some(ref auth) = auth
                {
                    match clankers_ucan::Credential::from_base64(token_b64) {
                        Ok(cred) => match auth.verify_credential(&cred) {
                            Ok(caps) => {
                                info!("[rpc/1 {}] authenticated with {} capabilities", &peer_id[..8.min(peer_id.len())], caps.len());
                                authenticated = true;
                                auth.store_credential(&peer_id, &cred);
                            }
                            Err(e) => {
                                warn!("[rpc/1 {}] credential verification failed: {e}", &peer_id[..8.min(peer_id.len())]);
                                let err = json!({ "error": format!("Token rejected: {e}") });
                                let mut send = send;
                                let _ = write_frame(&mut send, &serde_json::to_vec(&err).unwrap_or_default()).await;
                                let _ = send.finish();
                            }
                        },
                        Err(e) => {
                            warn!("[rpc/1] invalid token encoding: {e}");
                        }
                    }
                }
                continue;
            }
        }

        // Reject unauthenticated requests when auth is required
        if !authenticated {
            let err = json!({ "error": "Authentication required. Send an auth frame first." });
            let mut send = send;
            let _ = write_frame(&mut send, &serde_json::to_vec(&err).unwrap_or_default()).await;
            let _ = send.finish();
            continue;
        }

        // Parse as RPC request
        let request: Request = match serde_json::from_value(request) {
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
