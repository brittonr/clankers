//! Daemon mode — headless agent that listens on iroh and Matrix.
//!
//! Runs as a long-lived background process. Incoming messages from either
//! transport are routed to per-sender agent sessions. Responses are sent
//! back through the originating channel.
//!
//! ## Transport: iroh
//!
//! Uses ALPN negotiation on the iroh QUIC endpoint:
//! - `clankers/rpc/1` — existing JSON-RPC protocol (ping, status, prompt, file)
//! - `clankers/chat/1` — conversational channel with persistent sessions
//!
//! ## Transport: Matrix
//!
//! Listens for `ClankersEvent::Text` (human messages) and `ClankersEvent::Request`
//! in joined rooms. Responses are sent back as `matrix_send`.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use chrono::Utc;
use serde_json::json;
use tokio::sync::Mutex;
use tokio::sync::RwLock;
use tokio_util::sync::CancellationToken;
use tracing::error;
use tracing::info;
use tracing::warn;

use crate::agent::Agent;
use crate::agent::events::AgentEvent;
use crate::config::ClankersPaths;
use crate::config::settings::Settings;
use crate::error::Result;
use crate::modes::rpc::iroh;
use crate::modes::rpc::iroh::write_frame;
use crate::modes::rpc::protocol::Request;
use crate::modes::rpc::protocol::Response;
use crate::provider::Provider;
use crate::provider::streaming::ContentDelta;
use crate::session::SessionManager;
use crate::tools::Tool;

/// Chat ALPN — conversational sessions with persistent memory.
pub const ALPN_CHAT: &[u8] = b"clankers/chat/1";

// ── Configuration ───────────────────────────────────────────────────────────

/// Daemon configuration.
#[derive(Debug, Clone)]
pub struct DaemonConfig {
    /// Model to use
    pub model: String,
    /// System prompt
    pub system_prompt: String,
    /// Settings
    pub settings: Settings,
    /// Capability tags for announcements
    pub tags: Vec<String>,
    /// Allow all iroh peers (no ACL)
    pub allow_all: bool,
    /// Enable Matrix bridge
    pub enable_matrix: bool,
    /// Heartbeat interval (0 = disabled)
    pub heartbeat_secs: u64,
    /// Maximum concurrent sessions
    pub max_sessions: usize,
    /// Idle session timeout in seconds (0 = disabled)
    pub idle_timeout_secs: u64,
    /// Matrix user allowlist (empty = allow all). Overridden by
    /// `CLANKERS_MATRIX_ALLOWED_USERS` env var (comma-separated).
    pub matrix_allowed_users: Vec<String>,
}

impl Default for DaemonConfig {
    fn default() -> Self {
        Self {
            model: "claude-sonnet-4-5".to_string(),
            system_prompt: crate::agent::system_prompt::default_system_prompt().to_string(),
            settings: Settings::default(),
            tags: Vec::new(),
            allow_all: false,
            enable_matrix: false,
            heartbeat_secs: 60,
            max_sessions: 32,
            idle_timeout_secs: 1800, // 30 minutes
            matrix_allowed_users: Vec::new(),
        }
    }
}

// ── Session tracking ────────────────────────────────────────────────────────

/// A live agent session for a specific sender.
struct LiveSession {
    /// The agent with conversation history
    agent: Agent,
    /// Session persistence manager
    #[allow(dead_code)]
    session_mgr: Option<SessionManager>,
    /// When the session was last active
    last_active: chrono::DateTime<Utc>,
    /// Number of turns so far
    turn_count: usize,
}

/// Identifies a session by transport + sender.
#[derive(Debug, Clone, Hash, Eq, PartialEq)]
enum SessionKey {
    /// iroh peer identified by public key
    Iroh(String),
    /// Matrix user in a room
    Matrix { user_id: String, room_id: String },
}

impl std::fmt::Display for SessionKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Iroh(id) => write!(f, "iroh:{}", &id[..12.min(id.len())]),
            Self::Matrix { user_id, room_id } => write!(f, "matrix:{}@{}", user_id, room_id),
        }
    }
}

/// Manages all active sessions.
struct SessionStore {
    sessions: HashMap<SessionKey, LiveSession>,
    /// Per-session locks to serialize prompt execution and prevent
    /// concurrent prompts from racing on conversation history.
    prompt_locks: HashMap<SessionKey, Arc<Mutex<()>>>,
    max_sessions: usize,
    /// Shared resources for creating new agents
    provider: Arc<dyn Provider>,
    tools: Vec<Arc<dyn Tool>>,
    settings: Settings,
    model: String,
    system_prompt: String,
    sessions_dir: PathBuf,
}

impl SessionStore {
    fn new(
        provider: Arc<dyn Provider>,
        tools: Vec<Arc<dyn Tool>>,
        settings: Settings,
        model: String,
        system_prompt: String,
        sessions_dir: PathBuf,
        max_sessions: usize,
    ) -> Self {
        Self {
            sessions: HashMap::new(),
            prompt_locks: HashMap::new(),
            max_sessions,
            provider,
            tools,
            settings,
            model,
            system_prompt,
            sessions_dir,
        }
    }

    /// Get or create a per-session lock for serializing prompt execution.
    fn prompt_lock(&mut self, key: &SessionKey) -> Arc<Mutex<()>> {
        self.prompt_locks.entry(key.clone()).or_insert_with(|| Arc::new(Mutex::new(()))).clone()
    }

    /// Get or create a session for the given key.
    fn get_or_create(&mut self, key: &SessionKey) -> &mut LiveSession {
        if !self.sessions.contains_key(key) {
            // Evict oldest session if at capacity
            if self.sessions.len() >= self.max_sessions {
                self.evict_oldest();
            }

            let agent = Agent::new(
                Arc::clone(&self.provider),
                self.tools.clone(),
                self.settings.clone(),
                self.model.clone(),
                self.system_prompt.clone(),
            );

            let session_mgr =
                SessionManager::create(&self.sessions_dir, "daemon", &self.model, Some("daemon"), None, None).ok();

            info!("Created new session for {}", key);

            self.sessions.insert(key.clone(), LiveSession {
                agent,
                session_mgr,
                last_active: Utc::now(),
                turn_count: 0,
            });
        }

        let session = self.sessions.get_mut(key).expect("just inserted");
        session.last_active = Utc::now();
        session
    }

    /// Remove the least recently used session.
    fn evict_oldest(&mut self) {
        if let Some(oldest_key) = self.sessions.iter().min_by_key(|(_, s)| s.last_active).map(|(k, _)| k.clone()) {
            info!("Evicting stale session: {}", oldest_key);
            self.sessions.remove(&oldest_key);
            self.prompt_locks.remove(&oldest_key);
        }
    }

    /// Number of active sessions.
    fn len(&self) -> usize {
        self.sessions.len()
    }

    /// Reap sessions that have been idle longer than `max_idle`.
    /// Returns the number of reaped sessions.
    fn reap_idle(&mut self, max_idle: std::time::Duration) -> usize {
        let now = Utc::now();
        let stale: Vec<SessionKey> = self
            .sessions
            .iter()
            .filter(|(_, s)| {
                let idle = now.signed_duration_since(s.last_active);
                idle.to_std().unwrap_or_default() > max_idle
            })
            .map(|(k, _)| k.clone())
            .collect();

        let count = stale.len();
        for key in &stale {
            info!("Reaping idle session: {}", key);
            self.sessions.remove(key);
            self.prompt_locks.remove(key);
        }
        count
    }
}

// ── Daemon entry point ──────────────────────────────────────────────────────

/// Start the daemon. Blocks until cancelled.
pub async fn run_daemon(
    provider: Arc<dyn Provider>,
    tools: Vec<Arc<dyn Tool>>,
    config: DaemonConfig,
    paths: &ClankersPaths,
) -> Result<()> {
    let cancel = CancellationToken::new();

    // Build the session store
    let store = Arc::new(RwLock::new(SessionStore::new(
        Arc::clone(&provider),
        tools.clone(),
        config.settings.clone(),
        config.model.clone(),
        config.system_prompt.clone(),
        paths.global_sessions_dir.clone(),
        config.max_sessions,
    )));

    // ── iroh endpoint ───────────────────────────────────────────────
    let identity_path = iroh::identity_path(paths);
    let identity = iroh::Identity::load_or_generate(&identity_path);
    let node_id = identity.public_key();

    // Build endpoint that accepts both ALPNs
    let mdns_service = ::iroh::address_lookup::MdnsAddressLookup::builder().service_name("_clankers._udp.local.");

    let endpoint = ::iroh::Endpoint::builder()
        .secret_key(identity.secret_key.clone())
        .alpns(vec![iroh::ALPN.to_vec(), ALPN_CHAT.to_vec()])
        .address_lookup(mdns_service)
        .bind()
        .await
        .map_err(|e| crate::error::Error::Provider {
            message: format!("Failed to bind iroh endpoint: {e}"),
        })?;

    // Build ACL
    let acl_path = iroh::allowlist_path(paths);
    let acl = if config.allow_all {
        iroh::AccessControl::open()
    } else {
        let allowed = iroh::load_allowlist(&acl_path);
        iroh::AccessControl::from_allowlist(allowed)
    };
    let acl = Arc::new(acl);

    println!("clankers daemon started");
    println!("  Node ID:  {}", node_id);
    println!("  Auth:     {}", if config.allow_all { "open" } else { "allowlist" });
    println!("  Model:    {}", config.model);
    println!("  Sessions: 0/{}", config.max_sessions);
    if !config.tags.is_empty() {
        println!("  Tags:     {}", config.tags.join(", "));
    }

    // ── iroh accept loop ────────────────────────────────────────────
    let iroh_store = Arc::clone(&store);
    let iroh_acl = Arc::clone(&acl);
    let iroh_cancel = cancel.clone();

    // Also build the legacy RPC state for the rpc/1 ALPN
    let rpc_state = Arc::new(iroh::ServerState {
        meta: iroh::NodeMeta {
            tags: config.tags.clone(),
            agent_names: Vec::new(),
        },
        agent: Some(iroh::RpcContext {
            provider: Arc::clone(&provider),
            tools: tools.clone(),
            settings: config.settings.clone(),
            model: config.model.clone(),
            system_prompt: config.system_prompt.clone(),
        }),
        acl: if config.allow_all {
            iroh::AccessControl::open()
        } else {
            let allowed = iroh::load_allowlist(&acl_path);
            iroh::AccessControl::from_allowlist(allowed)
        },
        receive_dir: Some(paths.global_config_dir.join("received")),
    });

    let iroh_endpoint = endpoint.clone();
    let iroh_handle = tokio::spawn(async move {
        info!("iroh accept loop started");
        loop {
            tokio::select! {
                incoming = iroh_endpoint.accept() => {
                    let Some(incoming) = incoming else { break };
                    let store = Arc::clone(&iroh_store);
                    let acl = Arc::clone(&iroh_acl);
                    let rpc_state = Arc::clone(&rpc_state);

                    tokio::spawn(async move {
                        if let Err(e) = handle_iroh_connection(incoming, store, acl, rpc_state).await {
                            warn!("iroh connection error: {e}");
                        }
                    });
                }
                _ = iroh_cancel.cancelled() => break,
            }
        }
    });

    // ── Matrix bridge (optional) ────────────────────────────────────
    let matrix_handle = if config.enable_matrix {
        let matrix_store = Arc::clone(&store);
        let matrix_cancel = cancel.clone();
        let matrix_paths = paths.clone();
        let matrix_allowed = config.matrix_allowed_users.clone();
        Some(tokio::spawn(async move {
            if let Err(e) = run_matrix_bridge(matrix_store, matrix_cancel, &matrix_paths, matrix_allowed).await {
                error!("Matrix bridge error: {e}");
            }
        }))
    } else {
        None
    };

    // ── Heartbeat ───────────────────────────────────────────────────
    if config.heartbeat_secs > 0 {
        let registry_path = crate::modes::rpc::peers::registry_path(paths);
        let interval = std::time::Duration::from_secs(config.heartbeat_secs);
        let hb_endpoint = Arc::new(
            iroh::start_endpoint(&identity)
                .await
                .unwrap_or_else(|_| panic!("failed to start heartbeat endpoint")),
        );
        let hb_cancel = cancel.clone();
        tokio::spawn(iroh::run_heartbeat(hb_endpoint, registry_path, interval, hb_cancel));
    }

    // ── Status logging ──────────────────────────────────────────────
    let status_store = Arc::clone(&store);
    let status_cancel = cancel.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(300));
        loop {
            tokio::select! {
                _ = interval.tick() => {
                    let store = status_store.read().await;
                    info!("daemon status: {} active session(s)", store.len());
                }
                _ = status_cancel.cancelled() => break,
            }
        }
    });

    // ── Idle session reaper ─────────────────────────────────────────
    if config.idle_timeout_secs > 0 {
        let reaper_store = Arc::clone(&store);
        let reaper_cancel = cancel.clone();
        let idle_timeout = std::time::Duration::from_secs(config.idle_timeout_secs);
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(std::time::Duration::from_secs(60));
            loop {
                tokio::select! {
                    _ = interval.tick() => {
                        let reaped = reaper_store.write().await.reap_idle(idle_timeout);
                        if reaped > 0 {
                            info!("Reaped {} idle session(s)", reaped);
                        }
                    }
                    _ = reaper_cancel.cancelled() => break,
                }
            }
        });
    }

    println!("\nListening... (Ctrl+C to stop)\n");
    println!("Chat:  clankers rpc prompt {} \"hello\"", node_id);
    println!("Ping:  clankers rpc ping {}", node_id);

    // ── Wait for shutdown ───────────────────────────────────────────
    tokio::signal::ctrl_c().await.ok();
    println!("\nShutting down...");
    cancel.cancel();

    iroh_handle.await.ok();
    if let Some(h) = matrix_handle {
        h.await.ok();
    }

    let store = store.read().await;
    println!("Daemon stopped ({} sessions served).", store.len());
    Ok(())
}

// ── iroh connection handler ─────────────────────────────────────────────────

async fn handle_iroh_connection(
    incoming: ::iroh::endpoint::Incoming,
    store: Arc<RwLock<SessionStore>>,
    acl: Arc<iroh::AccessControl>,
    rpc_state: Arc<iroh::ServerState>,
) -> Result<()> {
    let conn = incoming.await.map_err(|e| crate::error::Error::Provider {
        message: format!("Connection failed: {e}"),
    })?;

    let remote = conn.remote_id();

    // Auth check
    if !acl.is_allowed(&remote) {
        warn!("Rejected unauthorized peer {}", remote.fmt_short());
        conn.close(1u32.into(), b"unauthorized");
        return Ok(());
    }

    let alpn = conn.alpn();
    info!("Connection from {} (ALPN: {:?})", remote.fmt_short(), String::from_utf8_lossy(alpn));

    match alpn.as_slice() {
        // ── chat/1: conversational sessions ─────────────────────────
        x if x == ALPN_CHAT => {
            handle_chat_connection(conn, store, &remote.to_string()).await;
        }

        // ── rpc/1: legacy protocol (delegated to existing handler) ──
        x if x == iroh::ALPN => {
            // Reuse the existing RPC server logic
            handle_rpc_v1_connection(conn, rpc_state).await;
        }

        _ => {
            warn!("Unknown ALPN: {:?}", String::from_utf8_lossy(alpn));
            conn.close(2u32.into(), b"unknown alpn");
        }
    }

    Ok(())
}

/// Handle a clankers/chat/1 connection: bidirectional conversational stream.
///
/// Wire format per stream (same framing as rpc/1):
///   Client → Server: `{ "text": "...", "session_hint": "..." }`
///   Server → Client: N × notification frames (text deltas, tool events)
///   Server → Client: 1 × final response frame
async fn handle_chat_connection(conn: ::iroh::endpoint::Connection, store: Arc<RwLock<SessionStore>>, peer_id: &str) {
    let key = SessionKey::Iroh(peer_id.to_string());

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

        let text = match request.get("text").and_then(|v| v.as_str()) {
            Some(t) => t.to_string(),
            None => continue,
        };

        info!("[{}] prompt: {}", key, &text[..80.min(text.len())]);

        // Run the prompt in the session
        let store_clone = Arc::clone(&store);
        let key_clone = key.clone();
        tokio::spawn(async move {
            run_session_prompt(store_clone, key_clone, text, send).await;
        });
    }
}

/// Handle a clankers/rpc/1 connection using the existing server code.
async fn handle_rpc_v1_connection(conn: ::iroh::endpoint::Connection, state: Arc<iroh::ServerState>) {
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
fn dispatch_rpc(request: &Request, state: &iroh::ServerState) -> Response {
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
async fn run_session_prompt(
    store: Arc<RwLock<SessionStore>>,
    key: SessionKey,
    text: String,
    mut send: ::iroh::endpoint::SendStream,
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
        let session = store.get_or_create(&key);
        session.turn_count += 1;
        // We can't move the agent out of the HashMap, so create a fresh
        // agent seeded with the session's conversation history. The
        // per-session prompt_lock above ensures only one prompt runs at
        // a time, preventing history from being overwritten by a
        // concurrent prompt.
        let messages = session.agent.messages().to_vec();
        let agent = Agent::new(
            Arc::clone(&store.provider),
            store.tools.clone(),
            store.settings.clone(),
            store.model.clone(),
            store.system_prompt.clone(),
        );
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

// ── Matrix bridge ───────────────────────────────────────────────────────────

/// Resolve the Matrix user allowlist from (in priority order):
/// 1. `CLANKERS_MATRIX_ALLOWED_USERS` env var (comma-separated)
/// 2. `allowed_users` from `matrix.json`
/// 3. `matrix_allowed_users` from `DaemonConfig`
///
/// Empty = allow all.
fn resolve_matrix_allowlist(
    matrix_config: &clankers_matrix::MatrixConfig,
    daemon_allowed: &[String],
) -> Vec<String> {
    if let Ok(env_val) = std::env::var("CLANKERS_MATRIX_ALLOWED_USERS") {
        let users: Vec<String> = env_val.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect();
        if !users.is_empty() {
            return users;
        }
    }
    if !matrix_config.allowed_users.is_empty() {
        return matrix_config.allowed_users.clone();
    }
    daemon_allowed.to_vec()
}

/// Check if a user is in the allowlist (empty = allow all).
fn is_user_allowed(allowlist: &[String], user_id: &str) -> bool {
    allowlist.is_empty() || allowlist.iter().any(|u| u == user_id)
}

async fn run_matrix_bridge(
    store: Arc<RwLock<SessionStore>>,
    cancel: CancellationToken,
    paths: &ClankersPaths,
    daemon_allowed_users: Vec<String>,
) -> Result<()> {
    use clankers_matrix::MatrixConfig;
    use clankers_matrix::bridge::BridgeEvent;
    use clankers_matrix::bridge::MatrixBridge;

    let config_path = paths.global_config_dir.join("matrix.json");
    let config = MatrixConfig::load(&config_path).ok_or_else(|| crate::error::Error::Config {
        message: format!("Matrix config not found at {}. Run `clankers matrix login` first.", config_path.display()),
    })?;

    // Resolve allowlist
    let allowlist = resolve_matrix_allowlist(&config, &daemon_allowed_users);
    if allowlist.is_empty() {
        info!("Matrix allowlist: open (all users accepted)");
    } else {
        info!("Matrix allowlist: {} user(s)", allowlist.len());
    }

    let store_path = config.resolve_store_path(&paths.global_config_dir);
    let mut client = clankers_matrix::MatrixClient::new(config, "clankers-daemon");

    client.restore_session(&store_path).await.map_err(|e| crate::error::Error::Provider {
        message: format!("Matrix session restore failed: {e}"),
    })?;

    client.start_sync().await.map_err(|e| crate::error::Error::Provider {
        message: format!("Matrix sync failed: {e}"),
    })?;

    let our_user_id = client.user_id().map(|u| u.to_string()).unwrap_or_default();
    let event_rx = client.subscribe();
    let client = Arc::new(tokio::sync::RwLock::new(client));

    let mut bridge = MatrixBridge::new();
    let mut agent_rx = bridge.take_event_rx().expect("first call");
    bridge.start(event_rx, &our_user_id);

    info!("Matrix bridge started for {}", our_user_id);

    loop {
        tokio::select! {
            event = agent_rx.recv() => {
                let Some(event) = event else { break };
                match event {
                    BridgeEvent::TextMessage { sender, body, room_id }
                    | BridgeEvent::ChatMessage { sender, body, room_id, .. } => {
                        // ── Allowlist check ─────────────────────────
                        if !is_user_allowed(&allowlist, &sender) {
                            info!("Matrix: denied message from {}", sender);
                            continue;
                        }

                        // ── Skip client slash commands ──────────────
                        if body.starts_with('/') {
                            continue;
                        }

                        // ── Bot command dispatch ────────────────────
                        if body.starts_with('!') {
                            let room_id_parsed = match clankers_matrix::ruma::RoomId::parse(&room_id) {
                                Ok(rid) => rid.to_owned(),
                                Err(_) => continue,
                            };
                            let key = SessionKey::Matrix {
                                user_id: sender.clone(),
                                room_id: room_id.clone(),
                            };
                            let response = handle_bot_command(
                                &body,
                                &key,
                                Arc::clone(&store),
                            ).await;
                            let c = client.read().await;
                            if let Err(e) = c.send_text(&room_id_parsed, &response).await {
                                error!("Matrix send failed: {e}");
                            }
                            continue;
                        }

                        let key = SessionKey::Matrix {
                            user_id: sender.clone(),
                            room_id: room_id.clone(),
                        };

                        info!("[{}] message: {}", key, &body[..80.min(body.len())]);

                        let room_id_parsed = match clankers_matrix::ruma::RoomId::parse(&room_id) {
                            Ok(rid) => rid.to_owned(),
                            Err(_) => continue,
                        };

                        // ── Typing indicator: start ─────────────────
                        {
                            let c = client.read().await;
                            if let Err(e) = c.set_typing(&room_id_parsed, true).await {
                                warn!("Typing indicator start failed: {e}");
                            }
                        }

                        // Spawn a typing refresh task (re-sends every 25s)
                        let typing_cancel = CancellationToken::new();
                        let typing_client = Arc::clone(&client);
                        let typing_room = room_id_parsed.clone();
                        let typing_token = typing_cancel.clone();
                        tokio::spawn(async move {
                            let mut interval = tokio::time::interval(std::time::Duration::from_secs(25));
                            interval.tick().await; // skip the immediate tick
                            loop {
                                tokio::select! {
                                    _ = interval.tick() => {
                                        let c = typing_client.read().await;
                                        let _ = c.set_typing(&typing_room, true).await;
                                    }
                                    _ = typing_token.cancelled() => break,
                                }
                            }
                        });

                        // ── Run prompt ──────────────────────────────
                        let mut response = run_matrix_prompt(
                            Arc::clone(&store), key, body,
                        ).await;

                        // ── Empty response re-prompt ────────────────
                        if response.trim().is_empty() {
                            info!("Empty response, re-prompting for summary");
                            let re_key = SessionKey::Matrix {
                                user_id: sender.clone(),
                                room_id: room_id.clone(),
                            };
                            let retry = run_matrix_prompt(
                                Arc::clone(&store),
                                re_key,
                                "You completed some actions but your response contained \
                                 no text. Briefly summarize what you did.".to_string(),
                            ).await;
                            if retry.trim().is_empty() {
                                response = "(completed actions — no summary available)".to_string();
                            } else {
                                response = retry;
                            }
                        }

                        // ── Typing indicator: stop ──────────────────
                        typing_cancel.cancel();
                        {
                            let c = client.read().await;
                            let _ = c.set_typing(&room_id_parsed, false).await;
                        }

                        // ── Send response ───────────────────────────
                        let c = client.read().await;
                        if let Err(e) = c.send_text(&room_id_parsed, &response).await {
                            error!("Matrix send failed: {e}");
                        }
                    }
                    BridgeEvent::PeerUpdate(peer) => {
                        info!("Matrix peer update: {} ({})", peer.instance_name, peer.user_id);
                    }
                    _ => {}
                }
            }
            _ = cancel.cancelled() => break,
        }
    }

    Ok(())
}

// ── Bot commands ────────────────────────────────────────────────────────────

/// Handle a `!command` from a Matrix user. Returns the response text.
async fn handle_bot_command(
    body: &str,
    key: &SessionKey,
    store: Arc<RwLock<SessionStore>>,
) -> String {
    let parts: Vec<&str> = body.trim().splitn(2, char::is_whitespace).collect();
    let command = parts[0].to_lowercase();
    let args = parts.get(1).unwrap_or(&"").trim();

    match command.as_str() {
        "!help" => {
            "**Available commands:**\n\
             • `!help` — Show this message\n\
             • `!status` — Session info (model, turns, uptime)\n\
             • `!restart` — Clear session history and start fresh\n\
             • `!compact` — Trigger context compaction\n\
             • `!model <name>` — Switch model for this session\n\
             • `!skills` — List loaded skills"
                .to_string()
        }
        "!status" => {
            let store = store.read().await;
            if let Some(session) = store.sessions.get(key) {
                let idle = Utc::now().signed_duration_since(session.last_active);
                let idle_secs = idle.num_seconds().max(0);
                let idle_str = if idle_secs < 60 {
                    format!("{}s", idle_secs)
                } else if idle_secs < 3600 {
                    format!("{}m{}s", idle_secs / 60, idle_secs % 60)
                } else {
                    format!("{}h{}m", idle_secs / 3600, (idle_secs % 3600) / 60)
                };
                format!(
                    "**Session status:**\n\
                     • Model: `{}`\n\
                     • Turns: {}\n\
                     • Idle: {}\n\
                     • Messages in context: {}",
                    store.model,
                    session.turn_count,
                    idle_str,
                    session.agent.messages().len(),
                )
            } else {
                "No active session. Send a message to start one.".to_string()
            }
        }
        "!restart" => {
            let mut store = store.write().await;
            store.sessions.remove(key);
            store.prompt_locks.remove(key);
            "Session cleared. Next message starts a fresh conversation.".to_string()
        }
        "!compact" => {
            // Trigger compaction by running a prompt that asks the agent to compact
            let response = run_matrix_prompt(
                Arc::clone(&store),
                key.clone(),
                "/compact".to_string(),
            )
            .await;
            if response.trim().is_empty() {
                "Context compacted.".to_string()
            } else {
                format!("Compaction result: {}", response)
            }
        }
        "!model" => {
            if args.is_empty() {
                let store = store.read().await;
                format!("Current model: `{}`. Usage: `!model <name>`", store.model)
            } else {
                let mut store = store.write().await;
                let old_model = store.model.clone();
                store.model = args.to_string();
                format!("Model switched: `{}` → `{}`", old_model, args)
            }
        }
        "!skills" => {
            let store = store.read().await;
            let tool_names: Vec<String> = store.tools.iter().map(|t| t.definition().name.clone()).collect();
            if tool_names.is_empty() {
                "No tools loaded.".to_string()
            } else {
                format!("**Loaded tools ({}):**\n{}", tool_names.len(), tool_names.iter().map(|n| format!("• `{}`", n)).collect::<Vec<_>>().join("\n"))
            }
        }
        _ => {
            // Unknown ! command — pass to agent as a normal prompt
            run_matrix_prompt(Arc::clone(&store), key.clone(), body.to_string()).await
        }
    }
}

/// Run a prompt for a Matrix message and collect the full text response.
async fn run_matrix_prompt(store: Arc<RwLock<SessionStore>>, key: SessionKey, text: String) -> String {
    // Get conversation history
    let (mut agent, history) = {
        let mut store = store.write().await;
        let session = store.get_or_create(&key);
        session.turn_count += 1;
        let messages = session.agent.messages().to_vec();
        let agent = Agent::new(
            Arc::clone(&store.provider),
            store.tools.clone(),
            store.settings.clone(),
            store.model.clone(),
            store.system_prompt.clone(),
        );
        (agent, messages)
    };

    agent.seed_messages(history);

    let mut rx = agent.subscribe();

    let collector = tokio::spawn(async move {
        let mut collected = String::new();
        while let Ok(event) = rx.recv().await {
            if let AgentEvent::MessageUpdate {
                delta: ContentDelta::TextDelta { ref text },
                ..
            } = event
            {
                collected.push_str(text);
            }
            if matches!(event, AgentEvent::AgentEnd { .. }) {
                break;
            }
        }
        collected
    });

    let result = agent.prompt(&text).await;
    let collected = collector.await.unwrap_or_default();

    // Save updated messages back
    let messages = agent.messages().to_vec();
    {
        let mut store = store.write().await;
        if let Some(session) = store.sessions.get_mut(&key) {
            session.agent.seed_messages(messages);
        }
    }

    match result {
        Ok(()) => collected,
        Err(e) => format!("Error: {e}"),
    }
}
