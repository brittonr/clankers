//! Integration tests for the public UCAN + Basalt auth flow.

use std::sync::Arc;
use std::time::Duration;
use std::time::SystemTime;

use clankers_ucan::BasaltAdmissionRequest;
use clankers_ucan::PublicCredentialEnvelope;
use clankers_ucan::PublicUcanIssuer;
use ucan::AudienceDid;
use ucan::CapabilityDocument;
use ucan::CapabilitySet;
use ucan::ProofCollection;
use ucan::TokenSigner;
use ucan::TokenTimeBounds;
use ucan::issue_token_with_signer;

fn test_auth_layer(
    owner_key: &iroh::SecretKey,
) -> (Arc<clankers::modes::daemon::session_store::AuthLayer>, tempfile::TempDir) {
    let tmp = tempfile::tempdir().unwrap();
    let db_path = tmp.path().join("auth.db");
    let key_path = tmp.path().join("identity.key");
    let identity = clankers::modes::rpc::iroh::Identity {
        secret_key: owner_key.clone(),
        path: key_path,
    };
    let db = clankers::modes::daemon::session_store::open_daemon_db(&db_path).expect("db should open");
    let auth = clankers::modes::daemon::session_store::create_auth_layer(&db, &identity)
        .expect("auth layer should initialize");
    (auth, tmp)
}

fn issuer(byte: u8) -> PublicUcanIssuer {
    PublicUcanIssuer::from_signer(ucan::Ed25519InMemorySigner::from_seed_bytes([byte; ucan::ED25519_SECRET_KEY_BYTES]))
}

fn cap(resource: &str, ability: &str) -> CapabilityDocument {
    CapabilityDocument::new(resource.to_owned(), ability.to_owned()).expect("capability")
}

fn caps(items: Vec<CapabilityDocument>) -> CapabilitySet {
    CapabilitySet::new(items).expect("capability set")
}

fn session_prompt_request(user_id: &str) -> BasaltAdmissionRequest {
    clankers::modes::daemon::session_store::session_prompt_admission_request(user_id)
}

fn session_attach_request(session_id: &str) -> BasaltAdmissionRequest {
    clankers::modes::daemon::session_store::session_attach_admission_request(session_id)
}

fn session_create_request() -> BasaltAdmissionRequest {
    clankers::modes::daemon::session_store::session_create_admission_request()
}

fn root_credential_for(owner: &PublicUcanIssuer, user_id: &str) -> PublicCredentialEnvelope {
    let session = issuer(91);
    owner
        .issue_root_credential(
            session.audience().expect("audience"),
            caps(vec![
                cap("clankers:daemon/", "session/create"),
                cap("clankers:session/", "session/prompt"),
                cap("clankers:session/", "session/attach"),
                cap("clankers:session/", "session/manage"),
                cap("clankers:tool/read", "tool/use"),
                cap("clankers:file:/tmp/project/", "file/read"),
                cap("clankers:model/", "model/use"),
                cap(&format!("clankers:session/{user_id}"), "session/attach"),
            ]),
            Duration::from_secs(3600),
        )
        .expect("credential")
}

fn manual_owner_credential(
    owner_key: &iroh::SecretKey,
    envelope_audience: Option<AudienceDid>,
    capabilities: CapabilitySet,
    bounds: TokenTimeBounds,
) -> PublicCredentialEnvelope {
    let signer = ucan::Ed25519InMemorySigner::from_seed_bytes(owner_key.to_bytes());
    let session = issuer(97);
    let token_audience = session.audience().expect("token audience");
    let token = issue_token_with_signer(&signer, &token_audience, &capabilities, &ProofCollection::empty(), bounds)
        .expect("manual token");
    PublicCredentialEnvelope::new(token, Vec::new(), envelope_audience.unwrap_or(token_audience), vec![
        signer.issuer().expect("owner issuer"),
    ])
}

fn expired_credential(owner_key: &iroh::SecretKey) -> PublicCredentialEnvelope {
    manual_owner_credential(
        owner_key,
        None,
        caps(vec![cap("clankers:session/", "session/prompt")]),
        TokenTimeBounds::new(1, 2).expect("expired bounds"),
    )
}

fn not_before_credential(owner_key: &iroh::SecretKey) -> PublicCredentialEnvelope {
    let now = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).expect("clock").as_secs();
    manual_owner_credential(
        owner_key,
        None,
        caps(vec![cap("clankers:session/", "session/prompt")]),
        TokenTimeBounds::new(now + 3_600, now + 7_200).expect("future bounds"),
    )
}

#[test]
fn public_root_credential_roundtrip_through_auth_layer() {
    let owner_sk = iroh::SecretKey::generate(&mut rand::rng());
    let owner = PublicUcanIssuer::from_iroh_secret_key(&owner_sk);
    let (auth, _tmp) = test_auth_layer(&owner_sk);
    let cred = root_credential_for(&owner, "test-user-1");

    let b64 = cred.to_base64().unwrap();
    let decoded = PublicCredentialEnvelope::from_base64(&b64).unwrap();
    let receipt = auth.verify_credential(&decoded, &session_prompt_request("test-user-1")).unwrap();
    assert!(receipt.is_allowed());
    assert_eq!(receipt.revocation_status, "checked");

    auth.store_credential("test-user-1", &decoded);
    let looked_up = auth.lookup_credential("test-user-1").unwrap();
    assert_eq!(looked_up.token_reference(), decoded.token_reference());
}

#[test]
fn delegated_public_credential_chain_verification() {
    let owner_sk = iroh::SecretKey::generate(&mut rand::rng());
    let owner = PublicUcanIssuer::from_iroh_secret_key(&owner_sk);
    let (auth, _tmp) = test_auth_layer(&owner_sk);
    let child = issuer(93);
    let session = issuer(95);

    let parent = owner
        .delegate_to(&child, caps(vec![cap("clankers:session/", "session/prompt")]), Duration::from_secs(3600))
        .expect("parent");
    let child_cred = child
        .issue_child_from_parent(
            &parent,
            session.audience().expect("audience"),
            caps(vec![cap("clankers:session/alice", "session/prompt")]),
            Duration::from_secs(1800),
        )
        .expect("child credential");

    let receipt = auth.verify_credential(&child_cred, &session_prompt_request("alice")).unwrap();

    assert!(receipt.is_allowed());
    assert_eq!(child_cred.proofs().len(), 1);
    assert_eq!(receipt.trusted_roots, vec![owner.issuer().expect("owner issuer").to_string()]);
}

#[test]
fn untrusted_public_root_rejected() {
    let owner_sk = iroh::SecretKey::generate(&mut rand::rng());
    let (auth, _tmp) = test_auth_layer(&owner_sk);
    let rando = issuer(101);
    let cred = root_credential_for(&rando, "mallory");

    let result = auth.verify_credential(&cred, &session_prompt_request("mallory"));

    assert!(result.is_err());
    assert!(result.unwrap_err().contains("untrusted public UCAN root"));
}

#[test]
fn revoked_public_reference_is_rejected() {
    let owner_sk = iroh::SecretKey::generate(&mut rand::rng());
    let owner = PublicUcanIssuer::from_iroh_secret_key(&owner_sk);
    let (auth, _tmp) = test_auth_layer(&owner_sk);
    let cred = root_credential_for(&owner, "alice");
    auth.public_store.revoke_reference(&cred.token_reference()).expect("revoke");

    let result = auth.verify_credential(&cred, &session_prompt_request("alice"));

    assert!(result.is_err());
    assert!(result.unwrap_err().contains("verification denied"));
}

#[test]
fn remote_entrypoint_requests_match_public_ucan_vocabulary() {
    let create = session_create_request();
    assert_eq!(create.contract(), "session-create");
    assert_eq!(create.resource(), "clankers:daemon/remote/session");
    assert_eq!(create.ability(), "session/create");

    let attach = session_attach_request("session-1");
    assert_eq!(attach.contract(), "session-attach");
    assert_eq!(attach.resource(), "clankers:session/session-1");
    assert_eq!(attach.ability(), "session/attach");

    let prompt = session_prompt_request("@alice:example.org");
    assert_eq!(prompt.contract(), "session-prompt");
    assert_eq!(prompt.resource(), "clankers:session/@alice:example.org");
    assert_eq!(prompt.ability(), "session/prompt");
}

#[test]
fn quic_create_attach_and_chat_rpc_prompt_requests_accept_valid_public_ucan() {
    let owner_sk = iroh::SecretKey::generate(&mut rand::rng());
    let owner = PublicUcanIssuer::from_iroh_secret_key(&owner_sk);
    let (auth, _tmp) = test_auth_layer(&owner_sk);
    let cred = root_credential_for(&owner, "alice");

    assert!(auth.verify_credential(&cred, &session_create_request()).expect("create").is_allowed());
    assert!(auth.verify_credential(&cred, &session_attach_request("alice")).expect("attach").is_allowed());
    assert!(auth.verify_credential(&cred, &session_prompt_request("alice")).expect("prompt").is_allowed());
}

#[test]
fn remote_auth_rejects_malformed_and_legacy_base64() {
    let owner_sk = iroh::SecretKey::generate(&mut rand::rng());
    let (auth, _tmp) = test_auth_layer(&owner_sk);
    let request = session_prompt_request("alice");

    let malformed = auth.verify_credential_base64("not public ucan", &request).expect_err("malformed");
    assert!(malformed.contains("invalid public UCAN credential encoding"));

    let legacy_key = iroh::SecretKey::from([31u8; 32]);
    let legacy_token = clankers_ucan::TokenBuilder::new(legacy_key)
        .with_capability(clankers_ucan::Capability::Prompt)
        .build()
        .expect("legacy token");
    let legacy = clankers_ucan::Credential::from_root(legacy_token).to_base64().expect("legacy base64");
    let legacy_error = auth.verify_credential_base64(legacy.as_str(), &request).expect_err("legacy rejected");
    assert!(legacy_error.contains("invalid public UCAN credential encoding"));
}

#[test]
fn remote_auth_rejects_expired_not_before_wrong_audience_and_policy_denied_credentials() {
    let owner_sk = iroh::SecretKey::generate(&mut rand::rng());
    let owner = PublicUcanIssuer::from_iroh_secret_key(&owner_sk);
    let (auth, _tmp) = test_auth_layer(&owner_sk);

    let expired = expired_credential(&owner_sk);
    let expired_error = auth.verify_credential(&expired, &session_prompt_request("alice")).expect_err("expired");
    assert!(expired_error.contains("verification denied"));

    let future = not_before_credential(&owner_sk);
    let future_error = auth.verify_credential(&future, &session_prompt_request("alice")).expect_err("not before");
    assert!(future_error.contains("verification denied"));

    let wrong_audience = manual_owner_credential(
        &owner_sk,
        Some(issuer(111).audience().expect("wrong audience")),
        caps(vec![cap("clankers:session/", "session/prompt")]),
        TokenTimeBounds::from_unix_seconds_and_duration(
            SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).expect("clock").as_secs(),
            Duration::from_secs(3600),
        )
        .expect("bounds"),
    );
    let wrong_audience_error = auth
        .verify_credential(&wrong_audience, &session_prompt_request("alice"))
        .expect_err("wrong audience");
    assert!(wrong_audience_error.contains("audience mismatch"));

    let prompt_only = owner
        .issue_root_credential(
            issuer(113).audience().expect("audience"),
            caps(vec![cap("clankers:session/", "session/prompt")]),
            Duration::from_secs(3600),
        )
        .expect("prompt credential");
    let denied = auth.verify_credential(&prompt_only, &session_create_request()).expect_err("policy denied");
    assert!(denied.contains("public UCAN authorization denied") || denied.contains("Basalt policy denied"));
}

#[test]
fn matrix_stored_credential_resolution_does_not_reconsume_replay_id() {
    let owner_sk = iroh::SecretKey::generate(&mut rand::rng());
    let owner = PublicUcanIssuer::from_iroh_secret_key(&owner_sk);
    let (auth, _tmp) = test_auth_layer(&owner_sk);
    let cred = root_credential_for(&owner, "@alice:example.org").with_replay_id("matrix-register-1");
    let b64 = cred.to_base64().expect("base64");
    let (decoded, receipt) = auth
        .verify_credential_base64(b64.as_str(), &session_prompt_request("@alice:example.org"))
        .expect("initial registration");
    assert_eq!(receipt.replay_status, "accepted");
    auth.store_credential("@alice:example.org", &decoded);

    let first = auth
        .resolve_credential("@alice:example.org", &session_prompt_request("@alice:example.org"))
        .expect("stored")
        .expect("first resolve");
    let second = auth
        .resolve_credential("@alice:example.org", &session_prompt_request("@alice:example.org"))
        .expect("stored")
        .expect("second resolve");

    assert_eq!(first.token_reference(), decoded.token_reference());
    assert_eq!(second.token_reference(), decoded.token_reference());
}

#[test]
fn resolve_credential_returns_none_for_unknown_user() {
    let owner_sk = iroh::SecretKey::generate(&mut rand::rng());
    let (auth, _tmp) = test_auth_layer(&owner_sk);

    assert!(auth.resolve_credential("nobody", &session_prompt_request("nobody")).is_none());
}
