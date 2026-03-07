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

use clankers_auth::{
    Capability, CapabilityToken, TokenVerifier, RevocationStore, RedbRevocationStore,
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
pub(crate) struct ProactiveConfig {
    pub(crate) session_heartbeat_secs: u64,
    pub(crate) heartbeat_prompt: String,
    pub(crate) trigger_pipe_enabled: bool,
}

// ── Session tracking ────────────────────────────────────────────────────────

// ── Auth layer ──────────────────────────────────────────────────────────────

/// Shared auth state for token verification + user→token mappings.
pub(crate) struct AuthLayer {
    /// Verifier with the daemon owner's key as trusted root
    verifier: TokenVerifier,
    /// Persistent revocation store (redb-backed, used for runtime revocation checks)
    #[allow(dead_code)]
    revocation_store: RedbRevocationStore,
    /// redb database for token storage
    db: Arc<redb::Database>,
    /// Daemon owner's secret key (for signing delegated child tokens)
    pub(crate) owner_key: ::iroh::SecretKey,
}

impl AuthLayer {
    /// Look up a stored token for a user ID (Matrix user ID or iroh pubkey).
    pub(crate) fn lookup_token(&self, user_id: &str) -> Option<CapabilityToken> {
        let read_txn = self.db.begin_read().ok()?;
        let table = read_txn
            .open_table(clankers_auth::revocation::AUTH_TOKENS_TABLE)
            .ok()?;
        let guard = table.get(user_id).ok()??;
        let bytes = guard.value().to_vec();
        CapabilityToken::decode(&bytes).ok()
    }

    /// Store a token for a user ID.
    pub(crate) fn store_token(&self, user_id: &str, token: &CapabilityToken) {
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
    pub(crate) fn verify_token(&self, token: &CapabilityToken) -> std::result::Result<Vec<Capability>, String> {
        self.verifier
            .verify(token, None)
            .map_err(|e| format!("{e}"))?;
        Ok(token.capabilities.clone())
    }

    /// Resolve capabilities for a user: token → verify → capabilities,
    /// or None if no token (fall back to allowlist).
    pub(crate) fn resolve_capabilities(&self, user_id: &str) -> Option<std::result::Result<Vec<Capability>, String>> {
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
pub(crate) struct LiveSession {
    /// The agent with conversation history
    pub(crate) agent: Agent,
    /// Session persistence manager
    #[allow(dead_code)]
    pub(crate) session_mgr: Option<SessionManager>,
    /// When the session was last active
    pub(crate) last_active: chrono::DateTime<Utc>,
    /// Number of turns so far
    pub(crate) turn_count: usize,
    /// Deterministic working directory for this session
    pub(crate) session_dir: PathBuf,
    /// Cancellation token for the trigger pipe reader task
    pub(crate) trigger_cancel: Option<CancellationToken>,
    /// Capabilities from the user's token (None = full access via allowlist)
    #[allow(dead_code)]
    pub(crate) capabilities: Option<Vec<Capability>>,
    /// Tools available to this session (filtered by capabilities)
    pub(crate) session_tools: Vec<Arc<dyn Tool>>,
}

/// Identifies a session by transport + sender.
#[derive(Debug, Clone, Hash, Eq, PartialEq)]
pub(crate) enum SessionKey {
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
    pub(crate) fn dir_name(&self) -> String {
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
    pub(crate) fn matrix_room_id(&self) -> Option<&str> {
        match self {
            Self::Matrix { room_id, .. } => Some(room_id),
            _ => None,
        }
    }
}

/// Manages all active sessions.
pub(crate) struct SessionStore {
    pub(crate) sessions: HashMap<SessionKey, LiveSession>,
    /// Per-session locks to serialize prompt execution and prevent
    /// concurrent prompts from racing on conversation history.
    pub(crate) prompt_locks: HashMap<SessionKey, Arc<Mutex<()>>>,
    max_sessions: usize,
    /// Shared resources for creating new agents
    pub(crate) provider: Arc<dyn Provider>,
    pub(crate) tools: Vec<Arc<dyn Tool>>,
    pub(crate) settings: Settings,
    pub(crate) model: String,
    pub(crate) system_prompt: String,
    sessions_dir: PathBuf,
    /// Shared auth layer for token verification
    pub(crate) auth: Option<Arc<AuthLayer>>,
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
    pub(crate) fn prompt_lock(&mut self, key: &SessionKey) -> Arc<Mutex<()>> {
        self.prompt_locks.entry(key.clone()).or_insert_with(|| Arc::new(Mutex::new(()))).clone()
    }

    /// Get or create a session for the given key.
    ///
    /// If `capabilities` is Some, tools are filtered based on the token's
    /// ToolUse capability. If None, all tools are available (allowlist user).
    pub(crate) fn get_or_create(
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
                super::matrix_bridge::run_matrix_bridge(matrix_store, matrix_cancel, &matrix_paths, matrix_allowed, proactive_config).await
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
// Matrix bridge implementation moved to src/modes/matrix_bridge.rs
