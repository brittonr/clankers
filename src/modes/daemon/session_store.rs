//! Authentication and session catalog for daemon sessions.
//!
//! Two layers:
//! - **Auth**: Token verification, capability resolution, credential storage.
//! - **Catalog**: Persistent redb-backed index of daemon-managed sessions,
//!   their lifecycle state, and transport key mappings. Survives daemon restarts.

use std::path::PathBuf;
use std::sync::Arc;

use clankers_ucan::Credential;
use redb::ReadableTable;
use clankers_ucan::RedbRevocationStore;
use clankers_ucan::RevocationStore;
use clankers_ucan::TokenVerifier;
use clankers_ucan::Capability;
use tracing::info;
use tracing::warn;

// ── Auth layer ──────────────────────────────────────────────────────────────

/// Shared auth state for token verification + user→token mappings.
pub struct AuthLayer {
    /// Verifier with the daemon owner's key as trusted root
    verifier: TokenVerifier,
    /// Persistent revocation store (redb-backed)
    _revocation_store: RedbRevocationStore,
    /// redb database for token storage
    pub db: Arc<redb::Database>,
    /// Daemon owner's secret key (for signing delegated child tokens)
    pub owner_key: ::iroh::SecretKey,
}

impl AuthLayer {
    /// Look up a stored credential for a user ID (Matrix user ID or iroh pubkey).
    pub fn lookup_credential(&self, user_id: &str) -> Option<Credential> {
        let read_txn = self.db.begin_read().ok()?;
        let table = read_txn.open_table(clankers_ucan::revocation::AUTH_TOKENS_TABLE).ok()?;
        let guard = table.get(user_id).ok()??;
        let bytes = guard.value().to_vec();
        match Credential::decode(&bytes) {
            Ok(cred) => Some(cred),
            Err(e) => {
                warn!("Failed to decode credential for {user_id}, removing stale entry: {e}");
                // Remove the stale entry so we don't warn on every lookup
                drop(guard);
                drop(table);
                drop(read_txn);
                if let Ok(tx) = self.db.begin_write() {
                    if let Ok(mut table) = tx.open_table(clankers_ucan::revocation::AUTH_TOKENS_TABLE) {
                        let _ = table.remove(user_id);
                    }
                    let _ = tx.commit();
                }
                None
            }
        }
    }

    /// Store a credential for a user ID.
    pub fn store_credential(&self, user_id: &str, cred: &Credential) {
        let encoded = match cred.encode() {
            Ok(e) => e,
            Err(e) => {
                warn!("Failed to encode token for storage: {e}");
                return;
            }
        };
        if let Ok(tx) = self.db.begin_write() {
            {
                if let Ok(mut table) = tx.open_table(clankers_ucan::revocation::AUTH_TOKENS_TABLE) {
                    let _ = table.insert(user_id, encoded.as_slice());
                }
            }
            if let Err(e) = tx.commit() {
                warn!("Failed to store token: {e}");
            }
        }
    }

    /// Verify a credential and return its leaf token's capabilities, or an error message.
    pub fn verify_credential(&self, cred: &Credential) -> std::result::Result<Vec<Capability>, String> {
        self.verifier
            .verify_with_chain(&cred.token, &cred.proofs, None)
            .map_err(|e| format!("{e}"))?;
        Ok(cred.token.capabilities.clone())
    }

    /// Resolve capabilities for a user: credential → verify → capabilities,
    /// or None if no credential (fall back to allowlist).
    pub fn resolve_capabilities(&self, user_id: &str) -> Option<std::result::Result<Vec<Capability>, String>> {
        let cred = self.lookup_credential(user_id)?;
        Some(self.verify_credential(&cred))
    }
}

// ── Session catalog ─────────────────────────────────────────────────────────

/// redb table: session_id → JSON-encoded SessionCatalogEntry.
pub const SESSION_CATALOG_TABLE: redb::TableDefinition<&str, &[u8]> =
    redb::TableDefinition::new("session_catalog");

/// redb table: JSON-encoded SessionKey → session_id.
pub const SESSION_KEYS_TABLE: redb::TableDefinition<&str, &str> =
    redb::TableDefinition::new("session_keys");

/// Lifecycle state of a catalog entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SessionLifecycle {
    /// Actor is running.
    Active,
    /// Checkpointed, no actor — recoverable on demand.
    Suspended,
    /// Killed or expired — pending GC.
    Tombstoned,
}

impl std::fmt::Display for SessionLifecycle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Active => write!(f, "active"),
            Self::Suspended => write!(f, "suspended"),
            Self::Tombstoned => write!(f, "tombstoned"),
        }
    }
}

/// Persistent metadata for a daemon-managed session.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SessionCatalogEntry {
    pub session_id: String,
    pub automerge_path: PathBuf,
    pub model: String,
    pub created_at: String,
    pub last_active: String,
    pub turn_count: usize,
    pub state: SessionLifecycle,
}

/// Persistent session catalog backed by redb.
///
/// Shares the same `redb::Database` as the auth layer.
pub struct SessionCatalog {
    db: Arc<redb::Database>,
}

impl SessionCatalog {
    pub fn new(db: Arc<redb::Database>) -> Self {
        // Ensure tables exist
        if let Ok(tx) = db.begin_write() {
            let _ = tx.open_table(SESSION_CATALOG_TABLE);
            let _ = tx.open_table(SESSION_KEYS_TABLE);
            let _ = tx.commit();
        }
        Self { db }
    }

    // ── Catalog CRUD ────────────────────────────────────────────────

    pub fn insert_session(&self, entry: &SessionCatalogEntry) {
        let encoded = match serde_json::to_vec(entry) {
            Ok(e) => e,
            Err(e) => {
                warn!("catalog: failed to encode entry: {e}");
                return;
            }
        };
        if let Ok(tx) = self.db.begin_write() {
            {
                if let Ok(mut table) = tx.open_table(SESSION_CATALOG_TABLE) {
                    let _ = table.insert(entry.session_id.as_str(), encoded.as_slice());
                }
            }
            let _ = tx.commit();
        }
    }

    pub fn update_session(&self, entry: &SessionCatalogEntry) {
        // Same storage — insert overwrites.
        self.insert_session(entry);
    }

    pub fn get_session(&self, session_id: &str) -> Option<SessionCatalogEntry> {
        let tx = self.db.begin_read().ok()?;
        let table = tx.open_table(SESSION_CATALOG_TABLE).ok()?;
        let guard = table.get(session_id).ok()??;
        let bytes = guard.value().to_vec();
        serde_json::from_slice(&bytes).ok()
    }

    pub fn list_sessions(&self) -> Vec<SessionCatalogEntry> {
        let mut result = Vec::new();
        let Ok(tx) = self.db.begin_read() else {
            return result;
        };
        let Ok(table) = tx.open_table(SESSION_CATALOG_TABLE) else {
            return result;
        };
        let Ok(iter) = table.iter() else {
            return result;
        };
        for item in iter {
            let Ok((_key, value)) = item else { continue };
            let bytes = value.value().to_vec();
            if let Ok(entry) = serde_json::from_slice::<SessionCatalogEntry>(&bytes) {
                result.push(entry);
            }
        }
        result
    }

    pub fn list_by_state(&self, state: SessionLifecycle) -> Vec<SessionCatalogEntry> {
        self.list_sessions()
            .into_iter()
            .filter(|e| e.state == state)
            .collect()
    }

    pub fn remove_session(&self, session_id: &str) {
        if let Ok(tx) = self.db.begin_write() {
            {
                if let Ok(mut table) = tx.open_table(SESSION_CATALOG_TABLE) {
                    let _ = table.remove(session_id);
                }
            }
            let _ = tx.commit();
        }
        // Also remove any key index entries pointing to this session
        self.remove_keys_for_session(session_id);
    }

    /// Transition a session to a new lifecycle state.
    pub fn set_state(&self, session_id: &str, new_state: SessionLifecycle) {
        if let Some(mut entry) = self.get_session(session_id) {
            entry.state = new_state;
            self.update_session(&entry);
        }
    }

    /// Batch-transition all entries in `from_state` to `to_state`.
    /// Returns the number of entries transitioned.
    pub fn transition_all(&self, from_state: SessionLifecycle, to_state: SessionLifecycle) -> usize {
        let entries = self.list_by_state(from_state);
        let count = entries.len();
        for mut entry in entries {
            entry.state = to_state;
            self.update_session(&entry);
        }
        count
    }

    // ── Key index CRUD ──────────────────────────────────────────────

    /// Store a SessionKey → session_id mapping.
    pub fn insert_key(&self, key: &clankers_protocol::SessionKey, session_id: &str) {
        let key_json = match serde_json::to_string(key) {
            Ok(k) => k,
            Err(e) => {
                warn!("catalog: failed to encode session key: {e}");
                return;
            }
        };
        if let Ok(tx) = self.db.begin_write() {
            {
                if let Ok(mut table) = tx.open_table(SESSION_KEYS_TABLE) {
                    let _ = table.insert(key_json.as_str(), session_id);
                }
            }
            let _ = tx.commit();
        }
    }

    /// Look up a session ID by transport key.
    pub fn lookup_key(&self, key: &clankers_protocol::SessionKey) -> Option<String> {
        let key_json = serde_json::to_string(key).ok()?;
        let tx = self.db.begin_read().ok()?;
        let table = tx.open_table(SESSION_KEYS_TABLE).ok()?;
        let guard = table.get(key_json.as_str()).ok()??;
        Some(guard.value().to_string())
    }

    /// Remove all key index entries pointing to a session.
    pub fn remove_keys_for_session(&self, session_id: &str) {
        // Collect keys to remove (can't mutate while iterating)
        let keys_to_remove: Vec<String> = {
            let Ok(tx) = self.db.begin_read() else { return };
            let Ok(table) = tx.open_table(SESSION_KEYS_TABLE) else { return };
            let Ok(iter) = table.iter() else { return };
            let mut keys = Vec::new();
            for item in iter {
                let Ok((key, value)) = item else { continue };
                if value.value() == session_id {
                    keys.push(key.value().to_string());
                }
            }
            keys
        };
        if keys_to_remove.is_empty() {
            return;
        }
        if let Ok(tx) = self.db.begin_write() {
            {
                if let Ok(mut table) = tx.open_table(SESSION_KEYS_TABLE) {
                    for key in &keys_to_remove {
                        let _ = table.remove(key.as_str());
                    }
                }
            }
            let _ = tx.commit();
        }
    }

    /// List all key → session_id mappings.
    pub fn list_keys(&self) -> Vec<(clankers_protocol::SessionKey, String)> {
        let mut result = Vec::new();
        let Ok(tx) = self.db.begin_read() else { return result };
        let Ok(table) = tx.open_table(SESSION_KEYS_TABLE) else { return result };
        let Ok(iter) = table.iter() else { return result };
        for item in iter {
            let Ok((key, value)) = item else { continue };
            let key_str = key.value().to_string();
            let session_id = value.value().to_string();
            if let Ok(session_key) = serde_json::from_str::<clankers_protocol::SessionKey>(&key_str) {
                result.push((session_key, session_id));
            }
        }
        result
    }

    // ── Garbage collection ──────────────────────────────────────────

    /// Remove tombstoned entries older than `retention`. Returns count removed.
    pub fn gc_tombstoned(&self, retention: std::time::Duration) -> usize {
        let cutoff = chrono::Utc::now() - chrono::Duration::from_std(retention).unwrap_or_default();
        let tombstoned = self.list_by_state(SessionLifecycle::Tombstoned);
        let mut removed = 0;
        for entry in tombstoned {
            if let Ok(ts) = chrono::DateTime::parse_from_rfc3339(&entry.last_active) {
                if ts < cutoff {
                    self.remove_session(&entry.session_id);
                    removed += 1;
                }
            }
        }
        removed
    }
}

/// Create the session catalog, or None if the database can't be opened.
pub fn create_session_catalog(db_path: &std::path::Path) -> Option<Arc<SessionCatalog>> {
    std::fs::create_dir_all(db_path.parent()?).ok();
    match redb::Database::create(db_path) {
        Ok(db) => {
            let catalog = Arc::new(SessionCatalog::new(Arc::new(db)));
            info!("Session catalog initialized at {}", db_path.display());
            Some(catalog)
        }
        Err(e) => {
            warn!("Failed to open catalog database: {e} — session recovery disabled");
            None
        }
    }
}

// ── Auth layer ──────────────────────────────────────────────────────────────

/// Create the auth layer if possible, or None if auth database setup fails.
pub fn create_auth_layer(
    db_path: &std::path::Path,
    identity: &crate::modes::rpc::iroh::Identity,
) -> Option<Arc<AuthLayer>> {
    std::fs::create_dir_all(db_path.parent()?).ok();
    match redb::Database::create(db_path) {
        Ok(db) => {
            let db = Arc::new(db);
            // Ensure auth tables exist
            if let Ok(tx) = db.begin_write() {
                let _ = tx.open_table(clankers_ucan::revocation::AUTH_TOKENS_TABLE);
                let _ = tx.open_table(clankers_ucan::revocation::REVOKED_TOKENS_TABLE);
                let _ = tx.commit();
            }

            let revocation_store = match RedbRevocationStore::new(Arc::clone(&db)) {
                Ok(s) => s,
                Err(e) => {
                    warn!("Failed to init revocation store: {e}");
                    RedbRevocationStore::new(Arc::clone(&db)).expect("retry")
                }
            };

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
