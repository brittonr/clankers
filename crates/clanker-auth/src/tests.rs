//! Tests using a simple TestCap to validate the generic token machinery.

use std::time::Duration;

use iroh::SecretKey;
use serde::Deserialize;
use serde::Serialize;

use crate::Cap;
use crate::Credential;
use crate::TokenBuilder;
use crate::TokenVerifier;
use crate::constants::MAX_DELEGATION_DEPTH;
use crate::error::AuthError;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
enum TestCap {
    Read,
    Write,
    Admin,
    Delegate,
}

#[derive(Debug)]
enum TestOp {
    Read,
    Write,
}

impl Cap for TestCap {
    type Operation = TestOp;

    fn authorizes(&self, op: &TestOp) -> bool {
        match (self, op) {
            (TestCap::Read, TestOp::Read) => true,
            (TestCap::Write, TestOp::Write) => true,
            (TestCap::Admin, _) => true,
            _ => false,
        }
    }

    fn contains(&self, child: &TestCap) -> bool {
        match (self, child) {
            (TestCap::Admin, _) => true,
            (TestCap::Delegate, TestCap::Delegate) => true,
            (a, b) => a == b,
        }
    }

    fn is_delegate(&self) -> bool {
        matches!(self, TestCap::Delegate)
    }
}

#[test]
fn roundtrip_encode_decode() {
    let key = iroh::SecretKey::from([1u8; 32]);
    let token = TokenBuilder::<TestCap>::new(key)
        .with_capability(TestCap::Read)
        .with_lifetime(Duration::from_secs(3600))
        .build()
        .unwrap();

    let encoded = token.encode().unwrap();
    let decoded = crate::CapabilityToken::<TestCap>::decode(&encoded).unwrap();
    assert_eq!(decoded.capabilities, vec![TestCap::Read]);
}

#[test]
fn base64_roundtrip() {
    let key = iroh::SecretKey::from([2u8; 32]);
    let token = TokenBuilder::<TestCap>::new(key)
        .with_capability(TestCap::Write)
        .build()
        .unwrap();

    let b64 = token.to_base64().unwrap();
    let decoded = crate::CapabilityToken::<TestCap>::from_base64(&b64).unwrap();
    assert_eq!(decoded.capabilities, vec![TestCap::Write]);
}

#[test]
fn verify_valid_token() {
    let key = iroh::SecretKey::from([3u8; 32]);
    let token = TokenBuilder::<TestCap>::new(key)
        .with_capability(TestCap::Read)
        .build()
        .unwrap();

    let verifier = TokenVerifier::<TestCap>::new();
    verifier.verify(&token, None).unwrap();
}

#[test]
fn verify_rejects_tampered_signature() {
    let key = iroh::SecretKey::from([4u8; 32]);
    let mut token = TokenBuilder::<TestCap>::new(key)
        .with_capability(TestCap::Read)
        .build()
        .unwrap();

    token.signature[0] ^= 0xff;

    let verifier = TokenVerifier::<TestCap>::new();
    assert!(matches!(verifier.verify(&token, None), Err(AuthError::InvalidSignature)));
}

#[test]
fn authorize_read_op() {
    let key = iroh::SecretKey::from([5u8; 32]);
    let token = TokenBuilder::<TestCap>::new(key)
        .with_capability(TestCap::Read)
        .build()
        .unwrap();

    let verifier = TokenVerifier::<TestCap>::new();
    verifier.authorize(&token, &TestOp::Read, None).unwrap();
    assert!(verifier.authorize(&token, &TestOp::Write, None).is_err());
}

#[test]
fn admin_authorizes_everything() {
    let key = iroh::SecretKey::from([6u8; 32]);
    let token = TokenBuilder::<TestCap>::new(key)
        .with_capability(TestCap::Admin)
        .build()
        .unwrap();

    let verifier = TokenVerifier::<TestCap>::new();
    verifier.authorize(&token, &TestOp::Read, None).unwrap();
    verifier.authorize(&token, &TestOp::Write, None).unwrap();
}

// r[verify auth.verify.revocation]
#[test]
fn revoked_token_rejected() {
    let key = iroh::SecretKey::from([7u8; 32]);
    let token = TokenBuilder::<TestCap>::new(key)
        .with_capability(TestCap::Read)
        .with_random_nonce()
        .build()
        .unwrap();

    let verifier = TokenVerifier::<TestCap>::new();
    verifier.verify(&token, None).unwrap();

    verifier.revoke_token(&token).unwrap();
    assert!(matches!(verifier.verify(&token, None), Err(AuthError::TokenRevoked)));
}

// r[verify auth.delegation.transitivity]
#[test]
fn delegation_two_level() {
    let root_key = iroh::SecretKey::from([8u8; 32]);
    let child_key = iroh::SecretKey::from([9u8; 32]);

    let root_token = TokenBuilder::<TestCap>::new(root_key)
        .with_capability(TestCap::Admin)
        .with_capability(TestCap::Delegate)
        .build()
        .unwrap();

    let child_token = TokenBuilder::<TestCap>::new(child_key)
        .with_capability(TestCap::Read)
        .delegated_from(root_token.clone())
        .build()
        .unwrap();

    assert_eq!(child_token.delegation_depth, 1);

    let verifier = TokenVerifier::<TestCap>::new()
        .with_trusted_root(root_token.issuer);
    verifier.register_parent_token(root_token).unwrap();
    verifier.verify(&child_token, None).unwrap();
    verifier.authorize(&child_token, &TestOp::Read, None).unwrap();
}

// r[verify auth.delegation.transitivity]
#[test]
fn delegation_three_level() {
    let root_key = iroh::SecretKey::from([10u8; 32]);
    let mid_key = iroh::SecretKey::from([11u8; 32]);
    let leaf_key = iroh::SecretKey::from([12u8; 32]);

    let root = TokenBuilder::<TestCap>::new(root_key)
        .with_capability(TestCap::Admin)
        .with_capability(TestCap::Delegate)
        .build()
        .unwrap();

    let mid = TokenBuilder::<TestCap>::new(mid_key)
        .with_capability(TestCap::Read)
        .with_capability(TestCap::Delegate)
        .delegated_from(root.clone())
        .build()
        .unwrap();

    let leaf = TokenBuilder::<TestCap>::new(leaf_key)
        .with_capability(TestCap::Read)
        .delegated_from(mid.clone())
        .build()
        .unwrap();

    assert_eq!(leaf.delegation_depth, 2);

    let verifier = TokenVerifier::<TestCap>::new()
        .with_trusted_root(root.issuer);
    verifier.register_parent_token(root).unwrap();
    verifier.register_parent_token(mid).unwrap();
    verifier.verify(&leaf, None).unwrap();
}

// r[verify auth.build.no-escalation]
#[test]
fn escalation_prevented() {
    let root_key = iroh::SecretKey::from([13u8; 32]);
    let child_key = iroh::SecretKey::from([14u8; 32]);

    let root_token = TokenBuilder::<TestCap>::new(root_key)
        .with_capability(TestCap::Read)
        .with_capability(TestCap::Delegate)
        .build()
        .unwrap();

    let result = TokenBuilder::<TestCap>::new(child_key)
        .with_capability(TestCap::Write)
        .delegated_from(root_token)
        .build();

    assert!(matches!(result, Err(AuthError::CapabilityEscalation { .. })));
}

// r[verify auth.build.delegate-required]
#[test]
fn delegation_without_delegate_cap_rejected() {
    let root_key = iroh::SecretKey::from([15u8; 32]);
    let child_key = iroh::SecretKey::from([16u8; 32]);

    let root_token = TokenBuilder::<TestCap>::new(root_key)
        .with_capability(TestCap::Read)
        .build()
        .unwrap();

    let result = TokenBuilder::<TestCap>::new(child_key)
        .with_capability(TestCap::Read)
        .delegated_from(root_token)
        .build();

    assert!(matches!(result, Err(AuthError::DelegationNotAllowed)));
}

// r[verify auth.verify.audience]
#[test]
fn audience_enforcement() {
    let key = iroh::SecretKey::from([17u8; 32]);
    let audience_key = iroh::SecretKey::from([18u8; 32]);
    let wrong_key = iroh::SecretKey::from([19u8; 32]);

    let token = TokenBuilder::<TestCap>::new(key)
        .with_capability(TestCap::Read)
        .for_key(audience_key.public())
        .build()
        .unwrap();

    let verifier = TokenVerifier::<TestCap>::new();

    // Correct audience
    verifier.verify(&token, Some(&audience_key.public())).unwrap();

    // Wrong audience
    assert!(matches!(
        verifier.verify(&token, Some(&wrong_key.public())),
        Err(AuthError::WrongAudience { .. })
    ));

    // No audience provided
    assert!(matches!(
        verifier.verify(&token, None),
        Err(AuthError::AudienceRequired)
    ));
}

#[test]
fn trusted_root_enforcement() {
    let trusted_key = iroh::SecretKey::from([20u8; 32]);
    let untrusted_key = iroh::SecretKey::from([21u8; 32]);

    let trusted_token = TokenBuilder::<TestCap>::new(trusted_key.clone())
        .with_capability(TestCap::Read)
        .build()
        .unwrap();

    let untrusted_token = TokenBuilder::<TestCap>::new(untrusted_key)
        .with_capability(TestCap::Read)
        .build()
        .unwrap();

    let verifier = TokenVerifier::<TestCap>::new()
        .with_trusted_root(trusted_key.public());

    verifier.verify(&trusted_token, None).unwrap();
    assert!(matches!(verifier.verify(&untrusted_token, None), Err(AuthError::UntrustedRoot)));
}

// r[verify auth.verify.chain-complete]
#[test]
fn verify_with_chain() {
    let root_key = iroh::SecretKey::from([22u8; 32]);
    let child_key = iroh::SecretKey::from([23u8; 32]);

    let root = TokenBuilder::<TestCap>::new(root_key.clone())
        .with_capability(TestCap::Admin)
        .with_capability(TestCap::Delegate)
        .build()
        .unwrap();

    let child = TokenBuilder::<TestCap>::new(child_key)
        .with_capability(TestCap::Read)
        .delegated_from(root.clone())
        .build()
        .unwrap();

    let verifier = TokenVerifier::<TestCap>::new()
        .with_trusted_root(root_key.public());

    // Without chain — fails (parent not cached)
    assert!(verifier.verify(&child, None).is_err());

    // With explicit chain — works
    verifier.verify_with_chain(&child, &[root], None).unwrap();
}

// ── Credential tests ────────────────────────────────────────────────────────

#[test]
fn credential_encode_decode_roundtrip() {
    let sk = SecretKey::from([30u8; 32]);
    let token = TokenBuilder::<TestCap>::new(sk)
        .with_capability(TestCap::Read)
        .with_lifetime(Duration::from_secs(3600))
        .build()
        .unwrap();

    let cred = Credential::from_root(token);
    let bytes = cred.encode().unwrap();
    let decoded = Credential::<TestCap>::decode(&bytes).unwrap();

    assert_eq!(decoded.token.issuer, cred.token.issuer);
    assert_eq!(decoded.token.capabilities, vec![TestCap::Read]);
    assert!(decoded.proofs.is_empty());
}

#[test]
fn credential_base64_roundtrip() {
    let sk = SecretKey::from([31u8; 32]);
    let token = TokenBuilder::<TestCap>::new(sk)
        .with_capability(TestCap::Write)
        .build()
        .unwrap();

    let cred = Credential::from_root(token);
    let b64 = cred.to_base64().unwrap();
    let decoded = Credential::<TestCap>::from_base64(&b64).unwrap();

    assert_eq!(decoded.token.capabilities, vec![TestCap::Write]);
}

// r[verify auth.credential.self-contained]
#[test]
fn credential_two_level_delegation() {
    let root_sk = SecretKey::from([32u8; 32]);
    let root_pk = root_sk.public();
    let child_sk = SecretKey::from([33u8; 32]);
    let child_pk = child_sk.public();
    let presenter_pk = SecretKey::from([34u8; 32]).public();

    let root_token = TokenBuilder::<TestCap>::new(root_sk)
        .for_key(child_pk)
        .with_capability(TestCap::Admin)
        .with_capability(TestCap::Delegate)
        .with_lifetime(Duration::from_secs(3600))
        .build()
        .unwrap();

    let root_cred = Credential::from_root(root_token);
    let child_cred = root_cred
        .delegate(
            &child_sk,
            presenter_pk,
            vec![TestCap::Read],
            Duration::from_secs(1800),
        )
        .unwrap();

    assert_eq!(child_cred.proofs.len(), 1);
    assert_eq!(child_cred.token.delegation_depth, 1);
    assert!(child_cred.verify(&[root_pk], Some(&presenter_pk)).is_ok());
}

// r[verify auth.credential.self-contained]
#[test]
fn credential_three_level_delegation() {
    let root_sk = SecretKey::from([35u8; 32]);
    let root_pk = root_sk.public();
    let mid_sk = SecretKey::from([36u8; 32]);
    let mid_pk = mid_sk.public();
    let leaf_sk = SecretKey::from([37u8; 32]);
    let leaf_pk = leaf_sk.public();
    let presenter_pk = SecretKey::from([38u8; 32]).public();

    let root_token = TokenBuilder::<TestCap>::new(root_sk)
        .for_key(mid_pk)
        .with_capability(TestCap::Admin)
        .with_capability(TestCap::Delegate)
        .with_lifetime(Duration::from_secs(3600))
        .build()
        .unwrap();

    let root_cred = Credential::from_root(root_token);

    let mid_cred = root_cred
        .delegate(
            &mid_sk,
            leaf_pk,
            vec![TestCap::Read, TestCap::Delegate],
            Duration::from_secs(1800),
        )
        .unwrap();

    let leaf_cred = mid_cred
        .delegate(
            &leaf_sk,
            presenter_pk,
            vec![TestCap::Read],
            Duration::from_secs(900),
        )
        .unwrap();

    assert_eq!(leaf_cred.proofs.len(), 2);
    assert_eq!(leaf_cred.token.delegation_depth, 2);
    assert!(leaf_cred.verify(&[root_pk], Some(&presenter_pk)).is_ok());
}

// r[verify auth.verify.chain-complete]
#[test]
fn credential_broken_chain_rejected() {
    let root_sk = SecretKey::from([39u8; 32]);
    let root_pk = root_sk.public();
    let child_sk = SecretKey::from([40u8; 32]);
    let child_pk = child_sk.public();

    let root_token = TokenBuilder::<TestCap>::new(root_sk)
        .for_key(child_pk)
        .with_capability(TestCap::Admin)
        .with_capability(TestCap::Delegate)
        .with_lifetime(Duration::from_secs(3600))
        .build()
        .unwrap();

    // Build a child delegated from root
    let child_token = TokenBuilder::<TestCap>::new(child_sk)
        .with_capability(TestCap::Read)
        .delegated_from(root_token)
        .build()
        .unwrap();

    // Stick an unrelated token in the proof chain instead of the real parent
    let unrelated_sk = SecretKey::from([41u8; 32]);
    let unrelated_token = TokenBuilder::<TestCap>::new(unrelated_sk)
        .with_capability(TestCap::Write)
        .with_lifetime(Duration::from_secs(3600))
        .build()
        .unwrap();

    let broken_cred = Credential {
        token: child_token,
        proofs: vec![unrelated_token],
    };

    // Proof hash won't match — verification fails
    assert!(broken_cred.verify(&[root_pk], None).is_err());
}

// r[verify auth.build.depth-bound]
#[test]
fn credential_max_depth_enforced() {
    let keys: Vec<SecretKey> = (0..=(MAX_DELEGATION_DEPTH + 2))
        .map(|i| SecretKey::from([50 + i; 32]))
        .collect();

    let root_token = TokenBuilder::<TestCap>::new(keys[0].clone())
        .for_key(keys[1].public())
        .with_capability(TestCap::Admin)
        .with_capability(TestCap::Delegate)
        .with_lifetime(Duration::from_secs(86400))
        .build()
        .unwrap();

    let mut cred = Credential::from_root(root_token);

    // Delegate MAX_DELEGATION_DEPTH times (depth 1..=MAX)
    for i in 1..=MAX_DELEGATION_DEPTH {
        let audience = keys[(i + 1) as usize].public();
        cred = cred
            .delegate(
                &keys[i as usize],
                audience,
                vec![TestCap::Admin, TestCap::Delegate],
                Duration::from_secs(3600),
            )
            .unwrap();
    }

    assert_eq!(cred.token.delegation_depth, MAX_DELEGATION_DEPTH);
    assert_eq!(cred.proofs.len(), MAX_DELEGATION_DEPTH as usize);

    // Verify the full chain
    let presenter = keys[(MAX_DELEGATION_DEPTH + 1) as usize].public();
    assert!(cred.verify(&[keys[0].public()], Some(&presenter)).is_ok());

    // One more delegation exceeds max depth
    let extra = keys[(MAX_DELEGATION_DEPTH + 2) as usize].public();
    let result = cred.delegate(
        &keys[(MAX_DELEGATION_DEPTH + 1) as usize],
        extra,
        vec![TestCap::Read],
        Duration::from_secs(600),
    );
    assert!(matches!(result, Err(AuthError::DelegationTooDeep { .. })));
}

// r[verify auth.build.no-escalation]
#[test]
fn credential_escalation_rejected() {
    let root_sk = SecretKey::from([60u8; 32]);
    let child_sk = SecretKey::from([61u8; 32]);
    let child_pk = child_sk.public();

    // Root grants only Read
    let root_token = TokenBuilder::<TestCap>::new(root_sk)
        .for_key(child_pk)
        .with_capability(TestCap::Read)
        .with_capability(TestCap::Delegate)
        .with_lifetime(Duration::from_secs(3600))
        .build()
        .unwrap();

    let root_cred = Credential::from_root(root_token);

    // Child tries to delegate Write (escalation)
    let grandchild_pk = SecretKey::from([62u8; 32]).public();
    let result = root_cred.delegate(
        &child_sk,
        grandchild_pk,
        vec![TestCap::Write],
        Duration::from_secs(1800),
    );
    assert!(matches!(result, Err(AuthError::CapabilityEscalation { .. })));
}
