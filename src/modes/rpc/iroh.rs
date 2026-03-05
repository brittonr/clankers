//! Iroh P2P communication
//!
//! Peer-to-peer agent communication over iroh QUIC.
//! ALPN: b"clankers/rpc/1"
//!
//! ## Wire protocol
//!
//! Each bidirectional QUIC stream carries one request/response exchange.
//! All frames are length-prefixed JSON: `[4-byte big-endian length][JSON payload]`.
//!
//! Request:  `{ "method": "ping", "params": { ... } }`
//! Response: `{ "ok": <value> }` or `{ "error": "message" }`
//!
//! For streaming methods (prompt), intermediate notification frames
//! (no `ok`/`error` key) are sent before the final response.
//!
//! For file transfer, raw bytes follow the framed request/response.
//!
//! ## Auth
//!
//! The server maintains an allowlist of peer public keys. Connections from
//! unknown peers are rejected at the stream level. Use `--allow-all` to
//! disable the check, or `clankers rpc allow <node-id>` to add peers.
//!
//! ## Discovery
//!
//! The endpoint is configured with mDNS (LAN auto-discovery) and DNS pkarr
//! (WAN discovery via relay servers). Peers on the same LAN can find each
//! other without manual `peers add`. Use `clankers rpc discover --mdns` to scan
//! the local network for clankers instances.

use std::collections::HashSet;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;

use iroh::Endpoint;
use iroh::EndpointAddr;
use iroh::PublicKey;
use iroh::SecretKey;
use serde_json::json;
use tracing::debug;
use tracing::info;
use tracing::warn;

use super::protocol::Request;
use super::protocol::Response;
use crate::agent::Agent;
use crate::agent::events::AgentEvent;
use crate::provider::Provider;
use crate::provider::streaming::ContentDelta;
use crate::tools::Tool;

pub const ALPN: &[u8] = b"clankers/rpc/1";

/// mDNS service name for clankers auto-discovery on LAN
const MDNS_SERVICE_NAME: &str = "_clankers._udp.local.";

// ── Server types ────────────────────────────────────────────────────────────

/// Metadata about this node, always available.
pub struct NodeMeta {
    pub tags: Vec<String>,
    pub agent_names: Vec<String>,
}

/// Context for handling RPC requests that need agent capabilities.
pub struct RpcContext {
    pub provider: Arc<dyn Provider>,
    pub tools: Vec<Arc<dyn Tool>>,
    pub settings: crate::config::settings::Settings,
    pub model: String,
    pub system_prompt: String,
}

/// Combined server state.
pub struct ServerState {
    pub meta: NodeMeta,
    pub agent: Option<RpcContext>,
    pub acl: AccessControl,
    /// Directory where received files are stored
    pub receive_dir: Option<PathBuf>,
}

/// Access control for incoming connections.
pub struct AccessControl {
    /// If true, accept all peers (no allowlist check).
    pub allow_all: bool,
    /// Set of hex-encoded public keys that are allowed to connect.
    pub allowed: HashSet<String>,
}

impl AccessControl {
    pub fn open() -> Self {
        Self {
            allow_all: true,
            allowed: HashSet::new(),
        }
    }

    pub fn from_allowlist(allowed: HashSet<String>) -> Self {
        Self {
            allow_all: false,
            allowed,
        }
    }

    pub fn is_allowed(&self, peer: &PublicKey) -> bool {
        self.allow_all || self.allowed.contains(&peer.to_string())
    }
}

// ── Allowlist persistence ───────────────────────────────────────────────────

pub fn allowlist_path(paths: &crate::config::ClankersPaths) -> PathBuf {
    paths.global_config_dir.join("allowed_peers.json")
}

/// Load the allowlist from disk.
pub fn load_allowlist(path: &Path) -> HashSet<String> {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|s| serde_json::from_str::<Vec<String>>(&s).ok())
        .map(|v| v.into_iter().collect())
        .unwrap_or_default()
}

/// Save the allowlist to disk.
pub fn save_allowlist(path: &Path, allowed: &HashSet<String>) -> Result<(), std::io::Error> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let list: Vec<&String> = {
        let mut v: Vec<_> = allowed.iter().collect();
        v.sort();
        v
    };
    let json = serde_json::to_string_pretty(&list).map_err(std::io::Error::other)?;
    std::fs::write(path, json)
}

// ── Identity ────────────────────────────────────────────────────────────────

/// Persistent identity for this node.
pub struct Identity {
    pub secret_key: SecretKey,
    pub path: PathBuf,
}

impl Identity {
    pub fn load_or_generate(path: &Path) -> Self {
        let secret_key = if path.exists() {
            let bytes = std::fs::read(path).unwrap_or_default();
            if bytes.len() == 32 {
                let mut key_bytes = [0u8; 32];
                key_bytes.copy_from_slice(&bytes);
                SecretKey::from_bytes(&key_bytes)
            } else {
                let key = SecretKey::generate(&mut rand::rng());
                let _ = std::fs::create_dir_all(path.parent().unwrap_or(Path::new(".")));
                let _ = std::fs::write(path, key.to_bytes());
                key
            }
        } else {
            let key = SecretKey::generate(&mut rand::rng());
            let _ = std::fs::create_dir_all(path.parent().unwrap_or(Path::new(".")));
            let _ = std::fs::write(path, key.to_bytes());
            key
        };
        Self {
            secret_key,
            path: path.to_path_buf(),
        }
    }

    pub fn public_key(&self) -> PublicKey {
        self.secret_key.public()
    }
}

pub fn identity_path(paths: &crate::config::ClankersPaths) -> PathBuf {
    paths.global_config_dir.join("identity.key")
}

// ── Endpoint (with mDNS + DNS discovery) ────────────────────────────────────

/// Start an iroh endpoint with mDNS (LAN) and default DNS discovery.
///
/// The endpoint uses a shared QUIC socket that can both accept incoming
/// connections (server) and initiate outgoing connections (client), enabling
/// bidirectional communication through a single endpoint.
pub async fn start_endpoint(identity: &Identity) -> Result<Endpoint, crate::error::Error> {
    let mdns = iroh::address_lookup::MdnsAddressLookup::builder().service_name(MDNS_SERVICE_NAME);

    let endpoint = Endpoint::builder()
        .secret_key(identity.secret_key.clone())
        .alpns(vec![ALPN.to_vec()])
        .address_lookup(mdns)
        .bind()
        .await
        .map_err(|e| crate::error::Error::Provider {
            message: format!("Failed to bind iroh endpoint: {}", e),
        })?;
    Ok(endpoint)
}

/// Start an endpoint without mDNS (for tests or minimal usage).
pub async fn start_endpoint_no_mdns(identity: &Identity) -> Result<Endpoint, crate::error::Error> {
    let endpoint = Endpoint::builder()
        .secret_key(identity.secret_key.clone())
        .alpns(vec![ALPN.to_vec()])
        .clear_address_lookup()
        .bind()
        .await
        .map_err(|e| crate::error::Error::Provider {
            message: format!("Failed to bind iroh endpoint: {}", e),
        })?;
    Ok(endpoint)
}

// ── Client: send RPC ────────────────────────────────────────────────────────

/// Send an RPC request and return the single final response.
/// For streaming, use `send_rpc_streaming`.
pub async fn send_rpc(
    endpoint: &Endpoint,
    remote: impl Into<EndpointAddr>,
    request: &Request,
) -> Result<Response, crate::error::Error> {
    let (_, response) = send_rpc_streaming(endpoint, remote, request, |_| {}).await?;
    Ok(response)
}

/// Send an RPC request. Calls `on_notification` for each intermediate
/// notification frame, then returns the final response.
pub async fn send_rpc_streaming(
    endpoint: &Endpoint,
    remote: impl Into<EndpointAddr>,
    request: &Request,
    mut on_notification: impl FnMut(&serde_json::Value),
) -> Result<(Vec<serde_json::Value>, Response), crate::error::Error> {
    let addr: EndpointAddr = remote.into();
    let conn = endpoint.connect(addr, ALPN).await.map_err(|e| crate::error::Error::Provider {
        message: format!("Failed to connect to peer: {}", e),
    })?;

    let (mut send, mut recv) = conn.open_bi().await.map_err(|e| crate::error::Error::Provider {
        message: format!("Failed to open stream: {}", e),
    })?;

    // Send request
    write_frame(&mut send, &serde_json::to_vec(request).unwrap_or_default()).await?;
    send.finish().map_err(|e| crate::error::Error::Provider {
        message: format!("Failed to finish send: {}", e),
    })?;

    // Read frames until we get a final response (has "ok" or "error" field).
    // Intermediate frames without those fields are streaming notifications.
    let mut notifications = Vec::new();
    loop {
        let data = read_frame(&mut recv).await?;
        let value: serde_json::Value = serde_json::from_slice(&data).map_err(|e| crate::error::Error::Provider {
            message: format!("Failed to parse frame: {}", e),
        })?;

        if value.get("ok").is_some() || value.get("error").is_some() {
            let response: Response = serde_json::from_value(value).map_err(|e| crate::error::Error::Provider {
                message: format!("Failed to parse response: {}", e),
            })?;
            return Ok((notifications, response));
        }
        on_notification(&value);
        notifications.push(value);
    }
}

// ── Client: file transfer ───────────────────────────────────────────────────

/// Send a file to a remote peer.
///
/// Opens a bidirectional stream, sends a `file.send` request with metadata,
/// then streams the raw file bytes. The remote peer saves the file and
/// responds with the path where it was stored.
pub async fn send_file(
    endpoint: &Endpoint,
    remote: impl Into<EndpointAddr>,
    file_path: &Path,
) -> Result<Response, crate::error::Error> {
    let metadata = std::fs::metadata(file_path).map_err(|e| crate::error::Error::Provider {
        message: format!("Cannot stat file: {}", e),
    })?;
    let file_name = file_path.file_name().and_then(|n| n.to_str()).unwrap_or("unnamed").to_string();
    let file_size = metadata.len();

    let addr: EndpointAddr = remote.into();
    let conn = endpoint.connect(addr, ALPN).await.map_err(|e| crate::error::Error::Provider {
        message: format!("Failed to connect to peer: {}", e),
    })?;

    let (mut send, mut recv) = conn.open_bi().await.map_err(|e| crate::error::Error::Provider {
        message: format!("Failed to open stream: {}", e),
    })?;

    // Send the file.send request
    let request = Request::new("file.send", json!({ "name": file_name, "size": file_size }));
    write_frame(&mut send, &serde_json::to_vec(&request).unwrap_or_default()).await?;

    // Stream the file data in chunks
    let mut file = tokio::fs::File::open(file_path).await.map_err(|e| crate::error::Error::Provider {
        message: format!("Cannot open file: {}", e),
    })?;
    let mut buf = vec![0u8; 64 * 1024]; // 64KB chunks
    loop {
        let n =
            tokio::io::AsyncReadExt::read(&mut file, &mut buf)
                .await
                .map_err(|e| crate::error::Error::Provider {
                    message: format!("File read error: {}", e),
                })?;
        if n == 0 {
            break;
        }
        send.write_all(&buf[..n]).await.map_err(io_err)?;
    }
    send.finish().map_err(|e| crate::error::Error::Provider {
        message: format!("Failed to finish send: {}", e),
    })?;

    // Read the response
    let data = read_frame(&mut recv).await?;
    let response: Response = serde_json::from_slice(&data).map_err(|e| crate::error::Error::Provider {
        message: format!("Failed to parse response: {}", e),
    })?;
    Ok(response)
}

/// Request a file from a remote peer.
///
/// Sends a `file.recv` request. The server responds with a header frame
/// containing the file size, followed by the raw file bytes.
pub async fn recv_file(
    endpoint: &Endpoint,
    remote: impl Into<EndpointAddr>,
    remote_path: &str,
    local_path: &Path,
) -> Result<u64, crate::error::Error> {
    let addr: EndpointAddr = remote.into();
    let conn = endpoint.connect(addr, ALPN).await.map_err(|e| crate::error::Error::Provider {
        message: format!("Failed to connect to peer: {}", e),
    })?;

    let (mut send, mut recv) = conn.open_bi().await.map_err(|e| crate::error::Error::Provider {
        message: format!("Failed to open stream: {}", e),
    })?;

    // Send the file.recv request
    let request = Request::new("file.recv", json!({ "path": remote_path }));
    write_frame(&mut send, &serde_json::to_vec(&request).unwrap_or_default()).await?;
    send.finish().map_err(|e| crate::error::Error::Provider {
        message: format!("Failed to finish send: {}", e),
    })?;

    // Read the header response
    let data = read_frame(&mut recv).await?;
    let response: Response = serde_json::from_slice(&data).map_err(|e| crate::error::Error::Provider {
        message: format!("Failed to parse response: {}", e),
    })?;

    if let Some(err) = response.error {
        return Err(crate::error::Error::Provider {
            message: format!("Remote error: {}", err),
        });
    }

    let file_size = response.ok.as_ref().and_then(|r| r.get("size")).and_then(|v| v.as_u64()).unwrap_or(0);

    // Read the raw file data
    if let Some(parent) = local_path.parent() {
        tokio::fs::create_dir_all(parent).await.map_err(|e| crate::error::Error::Provider {
            message: format!("Cannot create directory: {}", e),
        })?;
    }

    let mut file = tokio::fs::File::create(local_path).await.map_err(|e| crate::error::Error::Provider {
        message: format!("Cannot create file: {}", e),
    })?;

    let mut total = 0u64;
    let mut buf = vec![0u8; 64 * 1024];
    loop {
        let n = match recv.read(&mut buf).await {
            Ok(Some(n)) => n,
            Ok(None) => break,
            Err(e) => {
                return Err(crate::error::Error::Provider {
                    message: format!("Stream read error: {}", e),
                });
            }
        };
        tokio::io::AsyncWriteExt::write_all(&mut file, &buf[..n])
            .await
            .map_err(|e| crate::error::Error::Provider {
                message: format!("File write error: {}", e),
            })?;
        total += n as u64;
    }

    // Flush to ensure all data is written to disk before returning.
    // Without this, the tokio::fs::File drop may not complete the write
    // synchronously, causing callers that immediately read the file to
    // see stale/empty content.
    tokio::io::AsyncWriteExt::shutdown(&mut file).await.map_err(|e| crate::error::Error::Provider {
        message: format!("File flush error: {}", e),
    })?;

    info!("Received file {} ({} bytes)", local_path.display(), total);
    if file_size > 0 && total != file_size {
        warn!("File size mismatch: expected {} bytes, got {} bytes", file_size, total);
    }

    Ok(total)
}

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
    mut recv: iroh::endpoint::RecvStream,
    mut send: iroh::endpoint::SendStream,
) {
    let file_name = request.params.get("name").and_then(|v| v.as_str()).unwrap_or("unnamed");
    let expected_size = request.params.get("size").and_then(|v| v.as_u64()).unwrap_or(0);

    // Sanitize filename — strip path separators
    let safe_name: String = file_name.replace(['/', '\\'], "_").replace("..", "_");

    let receive_dir = state.receive_dir.clone().unwrap_or_else(|| PathBuf::from("/tmp/clankers-received"));

    if let Err(e) = std::fs::create_dir_all(&receive_dir) {
        let resp = Response::error(format!("Cannot create dir: {}", e));
        let _ = write_frame(&mut send, &serde_json::to_vec(&resp).unwrap_or_default()).await;
        let _ = send.finish();
        return;
    }

    let dest = receive_dir.join(&safe_name);
    let mut file = match tokio::fs::File::create(&dest).await {
        Ok(f) => f,
        Err(e) => {
            let resp = Response::error(format!("Cannot create file: {}", e));
            let _ = write_frame(&mut send, &serde_json::to_vec(&resp).unwrap_or_default()).await;
            let _ = send.finish();
            return;
        }
    };

    let mut total = 0u64;
    let mut buf = vec![0u8; 64 * 1024];
    loop {
        let n = match recv.read(&mut buf).await {
            Ok(Some(n)) => n,
            Ok(None) => break,
            Err(e) => {
                warn!("File receive stream error: {}", e);
                break;
            }
        };
        if let Err(e) = tokio::io::AsyncWriteExt::write_all(&mut file, &buf[..n]).await {
            warn!("File write error: {}", e);
            let resp = Response::error(format!("Write error: {}", e));
            let _ = write_frame(&mut send, &serde_json::to_vec(&resp).unwrap_or_default()).await;
            let _ = send.finish();
            return;
        }
        total += n as u64;
    }

    info!("Received file '{}' ({} bytes, expected {})", dest.display(), total, expected_size);

    let resp = Response::success(json!({
        "path": dest.display().to_string(),
        "size": total,
    }));
    let _ = write_frame(&mut send, &serde_json::to_vec(&resp).unwrap_or_default()).await;
    let _ = send.finish();
}

/// Handle a file download request from a peer (file.recv).
///
/// Reads the file from disk, sends a header response with size, then
/// streams the raw bytes.
async fn handle_file_recv(request: &Request, _recv: iroh::endpoint::RecvStream, mut send: iroh::endpoint::SendStream) {
    let file_path = match request.params.get("path").and_then(|v| v.as_str()) {
        Some(p) => PathBuf::from(p),
        None => {
            let resp = Response::error("Missing required param: \"path\"");
            let _ = write_frame(&mut send, &serde_json::to_vec(&resp).unwrap_or_default()).await;
            let _ = send.finish();
            return;
        }
    };

    let metadata = match std::fs::metadata(&file_path) {
        Ok(m) => m,
        Err(e) => {
            let resp = Response::error(format!("Cannot stat file: {}", e));
            let _ = write_frame(&mut send, &serde_json::to_vec(&resp).unwrap_or_default()).await;
            let _ = send.finish();
            return;
        }
    };

    let file_name = file_path.file_name().and_then(|n| n.to_str()).unwrap_or("unnamed");

    // Send header response with file metadata
    let header = Response::success(json!({
        "name": file_name,
        "size": metadata.len(),
    }));
    if write_frame(&mut send, &serde_json::to_vec(&header).unwrap_or_default()).await.is_err() {
        return;
    }

    // Stream the file data
    let mut file = match tokio::fs::File::open(&file_path).await {
        Ok(f) => f,
        Err(e) => {
            warn!("Cannot open file for sending: {}", e);
            let _ = send.finish();
            return;
        }
    };

    let mut buf = vec![0u8; 64 * 1024];
    loop {
        let n = match tokio::io::AsyncReadExt::read(&mut file, &mut buf).await {
            Ok(0) => break,
            Ok(n) => n,
            Err(e) => {
                warn!("File read error during send: {}", e);
                break;
            }
        };
        if send.write_all(&buf[..n]).await.is_err() {
            break;
        }
    }

    let _ = send.finish();
    info!("Sent file '{}' ({} bytes)", file_path.display(), metadata.len());
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
    let ctx = match &state.agent {
        Some(c) => c,
        None => {
            let resp = Response::error("This server was not started with agent capabilities");
            let _ = write_frame(&mut send, &serde_json::to_vec(&resp).unwrap_or_default()).await;
            let _ = send.finish();
            return;
        }
    };

    let text = match request.params.get("text").and_then(|v| v.as_str()) {
        Some(t) => t.to_string(),
        None => {
            let resp = Response::error("Missing required param: \"text\"");
            let _ = write_frame(&mut send, &serde_json::to_vec(&resp).unwrap_or_default()).await;
            let _ = send.finish();
            return;
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

    let mut agent =
        Agent::new(Arc::clone(&ctx.provider), ctx.tools.clone(), ctx.settings.clone(), model, system_prompt);

    let mut rx = agent.subscribe();

    // Stream events to the QUIC send stream
    let streamer = tokio::spawn(async move {
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
    });

    // Run the agent
    let agent_result = agent.prompt(&text).await;

    // Wait for streamer to finish collecting
    let (mut send, collected_text) = match streamer.await {
        Ok(result) => result,
        Err(_) => return,
    };

    // Send the final response
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

// ── Peer health / heartbeat ─────────────────────────────────────────────────

/// Periodically probe all known peers and update the registry.
///
/// Runs in a background task. Probes each peer with a "status" RPC every
/// `interval`. Updates capabilities and last_seen on success, or marks
/// peers as stale after repeated failures.
pub async fn run_heartbeat(
    endpoint: Arc<Endpoint>,
    registry_path: PathBuf,
    interval: std::time::Duration,
    cancel: tokio_util::sync::CancellationToken,
) {
    info!("Heartbeat started (interval: {:?})", interval);
    loop {
        tokio::select! {
            () = tokio::time::sleep(interval) => {}
            () = cancel.cancelled() => {
                info!("Heartbeat stopped");
                return;
            }
        }

        let mut registry = super::peers::PeerRegistry::load(&registry_path);
        let peer_ids: Vec<String> = registry.peers.keys().cloned().collect();

        if peer_ids.is_empty() {
            continue;
        }

        debug!("Heartbeat: probing {} peer(s)", peer_ids.len());

        for node_id in &peer_ids {
            let remote: PublicKey = match node_id.parse() {
                Ok(pk) => pk,
                Err(_) => continue,
            };

            let request = Request::new("status", json!({}));
            match tokio::time::timeout(std::time::Duration::from_secs(10), send_rpc(&endpoint, remote, &request)).await
            {
                Ok(Ok(response)) => {
                    if let Some(result) = response.ok {
                        let caps = super::peers::PeerCapabilities {
                            accepts_prompts: result.get("accepts_prompts").and_then(|v| v.as_bool()).unwrap_or(false),
                            agents: result
                                .get("agents")
                                .and_then(|v| v.as_array())
                                .map(|a| a.iter().filter_map(|v| v.as_str().map(String::from)).collect())
                                .unwrap_or_default(),
                            tools: result
                                .get("tools")
                                .and_then(|v| v.as_array())
                                .map(|a| a.iter().filter_map(|v| v.as_str().map(String::from)).collect())
                                .unwrap_or_default(),
                            tags: result
                                .get("tags")
                                .and_then(|v| v.as_array())
                                .map(|a| a.iter().filter_map(|v| v.as_str().map(String::from)).collect())
                                .unwrap_or_default(),
                            version: result.get("version").and_then(|v| v.as_str()).map(String::from),
                        };
                        registry.update_capabilities(node_id, caps);
                        debug!("Heartbeat: {} online", &node_id[..12.min(node_id.len())]);
                    } else {
                        registry.touch(node_id);
                    }
                }
                _ => {
                    debug!("Heartbeat: {} unreachable", &node_id[..12.min(node_id.len())]);
                    // Don't remove — just leave last_seen stale
                }
            }
        }

        if let Err(e) = registry.save(&registry_path) {
            warn!("Heartbeat: failed to save registry: {}", e);
        }
    }
}

// ── mDNS LAN discovery ─────────────────────────────────────────────────────

/// Scan the local network for clankers peers via mDNS.
///
/// Creates a dedicated mDNS listener, subscribes to discovery events for
/// `duration`, and returns discovered peer endpoint IDs. These can then be
/// probed with "status" to get capabilities before adding to the registry.
pub async fn discover_mdns_peers(
    endpoint: &Endpoint,
    duration: std::time::Duration,
) -> Vec<(iroh::EndpointId, Option<iroh::address_lookup::EndpointInfo>)> {
    use futures::StreamExt;
    use iroh::address_lookup::mdns::DiscoveryEvent;
    use iroh::address_lookup::mdns::MdnsAddressLookup;

    info!("Scanning LAN for clankers peers via mDNS ({:?})...", duration);

    // Build a dedicated mDNS instance for subscribing to events.
    // The builder needs the endpoint ID to filter out self-announcements.
    let mdns = match MdnsAddressLookup::builder().service_name(MDNS_SERVICE_NAME).build(endpoint.id()) {
        Ok(m) => m,
        Err(e) => {
            warn!("Failed to create mDNS scanner: {}", e);
            return Vec::new();
        }
    };

    let mut stream = mdns.subscribe().await;
    let mut discovered = Vec::new();

    let deadline = tokio::time::sleep(duration);
    tokio::pin!(deadline);

    loop {
        tokio::select! {
            () = &mut deadline => break,
            event = stream.next() => {
                match event {
                    Some(DiscoveryEvent::Discovered { endpoint_info, .. }) => {
                        let eid = endpoint_info.endpoint_id;
                        // Skip ourselves
                        if eid == endpoint.id() {
                            continue;
                        }
                        info!("mDNS: discovered peer {}", eid.fmt_short());
                        if !discovered.iter().any(|(id, _): &(iroh::EndpointId, _)| *id == eid) {
                            discovered.push((eid, Some(endpoint_info)));
                        }
                    }
                    Some(DiscoveryEvent::Expired { .. }) => {} // peer went offline
                    None => break,
                }
            }
        }
    }

    info!("mDNS scan complete: {} peer(s) found", discovered.len());
    discovered
}

// ── Frame I/O helpers ───────────────────────────────────────────────────────

pub(crate) async fn write_frame(send: &mut iroh::endpoint::SendStream, data: &[u8]) -> Result<(), crate::error::Error> {
    let len = (data.len() as u32).to_be_bytes();
    send.write_all(&len).await.map_err(io_err)?;
    send.write_all(data).await.map_err(io_err)?;
    Ok(())
}

pub(crate) async fn read_frame(recv: &mut iroh::endpoint::RecvStream) -> Result<Vec<u8>, crate::error::Error> {
    let mut len_buf = [0u8; 4];
    recv.read_exact(&mut len_buf).await.map_err(io_err)?;
    let len = u32::from_be_bytes(len_buf) as usize;
    if len > 10_000_000 {
        return Err(crate::error::Error::Provider {
            message: "Frame too large".to_string(),
        });
    }
    let mut data = vec![0u8; len];
    recv.read_exact(&mut data).await.map_err(io_err)?;
    Ok(data)
}

fn io_err(e: impl std::fmt::Display) -> crate::error::Error {
    crate::error::Error::Provider {
        message: format!("IO error: {}", e),
    }
}
