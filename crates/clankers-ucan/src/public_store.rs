//! Redb-backed public UCAN credential, revocation, and replay state.

use std::sync::Arc;
use std::time::SystemTime;

use redb::Database;
use redb::ReadableTable;
use redb::TableDefinition;

use crate::public_credential::PublicCredentialEnvelope;
use crate::public_credential::PublicCredentialError;

/// Table: user/peer id -> encoded [`PublicCredentialEnvelope`] JSON bytes.
pub const PUBLIC_AUTH_TOKENS_TABLE: TableDefinition<&str, &[u8]> = TableDefinition::new("public_auth_tokens");

/// Table: UCAN proof reference bytes -> revocation timestamp.
pub const PUBLIC_REVOKED_REFERENCES_TABLE: TableDefinition<&[u8], u64> =
    TableDefinition::new("public_revoked_references");

/// Table: replay key -> admission timestamp.
pub const PUBLIC_REPLAY_TABLE: TableDefinition<&str, u64> = TableDefinition::new("public_replay_admissions");

#[derive(Clone)]
pub struct RedbPublicCredentialStore {
    db: Arc<Database>,
}

impl RedbPublicCredentialStore {
    #[allow(clippy::result_large_err)]
    pub fn new(db: Arc<Database>) -> Result<Self, redb::Error> {
        let tx = db.begin_write()?;
        {
            let _ = tx.open_table(PUBLIC_AUTH_TOKENS_TABLE)?;
            let _ = tx.open_table(PUBLIC_REVOKED_REFERENCES_TABLE)?;
            let _ = tx.open_table(PUBLIC_REPLAY_TABLE)?;
        }
        tx.commit()?;
        Ok(Self { db })
    }

    #[must_use]
    pub const fn db(&self) -> &Arc<Database> {
        &self.db
    }

    pub fn lookup_credential(&self, user_id: &str) -> Option<PublicCredentialEnvelope> {
        let tx = self.db.begin_read().ok()?;
        let table = tx.open_table(PUBLIC_AUTH_TOKENS_TABLE).ok()?;
        let guard = table.get(user_id).ok()??;
        let bytes = guard.value().to_vec();
        PublicCredentialEnvelope::decode(bytes.as_slice()).ok()
    }

    pub fn store_credential(
        &self,
        user_id: &str,
        envelope: &PublicCredentialEnvelope,
    ) -> Result<(), PublicCredentialStoreError> {
        let encoded = envelope.encode().map_err(PublicCredentialStoreError::Credential)?;
        let tx = self.db.begin_write().map_err(|source| PublicCredentialStoreError::Backend {
            message: source.to_string(),
        })?;
        {
            let mut table =
                tx.open_table(PUBLIC_AUTH_TOKENS_TABLE).map_err(|source| PublicCredentialStoreError::Backend {
                    message: source.to_string(),
                })?;
            table.insert(user_id, encoded.as_slice()).map_err(|source| PublicCredentialStoreError::Backend {
                message: source.to_string(),
            })?;
        }
        tx.commit().map_err(|source| PublicCredentialStoreError::Backend {
            message: source.to_string(),
        })?;
        Ok(())
    }

    pub fn remove_credential(&self, user_id: &str) -> Result<(), PublicCredentialStoreError> {
        let tx = self.db.begin_write().map_err(|source| PublicCredentialStoreError::Backend {
            message: source.to_string(),
        })?;
        {
            let mut table =
                tx.open_table(PUBLIC_AUTH_TOKENS_TABLE).map_err(|source| PublicCredentialStoreError::Backend {
                    message: source.to_string(),
                })?;
            table.remove(user_id).map_err(|source| PublicCredentialStoreError::Backend {
                message: source.to_string(),
            })?;
        }
        tx.commit().map_err(|source| PublicCredentialStoreError::Backend {
            message: source.to_string(),
        })?;
        Ok(())
    }

    pub fn revoke_reference(&self, reference: &ucan::ProofReference) -> Result<(), PublicCredentialStoreError> {
        let tx = self.db.begin_write().map_err(|source| PublicCredentialStoreError::Backend {
            message: source.to_string(),
        })?;
        {
            let mut table = tx.open_table(PUBLIC_REVOKED_REFERENCES_TABLE).map_err(|source| {
                PublicCredentialStoreError::Backend {
                    message: source.to_string(),
                }
            })?;
            table.insert(reference.as_bytes(), now_unix_seconds()).map_err(|source| {
                PublicCredentialStoreError::Backend {
                    message: source.to_string(),
                }
            })?;
        }
        tx.commit().map_err(|source| PublicCredentialStoreError::Backend {
            message: source.to_string(),
        })?;
        Ok(())
    }

    pub fn admit_credential_replay(
        &self,
        envelope: &PublicCredentialEnvelope,
    ) -> Result<ReplayAdmissionStatus, PublicCredentialStoreError> {
        let Some(replay_id) = envelope.replay_id() else {
            return Ok(ReplayAdmissionStatus::NotPresent);
        };
        self.insert_replay_key(format!("credential:{}:{replay_id}", envelope.token_reference()))
    }

    fn insert_replay_key(&self, key: String) -> Result<ReplayAdmissionStatus, PublicCredentialStoreError> {
        let tx = self.db.begin_write().map_err(|source| PublicCredentialStoreError::Backend {
            message: source.to_string(),
        })?;
        {
            let mut table =
                tx.open_table(PUBLIC_REPLAY_TABLE).map_err(|source| PublicCredentialStoreError::Backend {
                    message: source.to_string(),
                })?;
            if table
                .get(key.as_str())
                .map_err(|source| PublicCredentialStoreError::Backend {
                    message: source.to_string(),
                })?
                .is_some()
            {
                return Ok(ReplayAdmissionStatus::Duplicate);
            }
            table
                .insert(key.as_str(), now_unix_seconds())
                .map_err(|source| PublicCredentialStoreError::Backend {
                    message: source.to_string(),
                })?;
        }
        tx.commit().map_err(|source| PublicCredentialStoreError::Backend {
            message: source.to_string(),
        })?;
        Ok(ReplayAdmissionStatus::Accepted)
    }
}

impl ucan::RevocationChecker for RedbPublicCredentialStore {
    fn is_revoked(&self, reference: &ucan::ProofReference) -> std::result::Result<bool, ucan::RevocationError> {
        let tx = self.db.begin_read().map_err(|source| ucan::RevocationError::Backend {
            reference: reference.clone(),
            message: source.to_string(),
        })?;
        let table =
            tx.open_table(PUBLIC_REVOKED_REFERENCES_TABLE).map_err(|source| ucan::RevocationError::Backend {
                reference: reference.clone(),
                message: source.to_string(),
            })?;
        table
            .get(reference.as_bytes())
            .map(|entry| entry.is_some())
            .map_err(|source| ucan::RevocationError::Backend {
                reference: reference.clone(),
                message: source.to_string(),
            })
    }
}

impl ucan::ReplayAdmission for RedbPublicCredentialStore {
    fn admit_invocation(
        &self,
        token_reference: &ucan::ProofReference,
        resource: &str,
        ability: &str,
    ) -> std::result::Result<(), ucan::ReplayAdmissionError> {
        match self.insert_replay_key(format!("invocation:{token_reference}:{resource}:{ability}")) {
            Ok(ReplayAdmissionStatus::Accepted | ReplayAdmissionStatus::NotPresent) => Ok(()),
            Ok(ReplayAdmissionStatus::Duplicate) => Err(ucan::ReplayAdmissionError::Duplicate {
                reference: token_reference.clone(),
            }),
            Err(error) => Err(ucan::ReplayAdmissionError::Backend {
                reference: token_reference.clone(),
                message: error.to_string(),
            }),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReplayAdmissionStatus {
    NotPresent,
    Accepted,
    Duplicate,
}

#[derive(Debug)]
pub enum PublicCredentialStoreError {
    Credential(PublicCredentialError),
    Backend { message: String },
}

impl std::fmt::Display for PublicCredentialStoreError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Credential(error) => write!(formatter, "{error}"),
            Self::Backend { message } => write!(formatter, "public credential store backend error: {message}"),
        }
    }
}

impl std::error::Error for PublicCredentialStoreError {}

#[cfg_attr(
    dylint_lib = "tigerstyle",
    allow(
        tigerstyle::ambient_clock,
        reason = "replay admission storage is an imperative shell boundary that timestamps accepted credential references"
    )
)]
fn now_unix_seconds() -> u64 {
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::time::Duration;

    use ucan::CapabilitySet;
    use ucan::VerificationTime;

    use super::*;
    use crate::public_issuer::PublicUcanIssuer;

    const ROOT_KEY_BYTE: u8 = 61;
    const SESSION_KEY_BYTE: u8 = 67;
    const RESOURCE: &str = "clankers:session/demo";
    const ABILITY: &str = "session/attach";

    fn store() -> (tempfile::TempDir, RedbPublicCredentialStore) {
        let tmp = tempfile::tempdir().expect("tempdir");
        let db = Arc::new(redb::Database::create(tmp.path().join("auth.db")).expect("db"));
        let store = RedbPublicCredentialStore::new(db).expect("store");
        (tmp, store)
    }

    fn issuer(byte: u8) -> PublicUcanIssuer {
        PublicUcanIssuer::from_signer(ucan::Ed25519InMemorySigner::from_seed_bytes(
            [byte; ucan::ED25519_SECRET_KEY_BYTES],
        ))
    }

    fn envelope() -> PublicCredentialEnvelope {
        let root = issuer(ROOT_KEY_BYTE);
        let session = issuer(SESSION_KEY_BYTE);
        root.issue_root_credential(
            session.audience().expect("audience"),
            CapabilitySet::single(RESOURCE, ABILITY).expect("capability"),
            Duration::from_secs(60),
        )
        .expect("envelope")
        .with_replay_id("nonce-1")
    }

    #[test]
    fn credential_storage_round_trips_public_envelope() {
        let (_tmp, store) = store();
        let envelope = envelope();

        store.store_credential("peer", &envelope).expect("store");
        let loaded = store.lookup_credential("peer").expect("loaded");

        assert_eq!(loaded.token_reference(), envelope.token_reference());
        assert_eq!(loaded.replay_id(), Some("nonce-1"));
    }

    #[test]
    fn revocation_checker_rejects_revoked_public_reference() {
        let (_tmp, store) = store();
        let envelope = envelope();
        store.revoke_reference(&envelope.token_reference()).expect("revoke");

        let error = envelope
            .verify_with_did_keys_and_revocations(
                VerificationTime::try_from_system_time(SystemTime::now()).expect("time"),
                &store,
            )
            .expect_err("revoked");

        assert!(matches!(error, PublicCredentialError::Verify { .. }));
    }

    #[test]
    fn credential_replay_is_admitted_once() {
        let (_tmp, store) = store();
        let envelope = envelope();

        assert_eq!(store.admit_credential_replay(&envelope).expect("first"), ReplayAdmissionStatus::Accepted);
        assert_eq!(store.admit_credential_replay(&envelope).expect("second"), ReplayAdmissionStatus::Duplicate);
    }
}
