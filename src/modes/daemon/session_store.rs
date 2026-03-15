//! Authentication layer for daemon sessions.
//!
//! Token verification, capability resolution, and persistent storage.
//! The session management types (`SessionStore`, `LiveSession`, `SessionKey`)
//! have been replaced by the actor-based system in `agent_process.rs` and
//! `DaemonState` in `clankers-controller/transport.rs`.

use std::sync::Arc;

use clankers_auth::Credential;
use clankers_auth::RedbRevocationStore;
use clankers_auth::RevocationStore;
use clankers_auth::TokenVerifier;
use clankers_auth::Capability;
use tracing::info;
use tracing::warn;

// ── Auth layer ──────────────────────────────────────────────────────────────

/// Shared auth state for token verification + user→token mappings.
pub(crate) struct AuthLayer {
    /// Verifier with the daemon owner's key as trusted root
    verifier: TokenVerifier,
    /// Persistent revocation store (redb-backed)
    _revocation_store: RedbRevocationStore,
    /// redb database for token storage
    db: Arc<redb::Database>,
    /// Daemon owner's secret key (for signing delegated child tokens)
    pub(crate) owner_key: ::iroh::SecretKey,
}

impl AuthLayer {
    /// Look up a stored credential for a user ID (Matrix user ID or iroh pubkey).
    pub(crate) fn lookup_credential(&self, user_id: &str) -> Option<Credential> {
        let read_txn = self.db.begin_read().ok()?;
        let table = read_txn.open_table(clankers_auth::revocation::AUTH_TOKENS_TABLE).ok()?;
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
                    if let Ok(mut table) = tx.open_table(clankers_auth::revocation::AUTH_TOKENS_TABLE) {
                        let _ = table.remove(user_id);
                    }
                    let _ = tx.commit();
                }
                None
            }
        }
    }

    /// Store a credential for a user ID.
    pub(crate) fn store_credential(&self, user_id: &str, cred: &Credential) {
        let encoded = match cred.encode() {
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

    /// Verify a credential and return its leaf token's capabilities, or an error message.
    pub(crate) fn verify_credential(&self, cred: &Credential) -> std::result::Result<Vec<Capability>, String> {
        self.verifier
            .verify_with_chain(&cred.token, &cred.proofs, None)
            .map_err(|e| format!("{e}"))?;
        Ok(cred.token.capabilities.clone())
    }

    /// Resolve capabilities for a user: credential → verify → capabilities,
    /// or None if no credential (fall back to allowlist).
    pub(crate) fn resolve_capabilities(&self, user_id: &str) -> Option<std::result::Result<Vec<Capability>, String>> {
        let cred = self.lookup_credential(user_id)?;
        Some(self.verify_credential(&cred))
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
