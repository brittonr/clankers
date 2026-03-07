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
use crate::provider::message::{Content, ImageSource};
use crate::provider::streaming::ContentDelta;
use crate::session::SessionManager;
use crate::tools::Tool;

use clankers_auth::{
    Capability, CapabilityToken, TokenBuilder, TokenVerifier, RevocationStore, RedbRevocationStore,
};

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
    /// Per-session heartbeat interval in seconds (0 = disabled).
    /// The daemon periodically reads each session's `HEARTBEAT.md`
    /// and prompts the agent with its contents. If the agent responds
    /// with "HEARTBEAT_OK", the response is suppressed.
    pub session_heartbeat_secs: u64,
    /// Prompt text prepended to HEARTBEAT.md contents.
    pub heartbeat_prompt: String,
    /// Enable per-session trigger pipes. When enabled, a FIFO at
    /// `{session_dir}/trigger.pipe` lets external processes inject
    /// prompts into the agent session.
    pub trigger_pipe_enabled: bool,
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
            session_heartbeat_secs: 300, // 5 minutes
            heartbeat_prompt: "Check your HEARTBEAT.md for pending tasks. \
                If nothing needs attention, respond with HEARTBEAT_OK."
                .to_string(),
            trigger_pipe_enabled: true,
        }
    }
}

/// Config subset passed to the Matrix bridge for proactive agent features.
#[derive(Debug, Clone)]
struct ProactiveConfig {
    session_heartbeat_secs: u64,
    heartbeat_prompt: String,
    trigger_pipe_enabled: bool,
}

// ── Session tracking ────────────────────────────────────────────────────────

// ── Auth layer ──────────────────────────────────────────────────────────────

/// Shared auth state for token verification + user→token mappings.
struct AuthLayer {
    /// Verifier with the daemon owner's key as trusted root
    verifier: TokenVerifier,
    /// Persistent revocation store (redb-backed, used for runtime revocation checks)
    #[allow(dead_code)]
    revocation_store: RedbRevocationStore,
    /// redb database for token storage
    db: Arc<redb::Database>,
    /// Daemon owner's secret key (for signing delegated child tokens)
    owner_key: ::iroh::SecretKey,
}

impl AuthLayer {
    /// Look up a stored token for a user ID (Matrix user ID or iroh pubkey).
    fn lookup_token(&self, user_id: &str) -> Option<CapabilityToken> {
        let read_txn = self.db.begin_read().ok()?;
        let table = read_txn
            .open_table(clankers_auth::revocation::AUTH_TOKENS_TABLE)
            .ok()?;
        let guard = table.get(user_id).ok()??;
        let bytes = guard.value().to_vec();
        CapabilityToken::decode(&bytes).ok()
    }

    /// Store a token for a user ID.
    fn store_token(&self, user_id: &str, token: &CapabilityToken) {
        let encoded = match token.encode() {
            Ok(e) => e,
            Err(e) => {
                warn!("Failed to encode token for storage: {e}");
                return;
            }
        };
        if let Ok(tx) = self.db.begin_write() {
            {
                if let Ok(mut table) = tx.open_table(clankers_auth::revocation::AUTH_TOKENS_TABLE) {
                    let _ = table.insert(user_id, encoded.as_slice());
                }
            }
            if let Err(e) = tx.commit() {
                warn!("Failed to store token: {e}");
            }
        }
    }

    /// Verify a token and return its capabilities, or an error message.
    fn verify_token(&self, token: &CapabilityToken) -> std::result::Result<Vec<Capability>, String> {
        self.verifier
            .verify(token, None)
            .map_err(|e| format!("{e}"))?;
        Ok(token.capabilities.clone())
    }

    /// Resolve capabilities for a user: token → verify → capabilities,
    /// or None if no token (fall back to allowlist).
    fn resolve_capabilities(&self, user_id: &str) -> Option<std::result::Result<Vec<Capability>, String>> {
        let token = self.lookup_token(user_id)?;
        Some(self.verify_token(&token))
    }
}

/// Filter a tool set based on capability tokens.
///
/// If capabilities include `ToolUse { "*" }`, all tools are kept.
/// Otherwise, only tools whose name appears in the ToolUse pattern are kept.
fn filter_tools_by_capabilities(
    tools: &[Arc<dyn Tool>],
    capabilities: &[Capability],
) -> Vec<Arc<dyn Tool>> {
    // Find the ToolUse capability (if any)
    let tool_pattern = capabilities.iter().find_map(|c| {
        if let Capability::ToolUse { tool_pattern } = c {
            Some(tool_pattern.as_str())
        } else {
            None
        }
    });

    match tool_pattern {
        None => Vec::new(), // No ToolUse capability → no tools
        Some("*") => tools.to_vec(), // Wildcard → all tools
        Some(pattern) => {
            let allowed: std::collections::HashSet<&str> =
                pattern.split(',').map(|s| s.trim()).collect();
            tools
                .iter()
                .filter(|t| allowed.contains(t.definition().name.as_str()))
                .cloned()
                .collect()
        }
    }
}



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
    /// Deterministic working directory for this session
    session_dir: PathBuf,
    /// Cancellation token for the trigger pipe reader task
    trigger_cancel: Option<CancellationToken>,
    /// Capabilities from the user's token (None = full access via allowlist)
    #[allow(dead_code)]
    capabilities: Option<Vec<Capability>>,
    /// Tools available to this session (filtered by capabilities)
    session_tools: Vec<Arc<dyn Tool>>,
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

impl SessionKey {
    /// Deterministic directory name for this session's working files.
    fn dir_name(&self) -> String {
        match self {
            Self::Iroh(id) => format!("daemon_iroh_{}", &id[..12.min(id.len())]),
            Self::Matrix { user_id, room_id } => {
                let user = user_id.replace(':', "_").replace('@', "");
                let room = room_id.replace(':', "_").replace('!', "");
                format!("daemon_matrix_{}_{}", user, room)
            }
        }
    }

    /// Extract the Matrix room_id if this is a Matrix session.
    fn matrix_room_id(&self) -> Option<&str> {
        match self {
            Self::Matrix { room_id, .. } => Some(room_id),
            _ => None,
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
    /// Shared auth layer for token verification
    auth: Option<Arc<AuthLayer>>,
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
        auth: Option<Arc<AuthLayer>>,
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
            auth,
        }
    }

    /// Get or create a per-session lock for serializing prompt execution.
    fn prompt_lock(&mut self, key: &SessionKey) -> Arc<Mutex<()>> {
        self.prompt_locks.entry(key.clone()).or_insert_with(|| Arc::new(Mutex::new(()))).clone()
    }

    /// Get or create a session for the given key.
    ///
    /// If `capabilities` is Some, tools are filtered based on the token's
    /// ToolUse capability. If None, all tools are available (allowlist user).
    fn get_or_create(
        &mut self,
        key: &SessionKey,
        capabilities: Option<&[Capability]>,
    ) -> &mut LiveSession {
        if !self.sessions.contains_key(key) {
            // Evict oldest session if at capacity
            if self.sessions.len() >= self.max_sessions {
                self.evict_oldest();
            }

            // Filter tools based on capabilities (or use all if no token)
            let session_tools = match capabilities {
                Some(caps) => filter_tools_by_capabilities(&self.tools, caps),
                None => self.tools.clone(),
            };

            let agent = crate::agent::builder::AgentBuilder::new(
                Arc::clone(&self.provider),
                self.settings.clone(),
                self.model.clone(),
                self.system_prompt.clone(),
            )
            .with_tools(session_tools.clone())
            .build();

            let session_mgr =
                SessionManager::create(&self.sessions_dir, "daemon", &self.model, Some("daemon"), None, None).ok();

            // Create a deterministic session directory for heartbeat/trigger files
            let session_dir = self.sessions_dir.join(key.dir_name());
            if let Err(e) = std::fs::create_dir_all(&session_dir) {
                warn!("Failed to create session dir {}: {e}", session_dir.display());
            }

            let tool_count = if capabilities.is_some() {
                format!(" ({} tools)", session_tools.len())
            } else {
                " (full access)".to_string()
            };
            info!("Created new session for {}{} (dir: {})", key, tool_count, session_dir.display());

            self.sessions.insert(key.clone(), LiveSession {
                agent,
                session_mgr,
                last_active: Utc::now(),
                turn_count: 0,
                session_dir,
                trigger_cancel: None,
                capabilities: capabilities.map(|c| c.to_vec()),
                session_tools,
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
            if let Some(session) = self.sessions.get(key) {
                // Cancel trigger pipe reader
                if let Some(ref cancel) = session.trigger_cancel {
                    cancel.cancel();
                }
                // Clean up trigger pipe file
                let pipe_path = session.session_dir.join("trigger.pipe");
                if pipe_path.exists()
                    && let Err(e) = std::fs::remove_file(&pipe_path) {
                        warn!("Failed to remove trigger pipe {}: {e}", pipe_path.display());
                    }
            }
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

    // ── iroh identity (needed for auth layer trusted root) ──────────
    let identity_path = iroh::identity_path(paths);
    let identity = iroh::Identity::load_or_generate(&identity_path);

    // ── Auth layer (UCAN tokens) ───────────────────────────────────
    let auth_layer = {
        let db_path = paths.global_config_dir.join("clankers.db");
        std::fs::create_dir_all(&paths.global_config_dir).ok();
        match redb::Database::create(&db_path) {
            Ok(db) => {
                let db = Arc::new(db);
                // Ensure auth tables exist
                if let Ok(tx) = db.begin_write() {
                    let _ = tx.open_table(clankers_auth::revocation::AUTH_TOKENS_TABLE);
                    let _ = tx.open_table(clankers_auth::revocation::REVOKED_TOKENS_TABLE);
                    let _ = tx.commit();
                }

                let revocation_store = match RedbRevocationStore::new(Arc::clone(&db)) {
                    Ok(s) => s,
                    Err(e) => {
                        warn!("Failed to init revocation store: {e}");
                        // Continue without auth
                        RedbRevocationStore::new(Arc::clone(&db)).expect("retry")
                    }
                };

                // Load revoked tokens into the verifier
                let revoked = revocation_store.load_all();
                let verifier = TokenVerifier::new()
                    .with_trusted_root(identity.public_key());
                if !revoked.is_empty() {
                    if let Err(e) = verifier.load_revoked(&revoked) {
                        warn!("Failed to load revoked tokens: {e}");
                    }
                    info!("Loaded {} revoked token(s)", revoked.len());
                }

                let layer = Arc::new(AuthLayer {
                    verifier,
                    revocation_store,
                    db,
                    owner_key: identity.secret_key.clone(),
                });
                info!("Auth layer initialized (trusted root: {})", identity.public_key().fmt_short());
                Some(layer)
            }
            Err(e) => {
                warn!("Failed to open auth database: {e} — running without token auth");
                None
            }
        }
    };

    // Build the session store
    let store = Arc::new(RwLock::new(SessionStore::new(
        Arc::clone(&provider),
        tools.clone(),
        config.settings.clone(),
        config.model.clone(),
        config.system_prompt.clone(),
        paths.global_sessions_dir.clone(),
        config.max_sessions,
        auth_layer.clone(),
    )));

    // ── iroh endpoint ───────────────────────────────────────────────
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
    println!("  Auth:     {}", if config.allow_all { "open" } else { "allowlist + UCAN tokens" });
    println!("  Model:    {}", config.model);
    println!("  Sessions: 0/{}", config.max_sessions);
    if !config.tags.is_empty() {
        println!("  Tags:     {}", config.tags.join(", "));
    }
    println!("  Tokens:   create with `clankers token create`");

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
                () = iroh_cancel.cancelled() => break,
            }
        }
    });

    // ── Matrix bridge (optional) ────────────────────────────────────
    let matrix_handle = if config.enable_matrix {
        let matrix_store = Arc::clone(&store);
        let matrix_cancel = cancel.clone();
        let matrix_paths = paths.clone();
        let matrix_allowed = config.matrix_allowed_users.clone();
        let proactive_config = ProactiveConfig {
            session_heartbeat_secs: config.session_heartbeat_secs,
            heartbeat_prompt: config.heartbeat_prompt.clone(),
            trigger_pipe_enabled: config.trigger_pipe_enabled,
        };
        Some(tokio::spawn(async move {
            if let Err(e) =
                run_matrix_bridge(matrix_store, matrix_cancel, &matrix_paths, matrix_allowed, proactive_config).await
            {
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
                () = status_cancel.cancelled() => break,
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
                    () = reaper_cancel.cancelled() => break,
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
///
/// Optional auth frame (first frame on connection):
///   `{ "type": "auth", "token": "<base64>" }`
///   If present, the token is verified and capabilities are used for the session.
///   If absent, falls back to allowlist (backwards compatible).
async fn handle_chat_connection(conn: ::iroh::endpoint::Connection, store: Arc<RwLock<SessionStore>>, peer_id: &str) {
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
    proactive: ProactiveConfig,
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

    // ── Heartbeat scheduler ─────────────────────────────────────────
    if proactive.session_heartbeat_secs > 0 {
        let hb_store = Arc::clone(&store);
        let hb_client = Arc::clone(&client);
        let hb_cancel = cancel.clone();
        let hb_interval = std::time::Duration::from_secs(proactive.session_heartbeat_secs);
        let hb_prompt = proactive.heartbeat_prompt.clone();
        tokio::spawn(async move {
            run_session_heartbeat(hb_store, hb_client, hb_interval, hb_prompt, hb_cancel).await;
        });
        info!(
            "Session heartbeat scheduler started (interval: {}s)",
            proactive.session_heartbeat_secs
        );
    }

    loop {
        tokio::select! {
            event = agent_rx.recv() => {
                let Some(event) = event else { break };
                match event {
                    BridgeEvent::TextMessage { sender, body, room_id }
                    | BridgeEvent::ChatMessage { sender, body, room_id, .. } => {
                        // ── Auth check: token → allowlist fallback ──
                        let sender_capabilities: Option<Vec<Capability>> = {
                            let store_guard = store.read().await;
                            if let Some(ref auth) = store_guard.auth {
                                match auth.resolve_capabilities(&sender) {
                                    Some(Ok(caps)) => Some(caps),
                                    Some(Err(e)) => {
                                        // Token exists but is invalid (expired, revoked, etc.)
                                        warn!("Matrix: token error for {}: {e}", sender);
                                        let room_id_parsed = clankers_matrix::ruma::RoomId::parse(&room_id).ok();
                                        if let Some(rid) = room_id_parsed {
                                            let c = client.read().await;
                                            let msg = format!(
                                                "Your access token is invalid: {e}\n\
                                                 Request a new one from the daemon owner, \
                                                 or register with `!token <base64>`."
                                            );
                                            let _ = c.send_markdown(&rid, &msg).await;
                                        }
                                        continue;
                                    }
                                    None => {
                                        // No token — fall back to allowlist
                                        if !is_user_allowed(&allowlist, &sender) {
                                            info!("Matrix: denied message from {} (no token, not on allowlist)", sender);
                                            continue;
                                        }
                                        None // Full access via allowlist
                                    }
                                }
                            } else {
                                // No auth layer — use allowlist only
                                if !is_user_allowed(&allowlist, &sender) {
                                    info!("Matrix: denied message from {}", sender);
                                    continue;
                                }
                                None
                            }
                        };

                        // ── Skip client slash commands ──────────────
                        if body.starts_with('/') {
                            continue;
                        }

                        // ── Bot command dispatch ────────────────────
                        if body.starts_with('!') {
                            let room_id_parsed = match clankers_matrix::ruma::RoomId::parse(&room_id) {
                                Ok(rid) => rid.clone(),
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
                            if let Err(e) = c.send_markdown(&room_id_parsed, &response).await {
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
                            Ok(rid) => rid.clone(),
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
                                    () = typing_token.cancelled() => break,
                                }
                            }
                        });

                        // ── Run prompt ──────────────────────────────
                        let mut response = run_matrix_prompt(
                            Arc::clone(&store), key, body,
                            sender_capabilities.as_deref(),
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
                                sender_capabilities.as_deref(),
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

                        // ── Sendfile extraction + upload ─────────────
                        let (cleaned, sendfiles) = extract_sendfile_tags(&response);
                        response = cleaned;

                        if !sendfiles.is_empty() {
                            let errors = upload_sendfiles(&client, &room_id_parsed, &sendfiles).await;
                            for err in errors {
                                response.push('\n');
                                response.push_str(&err);
                            }
                        }

                        // ── Send response ───────────────────────────
                        let c = client.read().await;
                        if let Err(e) = c.send_markdown(&room_id_parsed, &response).await {
                            error!("Matrix send failed: {e}");
                        }

                        // ── Trigger pipe setup ──────────────────────
                        if proactive.trigger_pipe_enabled {
                            let tp_key = SessionKey::Matrix {
                                user_id: sender.clone(),
                                room_id: room_id.clone(),
                            };
                            ensure_trigger_pipe(
                                Arc::clone(&store),
                                &tp_key,
                                Arc::clone(&client),
                            )
                            .await;
                        }
                    }
                    BridgeEvent::MediaMessage {
                        sender,
                        body,
                        filename,
                        media_type,
                        source,
                        room_id,
                    } => {
                        // ── Auth check: token → allowlist fallback ──
                        let sender_capabilities: Option<Vec<Capability>> = {
                            let store_guard = store.read().await;
                            if let Some(ref auth) = store_guard.auth {
                                match auth.resolve_capabilities(&sender) {
                                    Some(Ok(caps)) => Some(caps),
                                    Some(Err(_)) => {
                                        info!("Matrix: denied media from {} (invalid token)", sender);
                                        continue;
                                    }
                                    None => {
                                        if !is_user_allowed(&allowlist, &sender) {
                                            info!("Matrix: denied media from {}", sender);
                                            continue;
                                        }
                                        None
                                    }
                                }
                            } else {
                                if !is_user_allowed(&allowlist, &sender) {
                                    info!("Matrix: denied media from {}", sender);
                                    continue;
                                }
                                None
                            }
                        };

                        let key = SessionKey::Matrix {
                            user_id: sender.clone(),
                            room_id: room_id.clone(),
                        };

                        info!("[{}] media: {} ({})", key, filename, media_type);

                        let room_id_parsed = match clankers_matrix::ruma::RoomId::parse(&room_id) {
                            Ok(rid) => rid.clone(),
                            Err(_) => continue,
                        };

                        // ── Typing indicator: start ─────────────────
                        {
                            let c = client.read().await;
                            if let Err(e) = c.set_typing(&room_id_parsed, true).await {
                                warn!("Typing indicator start failed: {e}");
                            }
                        }

                        let typing_cancel = CancellationToken::new();
                        let typing_client = Arc::clone(&client);
                        let typing_room = room_id_parsed.clone();
                        let typing_token = typing_cancel.clone();
                        tokio::spawn(async move {
                            let mut interval = tokio::time::interval(std::time::Duration::from_secs(25));
                            interval.tick().await;
                            loop {
                                tokio::select! {
                                    _ = interval.tick() => {
                                        let c = typing_client.read().await;
                                        let _ = c.set_typing(&typing_room, true).await;
                                    }
                                    () = typing_token.cancelled() => break,
                                }
                            }
                        });

                        // ── Download the attachment ─────────────────
                        let download_result = {
                            let c = client.read().await;
                            c.download_media(&source).await
                        };

                        let file_bytes = match download_result {
                            Ok(bytes) => bytes,
                            Err(e) => {
                                error!("Failed to download media {}: {e}", filename);
                                typing_cancel.cancel();
                                let c = client.read().await;
                                let _ = c.set_typing(&room_id_parsed, false).await;
                                let _ = c.send_markdown(&room_id_parsed, &format!(
                                    "Failed to download attachment: {e}"
                                )).await;
                                continue;
                            }
                        };

                        // ── Save to session attachments dir ─────────
                        let attachments_dir = paths
                            .global_sessions_dir
                            .join(format!(
                                "matrix_{}_{}",
                                sender.replace(':', "_"),
                                room_id.replace(':', "_")
                            ))
                            .join("attachments");
                        if let Err(e) = std::fs::create_dir_all(&attachments_dir) {
                            error!("Failed to create attachments dir: {e}");
                        }

                        let save_path = attachments_dir.join(&filename);
                        if let Err(e) = std::fs::write(&save_path, &file_bytes) {
                            error!("Failed to save attachment {}: {e}", save_path.display());
                        }

                        // ── Build prompt with image if applicable ───
                        let caption = if body != filename { &body } else { "" };
                        let is_image = media_type == "image";

                        let mut response = if is_image {
                            // Pass image as base64 content block for vision
                            let b64 = base64::Engine::encode(
                                &base64::engine::general_purpose::STANDARD,
                                &file_bytes,
                            );
                            let mime_str = guess_mime(&save_path).to_string();

                            let image_content = Content::Image {
                                source: ImageSource::Base64 {
                                    media_type: mime_str,
                                    data: b64,
                                },
                            };

                            let prompt_text = if caption.is_empty() {
                                format!(
                                    "User sent an image: {} (saved to {})",
                                    filename,
                                    save_path.display()
                                )
                            } else {
                                format!(
                                    "User sent an image with caption \"{}\": {} (saved to {})",
                                    caption, filename, save_path.display()
                                )
                            };

                            run_matrix_prompt_with_images(
                                Arc::clone(&store),
                                key,
                                prompt_text,
                                vec![image_content],
                                sender_capabilities.as_deref(),
                            )
                            .await
                        } else {
                            let prompt_text = if caption.is_empty() {
                                format!(
                                    "User sent a {} file: {} (saved to {})",
                                    media_type, filename, save_path.display()
                                )
                            } else {
                                format!(
                                    "User sent a {} file with caption \"{}\": {} (saved to {})",
                                    media_type, caption, filename, save_path.display()
                                )
                            };

                            run_matrix_prompt(Arc::clone(&store), key, prompt_text, sender_capabilities.as_deref()).await
                        };

                        // ── Empty response re-prompt ────────────────
                        if response.trim().is_empty() {
                            let re_key = SessionKey::Matrix {
                                user_id: sender.clone(),
                                room_id: room_id.clone(),
                            };
                            let retry = run_matrix_prompt(
                                Arc::clone(&store),
                                re_key,
                                "You processed a file but your response contained no text. \
                                 Briefly summarize what you did."
                                    .to_string(),
                                sender_capabilities.as_deref(),
                            )
                            .await;
                            response = if retry.trim().is_empty() {
                                "(processed file — no summary available)".to_string()
                            } else {
                                retry
                            };
                        }

                        // ── Sendfile extraction + upload ────────────
                        let (cleaned, sendfiles) = extract_sendfile_tags(&response);
                        response = cleaned;

                        if !sendfiles.is_empty() {
                            let errors = upload_sendfiles(&client, &room_id_parsed, &sendfiles).await;
                            for err in errors {
                                response.push('\n');
                                response.push_str(&err);
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
                        if let Err(e) = c.send_markdown(&room_id_parsed, &response).await {
                            error!("Matrix send failed: {e}");
                        }

                        // ── Trigger pipe setup ──────────────────────
                        if proactive.trigger_pipe_enabled {
                            let tp_key = SessionKey::Matrix {
                                user_id: sender.clone(),
                                room_id: room_id.clone(),
                            };
                            ensure_trigger_pipe(
                                Arc::clone(&store),
                                &tp_key,
                                Arc::clone(&client),
                            )
                            .await;
                        }
                    }
                    BridgeEvent::PeerUpdate(peer) => {
                        info!("Matrix peer update: {} ({})", peer.instance_name, peer.user_id);
                    }
                    _ => {}
                }
            }
            () = cancel.cancelled() => break,
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
             • `!skills` — List loaded skills\n\
             • `!token <base64>` — Register an access token\n\
             • `!delegate [opts]` — Create a child token from yours"
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
                None, // compaction uses existing session capabilities
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
        "!token" => {
            if args.is_empty() {
                return "Usage: `!token <base64-encoded-token>`\n\n\
                        Register an access token to get daemon capabilities.\n\
                        Get a token from the daemon owner: `clankers token create`".to_string();
            }

            let store_guard = store.read().await;
            let Some(ref auth) = store_guard.auth else {
                return "Token auth is not enabled on this daemon.".to_string();
            };

            // Decode and verify the token
            let token = match CapabilityToken::from_base64(args) {
                Ok(t) => t,
                Err(e) => return format!("Invalid token: {e}"),
            };

            match auth.verify_token(&token) {
                Ok(caps) => {
                    // Extract the user ID from the session key
                    let user_id = match key {
                        SessionKey::Matrix { user_id, .. } => user_id.clone(),
                        SessionKey::Iroh(id) => id.clone(),
                    };

                    // Store the token mapped to the user ID
                    auth.store_token(&user_id, &token);

                    // Restart the session so it picks up the new capabilities
                    drop(store_guard);
                    {
                        let mut store = store.write().await;
                        store.sessions.remove(key);
                        store.prompt_locks.remove(key);
                    }

                    let cap_names: Vec<&str> = caps.iter().map(|c| match c {
                        Capability::Prompt => "Prompt",
                        Capability::ToolUse { .. } => "ToolUse",
                        Capability::ShellExecute { .. } => "ShellExecute",
                        Capability::FileAccess { .. } => "FileAccess",
                        Capability::BotCommand { .. } => "BotCommand",
                        Capability::SessionManage => "SessionManage",
                        Capability::ModelSwitch => "ModelSwitch",
                        Capability::Delegate => "Delegate",
                    }).collect();

                    let expires = chrono::DateTime::from_timestamp(token.expires_at as i64, 0)
                        .map(|dt| dt.format("%Y-%m-%d %H:%M UTC").to_string())
                        .unwrap_or_else(|| "unknown".to_string());

                    format!(
                        "**Token accepted** ✓\n\n\
                         • Capabilities: {}\n\
                         • Expires: {}\n\
                         • Depth: {}\n\n\
                         Your session has been restarted with the new capabilities.",
                        cap_names.join(", "),
                        expires,
                        token.delegation_depth,
                    )
                }
                Err(e) => format!("**Token rejected:** {e}"),
            }
        }
        "!delegate" => {
            if args.is_empty() {
                return "**Delegate a child token from yours**\n\n\
                        Usage: `!delegate [options]`\n\n\
                        Options:\n\
                        • `--tools <pattern>` — comma-separated tool names or `*`\n\
                        • `--read-only` — shorthand for `--tools read,grep,find,ls`\n\
                        • `--expire <duration>` — e.g. `1h`, `7d`, `30m`\n\
                        • `--shell` — include ShellExecute capability\n\
                        • `--no-delegate` — child cannot further delegate\n\n\
                        Your token must have the Delegate capability.\n\
                        Child tokens cannot exceed your own permissions."
                    .to_string();
            }

            let store_guard = store.read().await;
            let Some(ref auth) = store_guard.auth else {
                return "Token auth is not enabled on this daemon.".to_string();
            };

            // Look up the sender's token
            let user_id = match key {
                SessionKey::Matrix { user_id, .. } => user_id.clone(),
                SessionKey::Iroh(id) => id.clone(),
            };

            let parent_token = match auth.lookup_token(&user_id) {
                Some(t) => t,
                None => return "You don't have a registered token. Use `!token <base64>` first.".to_string(),
            };

            // Verify the parent token is still valid
            if let Err(e) = auth.verify_token(&parent_token) {
                return format!("Your token is invalid: {e}");
            }

            // Check that the parent has Delegate capability
            if !parent_token.capabilities.contains(&Capability::Delegate) {
                return "Your token does not have the Delegate capability.".to_string();
            }

            // Parse !delegate flags
            let parts: Vec<&str> = args.split_whitespace().collect();
            let mut tools_pattern: Option<String> = None;
            let mut expire_str: Option<&str> = None;
            let mut include_shell = false;
            let mut allow_delegate = true;
            let mut read_only = false;
            let mut i = 0;

            while i < parts.len() {
                match parts[i] {
                    "--tools" => {
                        if i + 1 < parts.len() {
                            tools_pattern = Some(parts[i + 1].to_string());
                            i += 2;
                        } else {
                            return "`--tools` requires a pattern (e.g. `read,grep` or `*`)".to_string();
                        }
                    }
                    "--expire" => {
                        if i + 1 < parts.len() {
                            expire_str = Some(parts[i + 1]);
                            i += 2;
                        } else {
                            return "`--expire` requires a duration (e.g. `1h`, `7d`)".to_string();
                        }
                    }
                    "--shell" => {
                        include_shell = true;
                        i += 1;
                    }
                    "--no-delegate" => {
                        allow_delegate = false;
                        i += 1;
                    }
                    "--read-only" => {
                        read_only = true;
                        i += 1;
                    }
                    other => {
                        return format!("Unknown flag: `{other}`. See `!delegate` for usage.");
                    }
                }
            }

            if read_only {
                tools_pattern = Some("read,grep,find,ls".to_string());
            }

            // Default to 1h if no expiry specified
            let lifetime = match expire_str {
                Some(s) => match parse_delegate_duration(s) {
                    Some(d) => d,
                    None => return format!("Invalid duration: `{s}`. Use e.g. `30m`, `1h`, `7d`, `1y`."),
                },
                None => std::time::Duration::from_secs(3600),
            };

            // Cap child lifetime to parent's remaining lifetime
            let now = clankers_auth::utils::current_time_secs();
            let parent_remaining = parent_token.expires_at.saturating_sub(now);
            let lifetime = lifetime.min(std::time::Duration::from_secs(parent_remaining));

            // Build capabilities for the child token
            let mut child_caps = vec![Capability::Prompt];

            if let Some(pattern) = tools_pattern {
                child_caps.push(Capability::ToolUse {
                    tool_pattern: pattern,
                });
            }

            if include_shell {
                child_caps.push(Capability::ShellExecute {
                    command_pattern: "*".into(),
                    working_dir: None,
                });
            }

            if allow_delegate {
                child_caps.push(Capability::Delegate);
            }

            // Build the child token (signed by daemon owner key)
            let builder = TokenBuilder::new(auth.owner_key.clone())
                .delegated_from(parent_token)
                .with_capabilities(child_caps)
                .with_lifetime(lifetime)
                .with_random_nonce();

            match builder.build() {
                Ok(child_token) => {
                    let b64 = match child_token.to_base64() {
                        Ok(b) => b,
                        Err(e) => return format!("Failed to encode child token: {e}"),
                    };

                    let cap_names: Vec<&str> = child_token.capabilities.iter().map(|c| match c {
                        Capability::Prompt => "Prompt",
                        Capability::ToolUse { .. } => "ToolUse",
                        Capability::ShellExecute { .. } => "ShellExecute",
                        Capability::FileAccess { .. } => "FileAccess",
                        Capability::BotCommand { .. } => "BotCommand",
                        Capability::SessionManage => "SessionManage",
                        Capability::ModelSwitch => "ModelSwitch",
                        Capability::Delegate => "Delegate",
                    }).collect();

                    let expires = chrono::DateTime::from_timestamp(child_token.expires_at as i64, 0)
                        .map(|dt| dt.format("%Y-%m-%d %H:%M UTC").to_string())
                        .unwrap_or_else(|| "unknown".to_string());

                    format!(
                        "**Child token created** ✓\n\n\
                         • Capabilities: {}\n\
                         • Expires: {}\n\
                         • Depth: {}\n\n\
                         ```\n{}\n```\n\n\
                         Share this with the recipient. They register it with `!token <token>`.",
                        cap_names.join(", "),
                        expires,
                        child_token.delegation_depth,
                        b64,
                    )
                }
                Err(e) => format!("**Delegation failed:** {e}"),
            }
        }
        _ => {
            // Unknown ! command — pass to agent as a normal prompt
            run_matrix_prompt(Arc::clone(&store), key.clone(), body.to_string(), None).await
        }
    }
}

/// Parse duration strings like "30m", "1h", "7d", "1y" into `std::time::Duration`.
fn parse_delegate_duration(s: &str) -> Option<std::time::Duration> {
    let s = s.trim();
    let (num_str, unit) = if s.ends_with('m') {
        (&s[..s.len() - 1], 'm')
    } else if s.ends_with('h') {
        (&s[..s.len() - 1], 'h')
    } else if s.ends_with('d') {
        (&s[..s.len() - 1], 'd')
    } else if s.ends_with('y') {
        (&s[..s.len() - 1], 'y')
    } else {
        return None;
    };
    let num: u64 = num_str.parse().ok()?;
    let secs = match unit {
        'm' => num * 60,
        'h' => num * 3600,
        'd' => num * 86400,
        'y' => num * 86400 * 365,
        _ => return None,
    };
    Some(std::time::Duration::from_secs(secs))
}

// ── Sendfile tag extraction ──────────────────────────────────────────────

/// A file the agent wants to send back to the user.
struct SendfileTag {
    /// Absolute path to the file
    path: String,
}

/// Extract `<sendfile>/path</sendfile>` tags from response text.
/// Returns the cleaned text (tags stripped) and a list of file paths.
fn extract_sendfile_tags(text: &str) -> (String, Vec<SendfileTag>) {
    let mut cleaned = String::with_capacity(text.len());
    let mut tags = Vec::new();
    let mut remaining = text;

    while let Some(start) = remaining.find("<sendfile>") {
        // Copy text before the tag
        cleaned.push_str(&remaining[..start]);

        let after_open = &remaining[start + "<sendfile>".len()..];
        if let Some(end) = after_open.find("</sendfile>") {
            let path = after_open[..end].trim().to_string();
            if !path.is_empty() {
                tags.push(SendfileTag { path });
            }
            remaining = &after_open[end + "</sendfile>".len()..];
        } else {
            // Unclosed tag — keep it as-is (prefix already pushed above)
            cleaned.push_str("<sendfile>");
            remaining = after_open;
        }
    }
    cleaned.push_str(remaining);

    (cleaned, tags)
}

/// Guess MIME type from file extension.
fn guess_mime(path: &std::path::Path) -> mime::Mime {
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
    match ext.to_lowercase().as_str() {
        "png" => mime::IMAGE_PNG,
        "jpg" | "jpeg" => mime::IMAGE_JPEG,
        "gif" => mime::IMAGE_GIF,
        "webp" => "image/webp".parse().unwrap_or(mime::APPLICATION_OCTET_STREAM),
        "svg" => mime::IMAGE_SVG,
        "mp4" => "video/mp4".parse().unwrap_or(mime::APPLICATION_OCTET_STREAM),
        "webm" => "video/webm".parse().unwrap_or(mime::APPLICATION_OCTET_STREAM),
        "mp3" => "audio/mpeg".parse().unwrap_or(mime::APPLICATION_OCTET_STREAM),
        "ogg" => "audio/ogg".parse().unwrap_or(mime::APPLICATION_OCTET_STREAM),
        "wav" => "audio/wav".parse().unwrap_or(mime::APPLICATION_OCTET_STREAM),
        "pdf" => mime::APPLICATION_PDF,
        "txt" | "md" | "rs" | "py" | "js" | "ts" | "toml" | "yaml" | "yml" | "json" => mime::TEXT_PLAIN,
        _ => mime::APPLICATION_OCTET_STREAM,
    }
}

/// Upload sendfile tags to Matrix and return error annotations for failures.
/// Check whether a path is safe to send over Matrix.
///
/// Blocks known sensitive directories and files to prevent the agent from
/// accidentally exfiltrating credentials, keys, or system secrets.
fn is_sendfile_path_allowed(path: &std::path::Path) -> std::result::Result<(), String> {
    // Canonicalize to resolve symlinks and ../ tricks
    let canonical = path
        .canonicalize()
        .map_err(|e| format!("cannot resolve path: {e}"))?;
    let s = canonical.to_string_lossy();

    // Blocked directory prefixes (home-relative)
    if let Some(home) = dirs::home_dir() {
        let home_str = home.to_string_lossy();
        let blocked_dirs = [
            ".ssh",
            ".gnupg",
            ".gpg",
            ".aws",
            ".azure",
            ".config/gcloud",
            ".kube",
            ".docker",
            ".npmrc",
            ".pypirc",
            ".netrc",
            ".clankers/matrix.json",
        ];
        for dir in &blocked_dirs {
            let blocked = format!("{}/{}", home_str, dir);
            if s.starts_with(&blocked) {
                return Err(format!("blocked: path inside ~/{dir}"));
            }
        }
    }

    // Blocked system paths
    let blocked_system = [
        "/etc/shadow",
        "/etc/gshadow",
        "/etc/master.passwd",
        "/etc/sudoers",
    ];
    for bp in &blocked_system {
        if s.as_ref() == *bp || s.starts_with(&format!("{bp}.")) {
            return Err(format!("blocked: sensitive system file {bp}"));
        }
    }

    // Block private key files by name pattern
    let filename = canonical
        .file_name()
        .map(|f| f.to_string_lossy().to_string())
        .unwrap_or_default();
    let blocked_names = [
        "id_rsa",
        "id_ed25519",
        "id_ecdsa",
        "id_dsa",
        ".env",
        ".env.local",
        ".env.production",
    ];
    for bn in &blocked_names {
        if filename == *bn {
            return Err(format!("blocked: sensitive file name {bn}"));
        }
    }

    Ok(())
}

async fn upload_sendfiles(
    client: &tokio::sync::RwLock<clankers_matrix::MatrixClient>,
    room_id: &clankers_matrix::ruma::OwnedRoomId,
    tags: &[SendfileTag],
) -> Vec<String> {
    let mut errors = Vec::new();

    for tag in tags {
        let path = std::path::Path::new(&tag.path);

        if !path.exists() || !path.is_file() {
            errors.push(format!("(failed to send file {}: file not found)", tag.path));
            continue;
        }

        // Path validation: block sensitive files
        if let Err(reason) = is_sendfile_path_allowed(path) {
            warn!("Sendfile blocked: {} ({})", tag.path, reason);
            errors.push(format!("(refused to send file {}: {})", tag.path, reason));
            continue;
        }

        let data = match std::fs::read(path) {
            Ok(d) => d,
            Err(e) => {
                errors.push(format!(
                    "(failed to send file {}: {})",
                    path.file_name().unwrap_or_default().to_string_lossy(),
                    e
                ));
                continue;
            }
        };

        let filename = path.file_name().unwrap_or_default().to_string_lossy().to_string();
        let content_type = guess_mime(path);

        let c = client.read().await;
        if let Err(e) = c.send_file(room_id, &filename, &content_type, data).await {
            errors.push(format!("(failed to send file {}: {})", filename, e));
        } else {
            info!("Uploaded file to Matrix: {}", filename);
        }
    }

    errors
}

/// Run a prompt for a Matrix message and collect the full text response.
///
/// If `capabilities` is Some, the session is created with filtered tools.
/// If None, full access (allowlist user).
async fn run_matrix_prompt(
    store: Arc<RwLock<SessionStore>>,
    key: SessionKey,
    text: String,
    capabilities: Option<&[Capability]>,
) -> String {
    // Get conversation history
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

/// Run a prompt with image content blocks (for vision models).
async fn run_matrix_prompt_with_images(
    store: Arc<RwLock<SessionStore>>,
    key: SessionKey,
    text: String,
    images: Vec<Content>,
    capabilities: Option<&[Capability]>,
) -> String {
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

    let result = agent.prompt_with_images(&text, images).await;
    let collected = collector.await.unwrap_or_default();

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

// ── Proactive agent: heartbeat + trigger pipes ──────────────────────────────

/// Check whether a response signals "nothing to report".
fn is_heartbeat_ok(response: &str) -> bool {
    let upper = response.to_uppercase();
    upper.contains("HEARTBEAT_OK") || upper.contains("HEARTBEAT OK")
}

/// Run a prompt against a session without updating `last_active`.
/// Used for heartbeat and trigger prompts — these shouldn't prevent
/// idle reaping.
async fn run_proactive_prompt(
    store: Arc<RwLock<SessionStore>>,
    key: SessionKey,
    text: String,
) -> String {
    // Serialize via prompt lock
    let prompt_lock = {
        let mut store = store.write().await;
        store.prompt_lock(&key)
    };
    let _prompt_guard = prompt_lock.lock().await;

    let (mut agent, history) = {
        let mut store = store.write().await;
        let session = match store.sessions.get_mut(&key) {
            Some(s) => s,
            None => return String::new(), // session gone
        };
        // Deliberately do NOT update last_active or turn_count
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

/// Ensure a trigger pipe reader is running for a Matrix session.
/// No-op if the session already has one or the key is not Matrix.
async fn ensure_trigger_pipe(
    store: Arc<RwLock<SessionStore>>,
    key: &SessionKey,
    matrix_client: Arc<tokio::sync::RwLock<clankers_matrix::MatrixClient>>,
) {
    if key.matrix_room_id().is_none() {
        return;
    }
    let needs_spawn = {
        let store = store.read().await;
        match store.sessions.get(key) {
            Some(s) => s.trigger_cancel.is_none(),
            None => false,
        }
    };

    if !needs_spawn {
        return;
    }

    let session_dir = {
        let store = store.read().await;
        match store.sessions.get(key) {
            Some(s) => s.session_dir.clone(),
            None => return,
        }
    };

    let cancel = spawn_trigger_reader(
        &session_dir,
        key.clone(),
        Arc::clone(&store),
        matrix_client,
    );

    if let Some(cancel) = cancel {
        let mut store = store.write().await;
        if let Some(session) = store.sessions.get_mut(key) {
            session.trigger_cancel = Some(cancel);
        }
    }
}

/// Run the per-session heartbeat scheduler.
///
/// Iterates all active Matrix sessions, checks for HEARTBEAT.md,
/// and prompts the agent if the file is non-empty. Responses
/// containing "HEARTBEAT_OK" are suppressed.
async fn run_session_heartbeat(
    store: Arc<RwLock<SessionStore>>,
    matrix_client: Arc<tokio::sync::RwLock<clankers_matrix::MatrixClient>>,
    interval: std::time::Duration,
    heartbeat_prompt: String,
    cancel: CancellationToken,
) {
    let mut ticker = tokio::time::interval(interval);
    ticker.tick().await; // skip the immediate first tick

    loop {
        tokio::select! {
            _ = ticker.tick() => {}
            () = cancel.cancelled() => break,
        }

        // Snapshot all Matrix sessions that have a HEARTBEAT.md
        let targets: Vec<(SessionKey, PathBuf, String)> = {
            let store = store.read().await;
            store
                .sessions
                .iter()
                .filter_map(|(key, session)| {
                    let room_id = key.matrix_room_id()?.to_string();
                    let hb_path = session.session_dir.join("HEARTBEAT.md");
                    Some((key.clone(), hb_path, room_id))
                })
                .collect()
        };

        for (key, hb_path, room_id) in targets {
            // Read heartbeat file
            let contents = match tokio::fs::read_to_string(&hb_path).await {
                Ok(c) if !c.trim().is_empty() => c,
                _ => continue, // missing or empty — skip
            };

            info!("[{}] heartbeat: found {} bytes in HEARTBEAT.md", key, contents.len());

            let prompt = format!("{}\n\n---\n\n{}", heartbeat_prompt, contents);

            // Start typing
            let room_id_parsed = match clankers_matrix::ruma::RoomId::parse(&room_id) {
                Ok(rid) => rid.clone(),
                Err(_) => continue,
            };
            {
                let c = matrix_client.read().await;
                let _ = c.set_typing(&room_id_parsed, true).await;
            }

            // Prompt the agent (without updating last_active)
            let response = run_proactive_prompt(
                Arc::clone(&store),
                key.clone(),
                prompt,
            )
            .await;

            // Stop typing
            {
                let c = matrix_client.read().await;
                let _ = c.set_typing(&room_id_parsed, false).await;
            }

            // Suppress HEARTBEAT_OK responses
            if is_heartbeat_ok(&response) {
                info!("[{}] heartbeat: OK (suppressed)", key);
                continue;
            }

            if response.trim().is_empty() {
                info!("[{}] heartbeat: empty response (suppressed)", key);
                continue;
            }

            // Send response to Matrix
            let c = matrix_client.read().await;
            if let Err(e) = c.send_markdown(&room_id_parsed, &response).await {
                error!("[{}] heartbeat send failed: {e}", key);
            } else {
                info!("[{}] heartbeat: sent response ({} bytes)", key, response.len());
            }
        }
    }
}

/// Create a named pipe (FIFO) at the given path.
/// Returns Ok(()) if created or already exists, Err on failure.
fn create_fifo(path: &std::path::Path) -> std::io::Result<()> {
    if path.exists() {
        return Ok(());
    }

    let c_path = std::ffi::CString::new(path.to_string_lossy().as_bytes())
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidInput, e))?;

    // Mode 0o660: owner+group read/write
    let result = unsafe { libc::mkfifo(c_path.as_ptr(), 0o660) };
    if result == 0 {
        Ok(())
    } else {
        Err(std::io::Error::last_os_error())
    }
}

/// Spawn a trigger pipe reader task for a session.
///
/// Creates a FIFO at `{session_dir}/trigger.pipe` and reads lines from it.
/// Each line becomes a prompt to the agent; responses go to the Matrix room.
/// Returns the cancellation token used to stop the reader.
fn spawn_trigger_reader(
    session_dir: &std::path::Path,
    key: SessionKey,
    store: Arc<RwLock<SessionStore>>,
    matrix_client: Arc<tokio::sync::RwLock<clankers_matrix::MatrixClient>>,
) -> Option<CancellationToken> {
    let room_id = key.matrix_room_id()?.to_string();
    let pipe_path = session_dir.join("trigger.pipe");

    if let Err(e) = create_fifo(&pipe_path) {
        error!("[{}] failed to create trigger pipe {}: {e}", key, pipe_path.display());
        return None;
    }

    info!("[{}] trigger pipe: {}", key, pipe_path.display());

    let cancel = CancellationToken::new();
    let cancel_clone = cancel.clone();

    tokio::spawn(async move {
        use tokio::io::AsyncBufReadExt;

        loop {
            // Open the FIFO — this blocks until a writer opens the other end.
            // When the writer closes, we get EOF and re-open.
            let file = tokio::select! {
                f = tokio::fs::File::open(&pipe_path) => {
                    match f {
                        Ok(f) => f,
                        Err(e) => {
                            warn!("[{}] trigger pipe open failed: {e}", key);
                            tokio::time::sleep(std::time::Duration::from_secs(1)).await;
                            continue;
                        }
                    }
                }
                () = cancel_clone.cancelled() => break,
            };

            let reader = tokio::io::BufReader::new(file);
            let mut lines = reader.lines();

            loop {
                let line = tokio::select! {
                    l = lines.next_line() => l,
                    () = cancel_clone.cancelled() => break,
                };

                match line {
                    Ok(Some(text)) if !text.trim().is_empty() => {
                        info!("[{}] trigger: {}", key, &text[..80.min(text.len())]);

                        let room_id_parsed = match clankers_matrix::ruma::RoomId::parse(&room_id) {
                            Ok(rid) => rid.clone(),
                            Err(_) => continue,
                        };

                        // Typing indicator
                        {
                            let c = matrix_client.read().await;
                            let _ = c.set_typing(&room_id_parsed, true).await;
                        }

                        let response = run_proactive_prompt(
                            Arc::clone(&store),
                            key.clone(),
                            text,
                        )
                        .await;

                        {
                            let c = matrix_client.read().await;
                            let _ = c.set_typing(&room_id_parsed, false).await;
                        }

                        if is_heartbeat_ok(&response) || response.trim().is_empty() {
                            info!("[{}] trigger: suppressed (ok/empty)", key);
                            continue;
                        }

                        let c = matrix_client.read().await;
                        if let Err(e) = c.send_markdown(&room_id_parsed, &response).await {
                            error!("[{}] trigger send failed: {e}", key);
                        }
                    }
                    Ok(Some(_)) => {} // empty line, skip
                    Ok(None) => break, // EOF — writer closed, re-open
                    Err(e) => {
                        warn!("[{}] trigger pipe read error: {e}", key);
                        break;
                    }
                }
            }
        }
    });

    Some(cancel)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_sendfile_single_tag() {
        let text = "Here is the file: <sendfile>/tmp/output.png</sendfile> done.";
        let (cleaned, tags) = extract_sendfile_tags(text);
        assert_eq!(cleaned, "Here is the file:  done.");
        assert_eq!(tags.len(), 1);
        assert_eq!(tags[0].path, "/tmp/output.png");
    }

    #[test]
    fn extract_sendfile_multiple_tags() {
        let text = "Files: <sendfile>/a.png</sendfile> and <sendfile>/b.txt</sendfile>.";
        let (cleaned, tags) = extract_sendfile_tags(text);
        assert_eq!(cleaned, "Files:  and .");
        assert_eq!(tags.len(), 2);
        assert_eq!(tags[0].path, "/a.png");
        assert_eq!(tags[1].path, "/b.txt");
    }

    #[test]
    fn extract_sendfile_no_tags() {
        let text = "No files here.";
        let (cleaned, tags) = extract_sendfile_tags(text);
        assert_eq!(cleaned, "No files here.");
        assert!(tags.is_empty());
    }

    #[test]
    fn extract_sendfile_unclosed_tag() {
        let text = "Broken: <sendfile>/tmp/lost";
        let (cleaned, tags) = extract_sendfile_tags(text);
        assert_eq!(cleaned, "Broken: <sendfile>/tmp/lost");
        assert!(tags.is_empty());
    }

    #[test]
    fn extract_sendfile_whitespace_in_path() {
        let text = "<sendfile>  /tmp/with spaces.png  </sendfile>";
        let (cleaned, tags) = extract_sendfile_tags(text);
        assert_eq!(cleaned, "");
        assert_eq!(tags.len(), 1);
        assert_eq!(tags[0].path, "/tmp/with spaces.png");
    }

    #[test]
    fn extract_sendfile_empty_path_skipped() {
        let text = "Empty: <sendfile>  </sendfile> end.";
        let (cleaned, tags) = extract_sendfile_tags(text);
        assert_eq!(cleaned, "Empty:  end.");
        assert!(tags.is_empty());
    }

    #[test]
    fn guess_mime_common_types() {
        assert_eq!(guess_mime(std::path::Path::new("photo.png")), mime::IMAGE_PNG);
        assert_eq!(guess_mime(std::path::Path::new("photo.jpg")), mime::IMAGE_JPEG);
        assert_eq!(guess_mime(std::path::Path::new("photo.JPEG")), mime::IMAGE_JPEG);
        assert_eq!(guess_mime(std::path::Path::new("doc.pdf")), mime::APPLICATION_PDF);
        assert_eq!(guess_mime(std::path::Path::new("code.rs")), mime::TEXT_PLAIN);
        assert_eq!(guess_mime(std::path::Path::new("data.bin")), mime::APPLICATION_OCTET_STREAM);
        assert_eq!(guess_mime(std::path::Path::new("noext")), mime::APPLICATION_OCTET_STREAM);
    }

    // ── Proactive agent tests ───────────────────────────────────────

    #[test]
    fn heartbeat_ok_detection() {
        assert!(is_heartbeat_ok("HEARTBEAT_OK"));
        assert!(is_heartbeat_ok("heartbeat_ok"));
        assert!(is_heartbeat_ok("Nothing to do. HEARTBEAT_OK."));
        assert!(is_heartbeat_ok("All clear — HEARTBEAT OK"));
        assert!(is_heartbeat_ok("Heartbeat_Ok"));
        assert!(!is_heartbeat_ok("I found some issues to report."));
        assert!(!is_heartbeat_ok("The heartbeat file is empty."));
        assert!(!is_heartbeat_ok(""));
    }

    #[test]
    fn session_key_dir_name_matrix() {
        let key = SessionKey::Matrix {
            user_id: "@alice:example.com".to_string(),
            room_id: "!abc123:example.com".to_string(),
        };
        let name = key.dir_name();
        assert_eq!(name, "daemon_matrix_alice_example.com_abc123_example.com");
        assert!(!name.contains(':'));
        assert!(!name.contains('@'));
        assert!(!name.contains('!'));
    }

    #[test]
    fn session_key_dir_name_iroh() {
        let key = SessionKey::Iroh("abcdef123456789".to_string());
        assert_eq!(key.dir_name(), "daemon_iroh_abcdef123456");
    }

    #[test]
    fn session_key_matrix_room_id() {
        let key = SessionKey::Matrix {
            user_id: "@alice:example.com".to_string(),
            room_id: "!room:example.com".to_string(),
        };
        assert_eq!(key.matrix_room_id(), Some("!room:example.com"));

        let key = SessionKey::Iroh("abc".to_string());
        assert_eq!(key.matrix_room_id(), None);
    }

    #[test]
    fn create_fifo_in_tempdir() {
        let dir = tempfile::tempdir().unwrap();
        let pipe_path = dir.path().join("test.pipe");

        // First create should succeed
        create_fifo(&pipe_path).unwrap();
        assert!(pipe_path.exists());

        // Second create should be a no-op (already exists)
        create_fifo(&pipe_path).unwrap();
    }

    #[test]
    fn proactive_config_defaults() {
        let config = DaemonConfig::default();
        assert_eq!(config.session_heartbeat_secs, 300);
        assert!(config.trigger_pipe_enabled);
        assert!(config.heartbeat_prompt.contains("HEARTBEAT"));
    }

    // ── Path validation tests ───────────────────────────────────────

    #[test]
    fn sendfile_allows_normal_tmp_file() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("output.png");
        std::fs::write(&file, "fake png").unwrap();
        assert!(is_sendfile_path_allowed(&file).is_ok());
    }

    #[test]
    fn sendfile_blocks_ssh_keys() {
        let home = dirs::home_dir().unwrap();
        let ssh_key = home.join(".ssh/id_rsa");
        if ssh_key.exists() {
            assert!(is_sendfile_path_allowed(&ssh_key).is_err());
        }
        // Even if file doesn't exist, canonicalize will fail — that's fine
    }

    #[test]
    fn sendfile_blocks_dot_env() {
        let dir = tempfile::tempdir().unwrap();
        let env_file = dir.path().join(".env");
        std::fs::write(&env_file, "SECRET=hunter2").unwrap();
        assert!(is_sendfile_path_allowed(&env_file).is_err());
    }

    #[test]
    fn sendfile_blocks_env_local() {
        let dir = tempfile::tempdir().unwrap();
        let env_file = dir.path().join(".env.local");
        std::fs::write(&env_file, "SECRET=hunter2").unwrap();
        assert!(is_sendfile_path_allowed(&env_file).is_err());
    }

    #[test]
    fn sendfile_blocks_env_production() {
        let dir = tempfile::tempdir().unwrap();
        let env_file = dir.path().join(".env.production");
        std::fs::write(&env_file, "SECRET=hunter2").unwrap();
        assert!(is_sendfile_path_allowed(&env_file).is_err());
    }

    #[test]
    fn sendfile_rejects_nonexistent_path() {
        let result = is_sendfile_path_allowed(std::path::Path::new("/no/such/file"));
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("cannot resolve"));
    }

    #[test]
    fn sendfile_allows_regular_project_file() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("report.txt");
        std::fs::write(&file, "report contents").unwrap();
        assert!(is_sendfile_path_allowed(&file).is_ok());
    }
}
