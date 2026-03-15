//! Integration tests for the Credential auth flow.
//!
//! Tests the full path: create credential → encode to base64 → decode →
//! store in AuthLayer → look up → verify chain → extract capabilities.
//! This exercises the same code paths as the QUIC handshake and !token
//! bot command without needing a running daemon.

use std::sync::Arc;
use std::time::Duration;

use clankers_auth::Capability;
use clankers_auth::Credential;
use clankers_auth::TokenBuilder;

/// Create an AuthLayer backed by a temp redb database.
///
/// Returns (auth_layer, _tmpdir) — hold onto _tmpdir to keep the database alive.
fn test_auth_layer(
    owner_key: &iroh::SecretKey,
) -> (
    Arc<clankers::modes::daemon::session_store::AuthLayer>,
    tempfile::TempDir,
) {
    let tmp = tempfile::tempdir().unwrap();
    let db_path = tmp.path().join("auth.db");
    let key_path = tmp.path().join("identity.key");
    let identity = clankers::modes::rpc::iroh::Identity {
        secret_key: owner_key.clone(),
        path: key_path,
    };
    let auth = clankers::modes::daemon::session_store::create_auth_layer(&db_path, &identity)
        .expect("auth layer should initialize");
    (auth, tmp)
}

#[test]
fn root_credential_roundtrip_through_auth_layer() {
    let owner_sk = iroh::SecretKey::generate(&mut rand::rng());
    let (auth, _tmp) = test_auth_layer(&owner_sk);

    // Owner creates a root credential
    let token = TokenBuilder::new(owner_sk)
        .with_capability(Capability::Prompt)
        .with_capability(Capability::ToolUse {
            tool_pattern: "read,grep".into(),
        })
        .with_lifetime(Duration::from_secs(3600))
        .with_random_nonce()
        .build()
        .unwrap();
    let cred = Credential::from_root(token);

    // Encode to base64 (what goes over the wire)
    let b64 = cred.to_base64().unwrap();

    // Decode (what the handler does on receive)
    let decoded = Credential::from_base64(&b64).unwrap();

    // Verify (AuthLayer path)
    let caps = auth.verify_credential(&decoded).unwrap();
    assert_eq!(caps.len(), 2);

    // Store and look up (persistence path)
    let user_id = "test-user-1";
    auth.store_credential(user_id, &decoded);
    let looked_up = auth.lookup_credential(user_id).unwrap();
    assert_eq!(looked_up.token.capabilities.len(), 2);
    assert!(looked_up.proofs.is_empty());
}

#[test]
fn delegated_credential_chain_verification() {
    let owner_sk = iroh::SecretKey::generate(&mut rand::rng());
    let (auth, _tmp) = test_auth_layer(&owner_sk);

    // Owner creates root credential
    let root_token = TokenBuilder::new(owner_sk)
        .with_capability(Capability::Prompt)
        .with_capability(Capability::ToolUse {
            tool_pattern: "*".into(),
        })
        .with_capability(Capability::Delegate)
        .with_lifetime(Duration::from_secs(3600))
        .with_random_nonce()
        .build()
        .unwrap();
    let root_cred = Credential::from_root(root_token);

    // Owner delegates to Alice with narrower scope (bearer — no audience key)
    let alice_sk = iroh::SecretKey::generate(&mut rand::rng());
    let alice_cred = root_cred
        .delegate_bearer(
            &alice_sk,
            vec![
                Capability::Prompt,
                Capability::ToolUse {
                    tool_pattern: "read,grep".into(),
                },
                Capability::Delegate,
            ],
            Duration::from_secs(1800),
        )
        .unwrap();

    assert_eq!(alice_cred.proofs.len(), 1);
    assert_eq!(alice_cred.token.delegation_depth, 1);

    // Alice's credential goes over the wire as base64
    let b64 = alice_cred.to_base64().unwrap();
    let decoded = Credential::from_base64(&b64).unwrap();

    // AuthLayer verifies the full chain (no presenter — bearer token)
    let caps = auth.verify_credential(&decoded).unwrap();
    assert!(caps.iter().any(|c| matches!(c, Capability::Prompt)));
    assert!(caps.iter().any(|c| matches!(c, Capability::ToolUse { .. })));

    // Store and look up
    auth.store_credential("alice", &decoded);
    let resolved = auth.resolve_capabilities("alice").unwrap().unwrap();
    assert_eq!(resolved.len(), 3);
}

#[test]
fn two_level_delegation_chain() {
    let owner_sk = iroh::SecretKey::generate(&mut rand::rng());
    let (auth, _tmp) = test_auth_layer(&owner_sk);

    // Root → Alice → Bob
    let root_token = TokenBuilder::new(owner_sk)
        .with_capability(Capability::Prompt)
        .with_capability(Capability::ToolUse {
            tool_pattern: "*".into(),
        })
        .with_capability(Capability::ShellExecute {
            command_pattern: "*".into(),
            working_dir: None,
        })
        .with_capability(Capability::Delegate)
        .with_lifetime(Duration::from_secs(3600))
        .with_random_nonce()
        .build()
        .unwrap();
    let root_cred = Credential::from_root(root_token);

    let alice_sk = iroh::SecretKey::generate(&mut rand::rng());
    let alice_cred = root_cred
        .delegate_bearer(
            &alice_sk,
            vec![
                Capability::Prompt,
                Capability::ToolUse {
                    tool_pattern: "read,grep,find".into(),
                },
                Capability::Delegate,
            ],
            Duration::from_secs(1800),
        )
        .unwrap();

    let bob_sk = iroh::SecretKey::generate(&mut rand::rng());
    let bob_cred = alice_cred
        .delegate_bearer(
            &bob_sk,
            vec![Capability::Prompt, Capability::ToolUse {
                tool_pattern: "read".into(),
            }],
            Duration::from_secs(900),
        )
        .unwrap();

    assert_eq!(bob_cred.proofs.len(), 2);
    assert_eq!(bob_cred.token.delegation_depth, 2);

    // Full round-trip through base64
    let b64 = bob_cred.to_base64().unwrap();
    let decoded = Credential::from_base64(&b64).unwrap();

    // AuthLayer verifies the 2-level chain back to the trusted root
    let caps = auth.verify_credential(&decoded).unwrap();
    assert_eq!(caps.len(), 2);
    assert!(caps.iter().any(|c| matches!(c, Capability::Prompt)));
}

#[test]
fn untrusted_root_rejected() {
    let owner_sk = iroh::SecretKey::generate(&mut rand::rng());
    let (auth, _tmp) = test_auth_layer(&owner_sk);

    // Some other key creates a credential — not the owner
    let rando_sk = iroh::SecretKey::generate(&mut rand::rng());
    let token = TokenBuilder::new(rando_sk)
        .with_capability(Capability::Prompt)
        .with_lifetime(Duration::from_secs(3600))
        .build()
        .unwrap();
    let cred = Credential::from_root(token);

    let result = auth.verify_credential(&cred);
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("untrusted"));
}

#[test]
fn broken_chain_rejected() {
    let owner_sk = iroh::SecretKey::generate(&mut rand::rng());
    let (auth, _tmp) = test_auth_layer(&owner_sk);

    // Create a valid root credential
    let root_token = TokenBuilder::new(owner_sk)
        .with_capability(Capability::Prompt)
        .with_capability(Capability::Delegate)
        .with_lifetime(Duration::from_secs(3600))
        .build()
        .unwrap();

    // Create a child delegated from root
    let child_sk = iroh::SecretKey::generate(&mut rand::rng());
    let child_token = TokenBuilder::new(child_sk)
        .with_capability(Capability::Prompt)
        .delegated_from(root_token)
        .build()
        .unwrap();

    // Substitute a fake parent in the proof chain
    let fake_sk = iroh::SecretKey::generate(&mut rand::rng());
    let fake_token = TokenBuilder::new(fake_sk)
        .with_capability(Capability::Prompt)
        .with_lifetime(Duration::from_secs(3600))
        .build()
        .unwrap();

    let broken_cred = Credential {
        token: child_token,
        proofs: vec![fake_token],
    };

    let result = auth.verify_credential(&broken_cred);
    assert!(result.is_err());
}

#[test]
fn stale_entry_cleaned_on_decode_failure() {
    let owner_sk = iroh::SecretKey::generate(&mut rand::rng());
    let (auth, _tmp) = test_auth_layer(&owner_sk);

    // Manually write garbage to the auth table (simulates old format)
    {
        let db = &auth.db;
        let tx = db.begin_write().unwrap();
        {
            let mut table = tx
                .open_table(clankers_auth::revocation::AUTH_TOKENS_TABLE)
                .unwrap();
            table.insert("stale-user", b"this is not a valid credential".as_slice()).unwrap();
        }
        tx.commit().unwrap();
    }

    // Lookup should return None and clean the entry
    assert!(auth.lookup_credential("stale-user").is_none());

    // Verify the stale entry was removed
    {
        let db = &auth.db;
        let tx = db.begin_read().unwrap();
        let table = tx
            .open_table(clankers_auth::revocation::AUTH_TOKENS_TABLE)
            .unwrap();
        assert!(table.get("stale-user").unwrap().is_none());
    }
}

#[test]
fn resolve_capabilities_returns_none_for_unknown_user() {
    let owner_sk = iroh::SecretKey::generate(&mut rand::rng());
    let (auth, _tmp) = test_auth_layer(&owner_sk);

    // No credential stored — should return None (allowlist fallback path)
    assert!(auth.resolve_capabilities("nobody").is_none());
}
