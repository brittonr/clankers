//! Public UCAN + Basalt authority checks for remote daemon admission.
//!
//! The authority verifies the versioned public-UCAN credential envelope and then
//! asks Basalt to enforce the same normalized resource/ability pair.  The output
//! is a redacted receipt suitable for daemon diagnostics and deterministic tests.

use serde::Deserialize;
use serde::Serialize;

use crate::public_credential::PublicCredentialEnvelope;
use crate::public_store::RedbPublicCredentialStore;
use crate::public_store::ReplayAdmissionStatus;

pub const BASALT_UCAN_RECEIPT_SCHEMA_V1: &str = "clankers.ucan-basalt.receipt.v1";

pub const CLANKERS_DAEMON_AUTH_POLICY_JSON: &str = r#"{
  "schema_version": "ucan-nickel-contracts.policy.v1",
  "contracts": {
    "session-create": {
      "id": "session-create",
      "description": "Create Clankers daemon sessions",
      "resource_prefixes": ["clankers:daemon/"],
      "abilities": ["session/create"]
    },
    "session-attach": {
      "id": "session-attach",
      "description": "Attach to Clankers sessions",
      "resource_prefixes": ["clankers:session/"],
      "abilities": ["session/attach"]
    },
    "session-prompt": {
      "id": "session-prompt",
      "description": "Prompt Clankers sessions",
      "resource_prefixes": ["clankers:session/"],
      "abilities": ["session/prompt"]
    },
    "session-manage": {
      "id": "session-manage",
      "description": "Manage Clankers sessions",
      "resource_prefixes": ["clankers:session/"],
      "abilities": ["session/manage"]
    },
    "tool-use": {
      "id": "tool-use",
      "description": "Invoke Clankers tools",
      "resource_prefixes": ["clankers:tool/"],
      "abilities": ["tool/use"]
    },
    "file-read": {
      "id": "file-read",
      "description": "Read files",
      "resource_prefixes": ["clankers:file:"],
      "abilities": ["file/read"]
    },
    "file-write": {
      "id": "file-write",
      "description": "Write files",
      "resource_prefixes": ["clankers:file:"],
      "abilities": ["file/write"]
    },
    "shell-execute": {
      "id": "shell-execute",
      "description": "Execute shell commands",
      "resource_prefixes": ["clankers:shell:"],
      "abilities": ["shell/execute"]
    },
    "process-action": {
      "id": "process-action",
      "description": "Observe and mutate process jobs",
      "resource_prefixes": ["clankers:process/"],
      "abilities": ["process/observe", "process/start", "process/mutate", "process/stdin", "process/logs"]
    },
    "model-use": {
      "id": "model-use",
      "description": "Use model/provider backends",
      "resource_prefixes": ["clankers:model/"],
      "abilities": ["model/use"]
    }
  }
}"#;

pub fn clankers_daemon_auth_policy() -> basalt::EnforcementResult<basalt::Policy> {
    basalt::parse_policy_json(CLANKERS_DAEMON_AUTH_POLICY_JSON)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BasaltAdmissionRequest {
    contract: String,
    resource: String,
    ability: String,
}

impl BasaltAdmissionRequest {
    #[must_use]
    pub fn new(contract: impl Into<String>, resource: impl Into<String>, ability: impl Into<String>) -> Self {
        Self {
            contract: contract.into(),
            resource: resource.into(),
            ability: ability.into(),
        }
    }

    #[must_use]
    pub fn contract(&self) -> &str {
        self.contract.as_str()
    }

    #[must_use]
    pub fn resource(&self) -> &str {
        self.resource.as_str()
    }

    #[must_use]
    pub fn ability(&self) -> &str {
        self.ability.as_str()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BasaltAdmissionReceipt {
    pub schema: String,
    pub allowed: bool,
    pub reason: String,
    pub policy_hash: String,
    pub contract: String,
    pub resource: String,
    pub ability: String,
    pub token_reference: String,
    pub audience: String,
    pub issuer: Option<String>,
    pub trusted_roots: Vec<String>,
    pub replay_id: Option<String>,
    pub replay_status: String,
    pub revocation_status: String,
    pub basalt_reason: Option<String>,
}

impl BasaltAdmissionReceipt {
    #[must_use]
    pub fn is_allowed(&self) -> bool {
        self.allowed
    }

    #[must_use]
    pub fn is_denied(&self) -> bool {
        !self.allowed
    }
}

pub struct BasaltUcanAuthority<'a> {
    policy: &'a basalt::Policy,
    policy_hash: String,
}

impl<'a> BasaltUcanAuthority<'a> {
    #[must_use]
    pub fn new(policy: &'a basalt::Policy) -> Self {
        Self {
            policy,
            policy_hash: policy_hash(policy),
        }
    }

    #[must_use]
    pub fn policy_hash(&self) -> &str {
        self.policy_hash.as_str()
    }

    #[must_use]
    pub fn authorize(
        &self,
        envelope: &PublicCredentialEnvelope,
        context: &ucan::VerificationContext,
        request: &BasaltAdmissionRequest,
    ) -> BasaltAdmissionReceipt {
        let token_reference = envelope.token_reference().to_string();
        let audience = envelope.audience().to_string();
        let verified = match envelope.verify_with_context(context) {
            Ok(verified) => verified,
            Err(error) => {
                return self.receipt(
                    false,
                    format!("public UCAN verification denied: {error}"),
                    None,
                    request,
                    ReceiptContext::new(envelope, token_reference, audience).with_revocation_status("not_checked"),
                );
            }
        };
        self.authorize_verified(
            &verified,
            request,
            ReceiptContext::new(envelope, token_reference, audience).with_revocation_status("not_checked"),
        )
    }

    pub fn authorize_with_revocations<R>(
        &self,
        envelope: &PublicCredentialEnvelope,
        time: ucan::VerificationTime,
        revocations: &R,
        request: &BasaltAdmissionRequest,
    ) -> BasaltAdmissionReceipt
    where
        R: ucan::RevocationChecker + ?Sized,
    {
        let token_reference = envelope.token_reference().to_string();
        let audience = envelope.audience().to_string();
        let context = ReceiptContext::new(envelope, token_reference, audience).with_revocation_status("checked");
        let verified = match envelope.verify_with_did_keys_and_revocations(time, revocations) {
            Ok(verified) => verified,
            Err(error) => {
                return self.receipt(
                    false,
                    format!("public UCAN verification denied: {error}"),
                    None,
                    request,
                    context,
                );
            }
        };
        self.authorize_verified(&verified, request, context)
    }

    pub fn authorize_with_store(
        &self,
        envelope: &PublicCredentialEnvelope,
        time: ucan::VerificationTime,
        store: &RedbPublicCredentialStore,
        request: &BasaltAdmissionRequest,
    ) -> BasaltAdmissionReceipt {
        let token_reference = envelope.token_reference().to_string();
        let audience = envelope.audience().to_string();
        let context = ReceiptContext::new(envelope, token_reference, audience).with_revocation_status("checked");
        let verified = match envelope.verify_with_did_keys_and_revocations(time, store) {
            Ok(verified) => verified,
            Err(error) => {
                return self.receipt(
                    false,
                    format!("public UCAN verification denied: {error}"),
                    None,
                    request,
                    context,
                );
            }
        };
        let replay_status = match store.admit_credential_replay(envelope) {
            Ok(status) => status,
            Err(error) => {
                return self.receipt(
                    false,
                    format!("public UCAN replay admission error: {error}"),
                    None,
                    request,
                    context.with_replay_status("backend_error"),
                );
            }
        };
        if replay_status == ReplayAdmissionStatus::Duplicate {
            return self.receipt(
                false,
                "public UCAN replay admission denied: duplicate credential replay id".to_owned(),
                None,
                request,
                context.with_replay_status("duplicate"),
            );
        }
        self.authorize_verified(&verified, request, context.with_replay_status(replay_status.as_str()))
    }

    fn authorize_verified(
        &self,
        verified: &ucan::VerifiedToken,
        request: &BasaltAdmissionRequest,
        context: ReceiptContext,
    ) -> BasaltAdmissionReceipt {
        let ucan_decision = verified.authorize(request.resource(), request.ability());
        if !ucan_decision.is_allowed() {
            return self.receipt(
                false,
                format!("public UCAN authorization denied: {ucan_decision:?}"),
                None,
                request,
                context.with_issuer(verified.issuer().to_string()),
            );
        }

        let grants = verified
            .effective_delegation()
            .capabilities()
            .as_slice()
            .iter()
            .map(|capability| basalt::CapabilityGrant::new(capability.resource.clone(), capability.ability.clone()))
            .collect::<Vec<_>>();
        let basalt_request = basalt::EnforcementRequest::new(request.contract(), request.resource(), request.ability())
            .with_capabilities(grants);
        let basalt_receipt = match basalt::enforce(self.policy, &basalt_request) {
            Ok(receipt) => receipt,
            Err(error) => {
                return self.receipt(
                    false,
                    format!("Basalt policy enforcement error: {error}"),
                    Some(error.to_string()),
                    request,
                    context.with_issuer(verified.issuer().to_string()),
                );
            }
        };
        let is_allowed = basalt_receipt.is_allowed();
        let reason = if is_allowed {
            "allowed by public UCAN and Basalt policy".to_owned()
        } else {
            format!("Basalt policy denied: {}", basalt_receipt.reason())
        };
        self.receipt(
            is_allowed,
            reason,
            Some(basalt_receipt.reason().to_owned()),
            request,
            context.with_issuer(verified.issuer().to_string()),
        )
    }

    fn receipt(
        &self,
        allowed: bool,
        reason: String,
        basalt_reason: Option<String>,
        request: &BasaltAdmissionRequest,
        context: ReceiptContext,
    ) -> BasaltAdmissionReceipt {
        BasaltAdmissionReceipt {
            schema: BASALT_UCAN_RECEIPT_SCHEMA_V1.to_owned(),
            allowed,
            reason,
            policy_hash: self.policy_hash.clone(),
            contract: request.contract().to_owned(),
            resource: request.resource().to_owned(),
            ability: request.ability().to_owned(),
            token_reference: context.token_reference,
            audience: context.audience,
            issuer: context.issuer,
            trusted_roots: context.trusted_roots,
            replay_id: context.replay_id,
            replay_status: context.replay_status,
            revocation_status: context.revocation_status,
            basalt_reason,
        }
    }
}

struct ReceiptContext {
    token_reference: String,
    audience: String,
    issuer: Option<String>,
    trusted_roots: Vec<String>,
    replay_id: Option<String>,
    replay_status: String,
    revocation_status: String,
}

impl ReceiptContext {
    fn new(envelope: &PublicCredentialEnvelope, token_reference: String, audience: String) -> Self {
        Self {
            token_reference,
            audience,
            issuer: None,
            trusted_roots: envelope.trusted_roots().iter().map(ToString::to_string).collect(),
            replay_id: envelope.replay_id().map(ToOwned::to_owned),
            replay_status: if envelope.replay_id().is_some() {
                "not_checked".to_owned()
            } else {
                "not_present".to_owned()
            },
            revocation_status: "not_checked".to_owned(),
        }
    }

    fn with_issuer(mut self, issuer: String) -> Self {
        self.issuer = Some(issuer);
        self
    }

    fn with_replay_status(mut self, status: &str) -> Self {
        status.clone_into(&mut self.replay_status);
        self
    }

    fn with_revocation_status(mut self, status: &str) -> Self {
        status.clone_into(&mut self.revocation_status);
        self
    }
}

impl ReplayAdmissionStatus {
    const fn as_str(self) -> &'static str {
        match self {
            Self::NotPresent => "not_present",
            Self::Accepted => "accepted",
            Self::Duplicate => "duplicate",
        }
    }
}

fn policy_hash(policy: &basalt::Policy) -> String {
    match serde_json::to_vec(policy) {
        Ok(bytes) => format!("blake3:{}", blake3::hash(bytes.as_slice()).to_hex()),
        Err(error) => format!("unhashable-policy:{}", error),
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use basalt::parse_policy_json;
    use ucan::AudienceDid;
    use ucan::CapabilitySet;
    use ucan::Ed25519InMemorySigner;
    use ucan::Ed25519VerificationKey;
    use ucan::KeyResolutionContext;
    use ucan::ProofCollection;
    use ucan::TokenSigner;
    use ucan::TokenTimeBounds;
    use ucan::VerificationContext;
    use ucan::VerificationTime;
    use ucan::issue_token_with_signer;

    use super::*;
    use crate::public_credential::PublicCredentialEnvelope;
    use crate::public_store::RedbPublicCredentialStore;

    const POLICY_JSON: &str = r#"{
      "schema_version": "ucan-nickel-contracts.policy.v1",
      "contracts": {
        "session-attach": {
          "id": "session-attach",
          "description": "Attach to Clankers sessions",
          "resource_prefixes": ["clankers:session/"],
          "abilities": ["session/attach"]
        }
      }
    }"#;
    const ROOT_KEY_BYTE: u8 = 31;
    const SESSION_KEY_BYTE: u8 = 37;
    const NOT_BEFORE_SECONDS: u64 = 1_000;
    const EXPIRES_AT_SECONDS: u64 = 2_000;
    const VERIFY_AT_SECONDS: u64 = 1_500;
    const RESOURCE: &str = "clankers:session/demo";
    const ABILITY: &str = "session/attach";

    fn signer(byte: u8) -> Ed25519InMemorySigner {
        Ed25519InMemorySigner::from_seed_bytes([byte; ucan::ED25519_SECRET_KEY_BYTES])
    }

    fn context(signers: &[&Ed25519InMemorySigner]) -> VerificationContext {
        let keys = signers
            .iter()
            .map(|signer| signer.verification_key())
            .collect::<Result<Vec<Ed25519VerificationKey>, _>>()
            .expect("verification keys");
        VerificationContext::new(
            VerificationTime::from_unix_seconds(VERIFY_AT_SECONDS),
            KeyResolutionContext::new(keys),
            ProofCollection::empty(),
        )
    }

    fn envelope(resource: &str, ability: &str) -> (PublicCredentialEnvelope, VerificationContext) {
        let root = signer(ROOT_KEY_BYTE);
        let session = signer(SESSION_KEY_BYTE);
        let audience = AudienceDid::from(session.issuer().expect("session issuer"));
        let token = issue_token_with_signer(
            &root,
            &audience,
            &CapabilitySet::single(resource, ability).expect("capability"),
            &ProofCollection::empty(),
            TokenTimeBounds::new(NOT_BEFORE_SECONDS, EXPIRES_AT_SECONDS).expect("bounds"),
        )
        .expect("token");
        (
            PublicCredentialEnvelope::new(token, Vec::new(), audience, vec![root.issuer().expect("root issuer")]),
            context(&[&root]),
        )
    }

    fn authority(policy: &basalt::Policy) -> BasaltUcanAuthority<'_> {
        BasaltUcanAuthority::new(policy)
    }

    fn store() -> (tempfile::TempDir, RedbPublicCredentialStore) {
        let tmp = tempfile::tempdir().expect("tempdir");
        let db = Arc::new(redb::Database::create(tmp.path().join("auth.db")).expect("db"));
        let store = RedbPublicCredentialStore::new(db, VERIFY_AT_SECONDS).expect("store");
        (tmp, store)
    }

    #[test]
    fn allows_when_public_ucan_and_basalt_policy_both_allow() {
        let policy = parse_policy_json(POLICY_JSON).expect("policy");
        let (envelope, context) = envelope(RESOURCE, ABILITY);
        let request = BasaltAdmissionRequest::new("session-attach", RESOURCE, ABILITY);

        let receipt = authority(&policy).authorize(&envelope, &context, &request);

        assert!(receipt.is_allowed(), "receipt should allow: {receipt:?}");
        assert_eq!(receipt.schema, BASALT_UCAN_RECEIPT_SCHEMA_V1);
        assert_eq!(receipt.basalt_reason.as_deref(), Some("allowed"));
        assert!(receipt.policy_hash.starts_with("blake3:"));
    }

    #[test]
    fn denies_when_policy_rejects_resource_even_with_valid_ucan() {
        let policy = parse_policy_json(POLICY_JSON).expect("policy");
        let (envelope, context) = envelope("clankers:other/demo", ABILITY);
        let request = BasaltAdmissionRequest::new("session-attach", "clankers:other/demo", ABILITY);

        let receipt = authority(&policy).authorize(&envelope, &context, &request);

        assert!(receipt.is_denied());
        assert!(receipt.reason.contains("Basalt policy denied"));
        assert!(receipt.basalt_reason.expect("basalt reason").contains("outside contract"));
    }

    #[test]
    fn denies_unknown_contract_even_with_matching_public_ucan_grant() {
        let policy = parse_policy_json(POLICY_JSON).expect("policy");
        let (envelope, context) = envelope(RESOURCE, ABILITY);
        let request = BasaltAdmissionRequest::new("unknown-contract", RESOURCE, ABILITY);

        let receipt = authority(&policy).authorize(&envelope, &context, &request);

        assert!(receipt.is_denied());
        assert!(receipt.reason.contains("Basalt policy denied") || receipt.reason.contains("policy enforcement error"));
        assert!(receipt.basalt_reason.is_some());
    }

    #[test]
    fn denies_unknown_ability_even_with_matching_public_ucan_grant() {
        let policy = parse_policy_json(POLICY_JSON).expect("policy");
        let (envelope, context) = envelope(RESOURCE, "session/delete");
        let request = BasaltAdmissionRequest::new("session-attach", RESOURCE, "session/delete");

        let receipt = authority(&policy).authorize(&envelope, &context, &request);

        assert!(receipt.is_denied());
        assert!(receipt.reason.contains("Basalt policy denied") || receipt.reason.contains("policy enforcement error"));
        assert!(receipt.basalt_reason.is_some());
    }

    #[test]
    fn denies_when_ucan_does_not_cover_requested_ability() {
        let policy = parse_policy_json(POLICY_JSON).expect("policy");
        let (envelope, context) = envelope(RESOURCE, "session/create");
        let request = BasaltAdmissionRequest::new("session-attach", RESOURCE, ABILITY);

        let receipt = authority(&policy).authorize(&envelope, &context, &request);

        assert!(receipt.is_denied());
        assert!(receipt.reason.contains("public UCAN authorization denied"));
        assert!(receipt.basalt_reason.is_none());
    }

    #[test]
    fn receipt_is_redacted() {
        let policy = parse_policy_json(POLICY_JSON).expect("policy");
        let (envelope, context) = envelope(RESOURCE, ABILITY);
        let request = BasaltAdmissionRequest::new("session-attach", RESOURCE, ABILITY);

        let receipt = authority(&policy).authorize(&envelope, &context, &request);
        let json = serde_json::to_string(&receipt).expect("receipt json");

        assert!(!json.contains(envelope.token().as_str()));
        for proof in envelope.proofs() {
            assert!(!json.contains(proof.as_str()));
        }
        assert!(json.contains("token_reference"));
        assert!(json.contains("policy_hash"));
    }

    #[test]
    fn store_authorization_records_revocation_and_replay_status() {
        let policy = parse_policy_json(POLICY_JSON).expect("policy");
        let (envelope, _context) = envelope(RESOURCE, ABILITY);
        let envelope = envelope.with_replay_id("replay-1");
        let (_tmp, store) = store();
        let request = BasaltAdmissionRequest::new("session-attach", RESOURCE, ABILITY);

        let receipt = authority(&policy).authorize_with_store(
            &envelope,
            VerificationTime::from_unix_seconds(VERIFY_AT_SECONDS),
            &store,
            &request,
        );

        assert!(receipt.is_allowed(), "receipt should allow: {receipt:?}");
        assert_eq!(receipt.replay_id.as_deref(), Some("replay-1"));
        assert_eq!(receipt.replay_status, "accepted");
        assert_eq!(receipt.revocation_status, "checked");
        assert!(receipt.issuer.is_some());
    }

    #[test]
    fn store_authorization_denies_duplicate_replay_id() {
        let policy = parse_policy_json(POLICY_JSON).expect("policy");
        let (envelope, _context) = envelope(RESOURCE, ABILITY);
        let envelope = envelope.with_replay_id("replay-1");
        let (_tmp, store) = store();
        let request = BasaltAdmissionRequest::new("session-attach", RESOURCE, ABILITY);

        let first = authority(&policy).authorize_with_store(
            &envelope,
            VerificationTime::from_unix_seconds(VERIFY_AT_SECONDS),
            &store,
            &request,
        );
        let second = authority(&policy).authorize_with_store(
            &envelope,
            VerificationTime::from_unix_seconds(VERIFY_AT_SECONDS),
            &store,
            &request,
        );

        assert!(first.is_allowed());
        assert!(second.is_denied());
        assert_eq!(second.replay_status, "duplicate");
        assert!(second.reason.contains("duplicate"));
    }

    #[test]
    fn store_authorization_denies_revoked_reference() {
        let policy = parse_policy_json(POLICY_JSON).expect("policy");
        let (envelope, _context) = envelope(RESOURCE, ABILITY);
        let (_tmp, store) = store();
        let request = BasaltAdmissionRequest::new("session-attach", RESOURCE, ABILITY);
        store.revoke_reference(&envelope.token_reference()).expect("revoke");

        let receipt = authority(&policy).authorize_with_store(
            &envelope,
            VerificationTime::from_unix_seconds(VERIFY_AT_SECONDS),
            &store,
            &request,
        );

        assert!(receipt.is_denied());
        assert_eq!(receipt.revocation_status, "checked");
        assert!(receipt.reason.contains("verification denied"));
    }
}
