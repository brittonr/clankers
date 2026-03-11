//! Session storage and authentication layer.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use chrono::Utc;
use clankers_auth::Capability;
use clankers_auth::CapabilityToken;
use clankers_auth::RedbRevocationStore;
use clankers_auth::RevocationStore;
use clankers_auth::TokenVerifier;
use tokio::sync::Mutex;
use tokio_util::sync::CancellationToken;
use tracing::info;
use tracing::warn;

use crate::agent::Agent;
use crate::config::settings::Settings;
use crate::provider::Provider;
use crate::session::SessionManager;
use crate::tools::Tool;

// ── Auth layer ──────────────────────────────────────────────────────────────

/// Shared auth state for token verification + user→token mappings.
pub(crate) struct AuthLayer {
    /// Verifier with the daemon owner's key as trusted root
    verifier: TokenVerifier,
    /// Persistent revocation store (redb-backed, used for runtime revocation checks)
    _revocation_store: RedbRevocationStore,
    /// redb database for token storage
    db: Arc<redb::Database>,
    /// Daemon owner's secret key (for signing delegated child tokens)
    pub(crate) owner_key: ::iroh::SecretKey,
}

impl AuthLayer {
    /// Look up a stored token for a user ID (Matrix user ID or iroh pubkey).
    pub(crate) fn lookup_token(&self, user_id: &str) -> Option<CapabilityToken> {
        let read_txn = self.db.begin_read().ok()?;
        let table = read_txn.open_table(clankers_auth::revocation::AUTH_TOKENS_TABLE).ok()?;
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
        self.verifier.verify(token, None).map_err(|e| format!("{e}"))?;
        Ok(token.capabilities.clone())
    }

    /// Resolve capabilities for a user: token → verify → capabilities,
    /// or None if no token (fall back to allowlist).
    pub(crate) fn resolve_capabilities(&self, user_id: &str) -> Option<std::result::Result<Vec<Capability>, String>> {
        let token = self.lookup_token(user_id)?;
        Some(self.verify_token(&token))
    }
}

/// Create the auth layer if possible, or None if auth database setup fails.
pub(crate) fn create_auth_layer(
    db_path: &std::path::Path,
    identity: &crate::modes::rpc::iroh::Identity,
) -> Option<Arc<AuthLayer>> {
    std::fs::create_dir_all(db_path.parent()?).ok();
    match redb::Database::create(db_path) {
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
            let verifier = TokenVerifier::new().with_trusted_root(identity.public_key());
            if !revoked.is_empty() {
                if let Err(e) = verifier.load_revoked(&revoked) {
                    warn!("Failed to load revoked tokens: {e}");
                }
                info!("Loaded {} revoked token(s)", revoked.len());
            }

            let layer = Arc::new(AuthLayer {
                verifier,
                _revocation_store: revocation_store,
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
}

/// Filter a tool set based on capability tokens.
///
/// If capabilities include `ToolUse { "*" }`, all tools are kept.
/// Otherwise, only tools whose name appears in the ToolUse pattern are kept.
pub(crate) fn filter_tools_by_capabilities(tools: &[Arc<dyn Tool>], capabilities: &[Capability]) -> Vec<Arc<dyn Tool>> {
    let tool_pattern = capabilities.iter().find_map(|c| {
        if let Capability::ToolUse { tool_pattern } = c {
            Some(tool_pattern.as_str())
        } else {
            None
        }
    });

    let result = match tool_pattern {
        None => Vec::new(),          // No ToolUse capability → no tools
        Some("*") => tools.to_vec(), // Wildcard → all tools
        Some(pattern) => {
            let allowed: std::collections::HashSet<&str> = pattern.split(',').map(|s| s.trim()).collect();
            tools.iter().filter(|t| allowed.contains(t.definition().name.as_str())).cloned().collect()
        }
    };

    // Tiger Style: result can never exceed input
    debug_assert!(result.len() <= tools.len());
    result
}

// ── Session types ───────────────────────────────────────────────────────────

/// A live agent session for a specific sender.
pub(crate) struct LiveSession {
    /// The agent with conversation history
    pub(crate) agent: Agent,
    /// Session persistence manager (owned for lifetime, persistence happens via agent events)
    pub(crate) _session_mgr: Option<SessionManager>,
    /// When the session was last active
    pub(crate) last_active: chrono::DateTime<Utc>,
    /// Number of turns so far
    pub(crate) turn_count: usize,
    /// Deterministic working directory for this session
    pub(crate) session_dir: PathBuf,
    /// Cancellation token for the trigger pipe reader task
    pub(crate) trigger_cancel: Option<CancellationToken>,
    /// Capabilities from the user's token (None = full access via allowlist, stored for inspection)
    pub(crate) _capabilities: Option<Vec<Capability>>,
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
    pub(crate) fn new(
        provider: Arc<dyn Provider>,
        tools: Vec<Arc<dyn Tool>>,
        settings: Settings,
        model: String,
        system_prompt: String,
        sessions_dir: PathBuf,
        max_sessions: usize,
        auth: Option<Arc<AuthLayer>>,
    ) -> Self {
        debug_assert!(max_sessions > 0, "max_sessions must be positive");
        debug_assert!(!model.is_empty(), "model must not be empty");

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
    pub(crate) fn get_or_create(&mut self, key: &SessionKey, capabilities: Option<&[Capability]>) -> &mut LiveSession {
        if !self.sessions.contains_key(key) {
            // Tiger Style: sessions must not exceed max_sessions
            debug_assert!(self.sessions.len() <= self.max_sessions);

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
                _session_mgr: session_mgr,
                last_active: Utc::now(),
                turn_count: 0,
                session_dir,
                trigger_cancel: None,
                _capabilities: capabilities.map(|c| c.to_vec()),
                session_tools,
            });
        }

        let session = self.sessions.get_mut(key).expect("just inserted");
        session.last_active = Utc::now();
        session
    }

    /// Remove the least recently used session.
    fn evict_oldest(&mut self) {
        debug_assert!(!self.sessions.is_empty(), "evict_oldest called on empty store");
        let before = self.sessions.len();
        if let Some(oldest_key) = self.sessions.iter().min_by_key(|(_, s)| s.last_active).map(|(k, _)| k.clone()) {
            info!("Evicting stale session: {}", oldest_key);
            self.sessions.remove(&oldest_key);
            self.prompt_locks.remove(&oldest_key);
        }
        debug_assert!(self.sessions.len() < before, "evict_oldest must remove a session");
    }

    /// Number of active sessions.
    pub(crate) fn len(&self) -> usize {
        self.sessions.len()
    }

    /// Reap sessions that have been idle longer than `max_idle`.
    /// Returns the number of reaped sessions.
    pub(crate) fn reap_idle(&mut self, max_idle: std::time::Duration) -> usize {
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
                    && let Err(e) = std::fs::remove_file(&pipe_path)
                {
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
