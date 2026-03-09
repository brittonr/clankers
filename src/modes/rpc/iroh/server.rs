//! Server-side RPC handling

use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;

use iroh::Endpoint;
use serde_json::json;
use tracing::info;
use tracing::warn;

use super::ServerState;
use super::protocol::read_frame;
use super::protocol::write_frame;
use crate::agent::events::AgentEvent;
use crate::modes::rpc::protocol::Request;
use crate::modes::rpc::protocol::Response;
use crate::provider::streaming::ContentDelta;

// ── Server: accept connections ──────────────────────────────────────────────

pub async fn serve_rpc(endpoint: Endpoint, state: Arc<ServerState>) -> Result<(), crate::error::Error> {
    info!("RPC server listening as {}", endpoint.id().fmt_short());
    loop {
        let incoming = match endpoint.accept().await {
            Some(incoming) => incoming,
            None => break,
        };

        let state = state.clone();
        tokio::spawn(async move {
            if let Err(e) = handle_connection(incoming, state).await {
                warn!("RPC connection error: {}", e);
            }
        });
    }
    Ok(())
}

async fn handle_connection(
    incoming: iroh::endpoint::Incoming,
    state: Arc<ServerState>,
) -> Result<(), crate::error::Error> {
    let conn = incoming.await.map_err(|e| crate::error::Error::Provider {
        message: format!("Connection failed: {}", e),
    })?;

    let remote = conn.remote_id();

    // Auth check
    if !state.acl.is_allowed(&remote) {
        warn!("Rejected connection from unauthorized peer {}", remote.fmt_short());
        conn.close(1u32.into(), b"unauthorized");
        return Ok(());
    }

    info!("Accepted connection from {}", remote.fmt_short());

    loop {
        let (send, mut recv) = match conn.accept_bi().await {
            Ok(streams) => streams,
            Err(_) => break,
        };

        // Read request
        let data = match read_frame(&mut recv).await {
            Ok(d) => d,
            Err(_) => break,
        };

        let request: Request = match serde_json::from_slice(&data) {
            Ok(r) => r,
            Err(_) => continue,
        };

        match request.method.as_str() {
            // Streaming prompt — needs special handling
            "prompt" => {
                let state = state.clone();
                tokio::spawn(async move {
                    handle_prompt_streaming(&request, &state, send).await;
                });
            }
            // File receive (client is sending us a file)
            "file.send" => {
                let state = state.clone();
                tokio::spawn(async move {
                    handle_file_send(&request, &state, recv, send).await;
                });
            }
            // File send (client wants to download a file from us)
            "file.recv" => {
                // Handle inline to avoid race between spawned task and connection lifecycle
                handle_file_recv(&request, recv, send).await;
            }
            // Simple request/response methods
            _ => {
                let response = handle_rpc_request(&request, &state);
                let response_bytes = serde_json::to_vec(&response).unwrap_or_default();
                let mut send = send;
                let _ = write_frame(&mut send, &response_bytes).await;
                let _ = send.finish();
            }
        }
    }
    Ok(())
}

fn handle_rpc_request(request: &Request, state: &ServerState) -> Response {
    match request.method.as_str() {
        "ping" => Response::success(json!("pong")),

        "version" => Response::success(json!({
            "version": env!("CARGO_PKG_VERSION"),
            "name": "clankers"
        })),

        "status" => {
            let mut result = json!({
                "status": "running",
                "version": env!("CARGO_PKG_VERSION"),
                "accepts_prompts": state.agent.is_some(),
                "tags": state.meta.tags,
                "agents": state.meta.agent_names,
            });
            if let Some(ref ctx) = state.agent {
                let tool_names: Vec<&str> = ctx.tools.iter().map(|t| t.definition().name.as_str()).collect();
                result["tools"] = json!(tool_names);
                result["model"] = json!(ctx.model);
            }
            Response::success(result)
        }

        // prompt is handled separately via handle_prompt_streaming
        "prompt" => Response::error("unreachable"),

        _ => Response::error(format!("Method not found: {}", request.method)),
    }
}

// ── File transfer handlers ──────────────────────────────────────────────────

/// Handle an incoming file from a peer (file.send).
///
/// The request frame contains `{ name, size }`. After the request frame,
/// the remaining bytes on the recv stream are the raw file data.
async fn handle_file_send(
    request: &Request,
    state: &ServerState,
    recv: iroh::endpoint::RecvStream,
    mut send: iroh::endpoint::SendStream,
) {
    match handle_file_send_inner(request, state, recv).await {
        Ok((path, size)) => {
            let resp = Response::success(json!({
                "path": path.display().to_string(),
                "size": size,
            }));
            let _ = write_frame(&mut send, &serde_json::to_vec(&resp).unwrap_or_default()).await;
            let _ = send.finish();
        }
        Err(e) => {
            let resp = Response::error(e);
            let _ = write_frame(&mut send, &serde_json::to_vec(&resp).unwrap_or_default()).await;
            let _ = send.finish();
        }
    }
}

/// Inner logic for receiving a file. Returns (path, bytes_received) or error message.
async fn handle_file_send_inner(
    request: &Request,
    state: &ServerState,
    mut recv: iroh::endpoint::RecvStream,
) -> Result<(PathBuf, u64), String> {
    let file_name = request.params.get("name").and_then(|v| v.as_str()).unwrap_or("unnamed");
    let expected_size = request.params.get("size").and_then(|v| v.as_u64()).unwrap_or(0);

    // Sanitize filename — strip path separators
    let safe_name: String = file_name.replace(['/', '\\'], "_").replace("..", "_");

    let receive_dir = state.receive_dir.clone().unwrap_or_else(|| PathBuf::from("/tmp/clankers-received"));

    std::fs::create_dir_all(&receive_dir).map_err(|e| format!("Cannot create dir: {}", e))?;

    let dest = receive_dir.join(&safe_name);
    let mut file = tokio::fs::File::create(&dest).await.map_err(|e| format!("Cannot create file: {}", e))?;

    let total = stream_to_file(&mut recv, &mut file).await?;

    info!("Received file '{}' ({} bytes, expected {})", dest.display(), total, expected_size);

    Ok((dest, total))
}

/// Stream data from recv into a file. Returns total bytes written.
async fn stream_to_file(recv: &mut iroh::endpoint::RecvStream, file: &mut tokio::fs::File) -> Result<u64, String> {
    let mut total = 0u64;
    let mut buf = vec![0u8; 64 * 1024];

    loop {
        let n = match recv.read(&mut buf).await {
            Ok(Some(n)) => n,
            Ok(None) => break,
            Err(e) => {
                warn!("File receive stream error: {}", e);
                return Err(format!("Stream error: {}", e));
            }
        };
        tokio::io::AsyncWriteExt::write_all(file, &buf[..n])
            .await
            .map_err(|e| format!("Write error: {}", e))?;
        total += n as u64;
    }

    Ok(total)
}

/// Handle a file download request from a peer (file.recv).
///
/// Reads the file from disk, sends a header response with size, then
/// streams the raw bytes.
async fn handle_file_recv(request: &Request, _recv: iroh::endpoint::RecvStream, mut send: iroh::endpoint::SendStream) {
    let file_path = match extract_file_path(request) {
        Ok(path) => path,
        Err(resp) => {
            let _ = write_frame(&mut send, &serde_json::to_vec(&resp).unwrap_or_default()).await;
            let _ = send.finish();
            return;
        }
    };

    let (file_name, file_size) = match get_file_metadata(&file_path) {
        Ok(info) => info,
        Err(resp) => {
            let _ = write_frame(&mut send, &serde_json::to_vec(&resp).unwrap_or_default()).await;
            let _ = send.finish();
            return;
        }
    };

    // Send header response with file metadata
    let header = Response::success(json!({
        "name": file_name,
        "size": file_size,
    }));
    if write_frame(&mut send, &serde_json::to_vec(&header).unwrap_or_default()).await.is_err() {
        return;
    }

    // Stream the file data
    if stream_file_to_send(&file_path, &mut send).await.is_ok() {
        info!("Sent file '{}' ({} bytes)", file_path.display(), file_size);
    }
    let _ = send.finish();
}

/// Extract file path from request parameters.
fn extract_file_path(request: &Request) -> Result<PathBuf, Response> {
    match request.params.get("path").and_then(|v| v.as_str()) {
        Some(p) => Ok(PathBuf::from(p)),
        None => Err(Response::error("Missing required param: \"path\"")),
    }
}

/// Get file metadata (name and size).
fn get_file_metadata(file_path: &Path) -> Result<(String, u64), Response> {
    let metadata = std::fs::metadata(file_path).map_err(|e| Response::error(format!("Cannot stat file: {}", e)))?;

    let file_name = file_path.file_name().and_then(|n| n.to_str()).unwrap_or("unnamed").to_string();

    Ok((file_name, metadata.len()))
}

/// Stream a file's contents to the send stream.
async fn stream_file_to_send(file_path: &Path, send: &mut iroh::endpoint::SendStream) -> Result<(), ()> {
    let mut file = tokio::fs::File::open(file_path).await.map_err(|e| {
        warn!("Cannot open file for sending: {}", e);
    })?;

    let mut buf = vec![0u8; 64 * 1024];
    loop {
        let n = match tokio::io::AsyncReadExt::read(&mut file, &mut buf).await {
            Ok(0) => break,
            Ok(n) => n,
            Err(e) => {
                warn!("File read error during send: {}", e);
                return Err(());
            }
        };
        send.write_all(&buf[..n]).await.map_err(|_| ())?;
    }

    Ok(())
}

// ── Streaming prompt handler ────────────────────────────────────────────────

/// Handle a prompt request with streaming notifications over the QUIC stream.
/// Sends text_delta / tool_call / tool_result notifications as they happen,
/// then sends the final Response.
async fn handle_prompt_streaming(request: &Request, state: &ServerState, send: iroh::endpoint::SendStream) {
    handle_prompt_streaming_pub(request, state, send).await;
}

/// Public wrapper for `handle_prompt_streaming` — used by the daemon module.
pub async fn handle_prompt_streaming_pub(request: &Request, state: &ServerState, mut send: iroh::endpoint::SendStream) {
    // Validate and extract agent context
    let ctx = match validate_agent_context(state, &mut send).await {
        Some(c) => c,
        None => return,
    };

    // Extract and validate request parameters
    let (text, model, system_prompt) = match extract_prompt_params(request, ctx, &mut send).await {
        Some(params) => params,
        None => return,
    };

    // Create agent and set up event streaming
    let mut agent = crate::agent::builder::AgentBuilder::new(
        Arc::clone(&ctx.provider),
        ctx.settings.clone(),
        model,
        system_prompt,
    )
    .with_tools(ctx.tools.clone())
    .build();
    let rx = agent.subscribe();

    // Stream events to the QUIC send stream in a background task
    let streamer = spawn_event_streamer(rx, send);

    // Run the agent
    let agent_result = agent.prompt(&text).await;

    // Wait for streamer to finish and send final response
    send_final_response(streamer, agent_result).await;
}

/// Validate that the server has agent capabilities and send error if not.
async fn validate_agent_context<'a>(
    state: &'a ServerState,
    send: &mut iroh::endpoint::SendStream,
) -> Option<&'a super::RpcContext> {
    match &state.agent {
        Some(c) => Some(c),
        None => {
            let resp = Response::error("This server was not started with agent capabilities");
            let _ = write_frame(send, &serde_json::to_vec(&resp).unwrap_or_default()).await;
            let _ = send.finish();
            None
        }
    }
}

/// Extract and validate prompt parameters from the request.
async fn extract_prompt_params(
    request: &Request,
    ctx: &super::RpcContext,
    send: &mut iroh::endpoint::SendStream,
) -> Option<(String, String, String)> {
    let text = match request.params.get("text").and_then(|v| v.as_str()) {
        Some(t) => t.to_string(),
        None => {
            let resp = Response::error("Missing required param: \"text\"");
            let _ = write_frame(send, &serde_json::to_vec(&resp).unwrap_or_default()).await;
            let _ = send.finish();
            return None;
        }
    };

    let model = request
        .params
        .get("model")
        .and_then(|v| v.as_str())
        .map(String::from)
        .unwrap_or_else(|| ctx.model.clone());

    let system_prompt = request
        .params
        .get("system_prompt")
        .and_then(|v| v.as_str())
        .map(String::from)
        .unwrap_or_else(|| ctx.system_prompt.clone());

    Some((text, model, system_prompt))
}

/// Spawn a task that streams agent events to the QUIC send stream.
/// Returns a join handle that resolves to (send stream, collected text).
fn spawn_event_streamer(
    mut rx: tokio::sync::broadcast::Receiver<AgentEvent>,
    mut send: iroh::endpoint::SendStream,
) -> tokio::task::JoinHandle<(iroh::endpoint::SendStream, String)> {
    tokio::spawn(async move {
        let mut collected = String::new();

        while let Ok(event) = rx.recv().await {
            match event {
                AgentEvent::MessageUpdate {
                    delta: ContentDelta::TextDelta { ref text },
                    ..
                } => {
                    collected.push_str(text);
                    let notification = json!({
                        "type": "text_delta",
                        "text": text,
                    });
                    let bytes = serde_json::to_vec(&notification).unwrap_or_default();
                    if write_frame(&mut send, &bytes).await.is_err() {
                        break;
                    }
                }
                AgentEvent::ToolCall {
                    ref tool_name,
                    ref call_id,
                    ref input,
                } => {
                    let notification = json!({
                        "type": "tool_call",
                        "tool_name": tool_name,
                        "call_id": call_id,
                        "input": input,
                    });
                    let bytes = serde_json::to_vec(&notification).unwrap_or_default();
                    let _ = write_frame(&mut send, &bytes).await;
                }
                AgentEvent::ToolExecutionEnd {
                    ref call_id,
                    ref result,
                    is_error,
                } => {
                    let notification = json!({
                        "type": "tool_result",
                        "call_id": call_id,
                        "content": format!("{:?}", result),
                        "is_error": is_error,
                    });
                    let bytes = serde_json::to_vec(&notification).unwrap_or_default();
                    let _ = write_frame(&mut send, &bytes).await;
                }
                AgentEvent::AgentEnd { .. } => break,
                _ => {}
            }
        }

        (send, collected)
    })
}

/// Wait for the event streamer to finish and send the final response.
async fn send_final_response(
    streamer: tokio::task::JoinHandle<(iroh::endpoint::SendStream, String)>,
    agent_result: Result<(), crate::error::Error>,
) {
    let (mut send, collected_text) = match streamer.await {
        Ok(result) => result,
        Err(_) => return,
    };

    let response = match agent_result {
        Ok(()) => Response::success(json!({
            "text": collected_text,
            "status": "complete"
        })),
        Err(e) => Response::error(format!("Agent error: {}", e)),
    };
    let _ = write_frame(&mut send, &serde_json::to_vec(&response).unwrap_or_default()).await;
    let _ = send.finish();
}
