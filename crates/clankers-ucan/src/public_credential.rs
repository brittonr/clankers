//! Versioned public-UCAN credential envelopes for daemon/remote auth.
//!
//! This module is the public-UCAN substrate for the daemon auth switch.  It
//! intentionally keeps legacy `clanker-auth` credentials out of the decode path:
//! callers must present a JSON, versioned envelope containing compact public
//! UCAN tokens and proof tokens.

use std::collections::HashMap;
use std::collections::HashSet;
use std::fmt;

use base64::Engine as _;
use serde::Deserialize;
use serde::Serialize;

pub const PUBLIC_CREDENTIAL_SCHEMA_V1: &str = "clankers.ucan.credential.v1";

const PUBLIC_UCAN_MAX_PROOF_DEPTH: usize = 16;
const PUBLIC_UCAN_MAX_PROOF_COUNT: usize = 64;
const PUBLIC_UCAN_MAX_COMPACT_TOKEN_BYTES: usize = 64 * 1024;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PublicCredentialEnvelope {
    schema: String,
    token: ucan::CompactToken,
    #[serde(default = "empty_proofs")]
    proofs: Vec<ucan::CompactToken>,
    audience: ucan::AudienceDid,
    #[serde(default = "empty_trusted_roots")]
    trusted_roots: Vec<ucan::IssuerDid>,
    #[serde(default = "no_replay_id", skip_serializing_if = "Option::is_none")]
    replay_id: Option<String>,
}

impl PublicCredentialEnvelope {
    #[must_use]
    pub fn new(
        token: ucan::CompactToken,
        proofs: Vec<ucan::CompactToken>,
        audience: ucan::AudienceDid,
        trusted_roots: Vec<ucan::IssuerDid>,
    ) -> Self {
        Self {
            schema: PUBLIC_CREDENTIAL_SCHEMA_V1.to_owned(),
            token,
            proofs,
            audience,
            trusted_roots,
            replay_id: None,
        }
    }

    #[must_use]
    pub fn with_replay_id(mut self, replay_id: impl Into<String>) -> Self {
        self.replay_id = Some(replay_id.into());
        self
    }

    #[must_use]
    pub fn schema(&self) -> &str {
        self.schema.as_str()
    }

    #[must_use]
    pub const fn token(&self) -> &ucan::CompactToken {
        &self.token
    }

    #[must_use]
    pub fn proofs(&self) -> &[ucan::CompactToken] {
        self.proofs.as_slice()
    }

    #[must_use]
    pub const fn audience(&self) -> &ucan::AudienceDid {
        &self.audience
    }

    #[must_use]
    pub fn trusted_roots(&self) -> &[ucan::IssuerDid] {
        self.trusted_roots.as_slice()
    }

    pub fn set_trusted_roots(&mut self, trusted_roots: Vec<ucan::IssuerDid>) {
        self.trusted_roots = trusted_roots;
    }

    #[must_use]
    pub fn replay_id(&self) -> Option<&str> {
        self.replay_id.as_deref()
    }

    #[must_use]
    pub fn token_reference(&self) -> ucan::ProofReference {
        ucan::proof_reference(&self.token)
    }

    #[must_use]
    pub fn proof_collection(&self) -> ucan::ProofCollection {
        ucan::ProofCollection::from_tokens(self.proofs.clone())
    }

    pub fn encode(&self) -> PublicCredentialResult<Vec<u8>> {
        self.ensure_supported_schema()?;
        serde_json::to_vec(self).map_err(|source| PublicCredentialError::Encode {
            message: source.to_string(),
        })
    }

    pub fn decode(bytes: &[u8]) -> PublicCredentialResult<Self> {
        let envelope: Self = serde_json::from_slice(bytes).map_err(|source| PublicCredentialError::Decode {
            message: source.to_string(),
        })?;
        envelope.ensure_supported_schema()?;
        Ok(envelope)
    }

    pub fn to_base64(&self) -> PublicCredentialResult<String> {
        Ok(base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(self.encode()?))
    }

    pub fn from_base64(input: &str) -> PublicCredentialResult<Self> {
        let bytes = base64::engine::general_purpose::URL_SAFE_NO_PAD.decode(input).map_err(|source| {
            PublicCredentialError::Decode {
                message: source.to_string(),
            }
        })?;
        Self::decode(bytes.as_slice())
    }

    pub fn verification_context_from(
        &self,
        context: &ucan::VerificationContext,
    ) -> PublicCredentialResult<ucan::VerificationContext> {
        self.ensure_supported_schema()?;
        Ok(ucan::VerificationContext::with_limits(
            context.time(),
            context.keys().clone(),
            self.proof_collection(),
            context.limits(),
        ))
    }

    pub fn verify_with_context(
        &self,
        context: &ucan::VerificationContext,
    ) -> PublicCredentialResult<ucan::VerifiedToken> {
        let context = self.verification_context_from(context)?;
        let verified =
            ucan::verify_compact_token(&self.token, &context).map_err(|source| PublicCredentialError::Verify {
                message: source.to_string(),
            })?;
        self.ensure_audience(&verified)?;
        self.ensure_trusted_root()?;
        Ok(verified)
    }

    pub fn verify_with_context_and_revocations<R>(
        &self,
        context: &ucan::VerificationContext,
        revocations: &R,
    ) -> PublicCredentialResult<ucan::VerifiedToken>
    where
        R: ucan::RevocationChecker + ?Sized,
    {
        self.ensure_supported_schema()?;
        let proofs = self.proof_collection();
        let verified = ucan::verify_compact_token_with_resolvers_and_revocations(
            &self.token,
            context.time(),
            context.keys(),
            &proofs,
            revocations,
            context.limits(),
        )
        .map_err(|source| PublicCredentialError::Verify {
            message: source.to_string(),
        })?;
        self.ensure_audience(&verified)?;
        self.ensure_trusted_root()?;
        Ok(verified)
    }

    pub fn verify_with_did_keys(&self, time: ucan::VerificationTime) -> PublicCredentialResult<ucan::VerifiedToken> {
        self.verify_with_did_keys_and_revocations(time, &ucan::NoRevocations)
    }

    pub fn verify_with_did_keys_and_revocations<R>(
        &self,
        time: ucan::VerificationTime,
        revocations: &R,
    ) -> PublicCredentialResult<ucan::VerifiedToken>
    where
        R: ucan::RevocationChecker + ?Sized,
    {
        self.ensure_supported_schema()?;
        let resolver = DidKeyResolver;
        let proofs = self.proof_collection();
        let verified = ucan::verify_compact_token_with_resolvers_and_revocations(
            &self.token,
            time,
            &resolver,
            &proofs,
            revocations,
            public_ucan_verification_limits(),
        )
        .map_err(|source| PublicCredentialError::Verify {
            message: source.to_string(),
        })?;
        self.ensure_audience(&verified)?;
        self.ensure_trusted_root()?;
        Ok(verified)
    }

    fn ensure_supported_schema(&self) -> PublicCredentialResult<()> {
        if self.schema == PUBLIC_CREDENTIAL_SCHEMA_V1 {
            return Ok(());
        }
        Err(PublicCredentialError::UnsupportedVersion {
            version: self.schema.clone(),
        })
    }

    fn ensure_audience(&self, verified: &ucan::VerifiedToken) -> PublicCredentialResult<()> {
        if verified.audience() == &self.audience {
            return Ok(());
        }
        Err(PublicCredentialError::AudienceMismatch {
            expected: self.audience.to_string(),
            actual: verified.audience().to_string(),
        })
    }

    fn ensure_trusted_root(&self) -> PublicCredentialResult<()> {
        if self.trusted_roots.is_empty() {
            return Err(PublicCredentialError::UntrustedRoot {
                token_reference: self.token_reference().to_string(),
            });
        }
        let proof_map = self
            .proofs
            .iter()
            .map(|proof| (ucan::proof_reference(proof).as_bytes().to_vec(), proof))
            .collect::<HashMap<_, _>>();
        let trusted = self.trusted_roots.iter().map(ToString::to_string).collect::<HashSet<_>>();
        if token_chain_has_trusted_root(&self.token, &proof_map, &trusted)? {
            return Ok(());
        }
        Err(PublicCredentialError::UntrustedRoot {
            token_reference: self.token_reference().to_string(),
        })
    }
}

struct DidKeyResolver;

impl ucan::KeyResolver for DidKeyResolver {
    fn resolve_key(
        &self,
        issuer: &ucan::IssuerDid,
    ) -> std::result::Result<ucan::Ed25519PublicKey, ucan::KeyResolverError> {
        let decoded = ucan::verified::decode_did_key(issuer.as_str().as_bytes()).ok_or_else(|| {
            ucan::KeyResolverError::Malformed {
                issuer: issuer.to_string(),
                message: "issuer is not an Ed25519 did:key".to_owned(),
            }
        })?;
        ucan::Ed25519PublicKey::try_from_slice(decoded.public_key_bytes.as_slice()).map_err(|source| {
            ucan::KeyResolverError::Malformed {
                issuer: issuer.to_string(),
                message: source.to_string(),
            }
        })
    }
}

fn token_chain_has_trusted_root<'a>(
    token: &'a ucan::CompactToken,
    proof_map: &HashMap<Vec<u8>, &'a ucan::CompactToken>,
    trusted: &HashSet<String>,
) -> PublicCredentialResult<bool> {
    let mut pending = vec![token];
    let mut visited = HashSet::new();

    while let Some(candidate) = pending.pop() {
        let reference = ucan::proof_reference(candidate).as_bytes().to_vec();
        if !visited.insert(reference) {
            continue;
        }
        let claims = token_claims(candidate)?;
        let issuer =
            ucan::IssuerDid::new(claims.issuer_did().to_owned()).map_err(|source| PublicCredentialError::Verify {
                message: source.to_string(),
            })?;
        if claims.proofs().is_empty() {
            if trusted.contains(issuer.as_str()) {
                return Ok(true);
            }
            continue;
        }
        for proof in claims.proofs() {
            let Some(parent) = proof_map.get(proof.as_bytes()) else {
                return Err(PublicCredentialError::MissingProof {
                    reference: base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(proof.as_bytes()),
                });
            };
            pending.push(parent);
        }
    }

    Ok(false)
}

fn empty_proofs() -> Vec<ucan::CompactToken> {
    Vec::new()
}

fn empty_trusted_roots() -> Vec<ucan::IssuerDid> {
    Vec::new()
}

const fn no_replay_id() -> Option<String> {
    None
}

const fn public_ucan_verification_limits() -> ucan::VerificationLimits {
    ucan::VerificationLimits::new(
        PUBLIC_UCAN_MAX_PROOF_DEPTH,
        PUBLIC_UCAN_MAX_PROOF_COUNT,
        PUBLIC_UCAN_MAX_COMPACT_TOKEN_BYTES,
    )
}

fn token_claims(token: &ucan::CompactToken) -> PublicCredentialResult<ucan::core::token::Claims> {
    let decoded = ucan::parse_compact_token(token).map_err(|source| PublicCredentialError::Decode {
        message: source.to_string(),
    })?;
    let valid = ucan::validate_decoded_token(decoded).map_err(|source| PublicCredentialError::Decode {
        message: source.to_string(),
    })?;
    Ok(valid.claims().clone())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PublicCredentialError {
    Decode { message: String },
    Encode { message: String },
    UnsupportedVersion { version: String },
    Verify { message: String },
    AudienceMismatch { expected: String, actual: String },
    MissingProof { reference: String },
    UntrustedRoot { token_reference: String },
}

impl fmt::Display for PublicCredentialError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Decode { message } => {
                write!(formatter, "failed to decode public UCAN credential envelope: {message}")
            }
            Self::Encode { message } => {
                write!(formatter, "failed to encode public UCAN credential envelope: {message}")
            }
            Self::UnsupportedVersion { version } => {
                write!(formatter, "unsupported public UCAN credential envelope version {version:?}")
            }
            Self::Verify { message } => write!(formatter, "public UCAN credential verification failed: {message}"),
            Self::AudienceMismatch { expected, actual } => {
                write!(formatter, "public UCAN credential audience mismatch: expected {expected}, got {actual}")
            }
            Self::MissingProof { reference } => {
                write!(formatter, "public UCAN credential is missing proof {reference}")
            }
            Self::UntrustedRoot { token_reference } => {
                write!(formatter, "public UCAN credential {token_reference} is not anchored to a trusted root")
            }
        }
    }
}

impl std::error::Error for PublicCredentialError {}

pub type PublicCredentialResult<T> = Result<T, PublicCredentialError>;

#[cfg(test)]
mod tests {
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

    const ROOT_KEY_BYTE: u8 = 11;
    const CHILD_KEY_BYTE: u8 = 13;
    const SESSION_KEY_BYTE: u8 = 17;
    const NOT_BEFORE_SECONDS: u64 = 1_000;
    const EXPIRES_AT_SECONDS: u64 = 2_000;
    const VERIFY_AT_SECONDS: u64 = 1_500;
    const RESOURCE: &str = "clankers:session/demo";
    const ABILITY: &str = "session/attach";

    fn signer(byte: u8) -> Ed25519InMemorySigner {
        Ed25519InMemorySigner::from_seed_bytes([byte; ucan::ED25519_SECRET_KEY_BYTES])
    }

    fn bounds() -> TokenTimeBounds {
        TokenTimeBounds::new(NOT_BEFORE_SECONDS, EXPIRES_AT_SECONDS).expect("bounds")
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

    fn root_token(root: &Ed25519InMemorySigner, audience: AudienceDid) -> ucan::CompactToken {
        issue_token_with_signer(
            root,
            &audience,
            &CapabilitySet::single(RESOURCE, ABILITY).expect("capability"),
            &ProofCollection::empty(),
            bounds(),
        )
        .expect("root token")
    }

    #[test]
    fn envelope_base64_round_trips_and_verifies_root_token() {
        let root = signer(ROOT_KEY_BYTE);
        let session = signer(SESSION_KEY_BYTE);
        let audience = AudienceDid::from(session.issuer().expect("session issuer"));
        let token = root_token(&root, audience.clone());
        let envelope =
            PublicCredentialEnvelope::new(token, Vec::new(), audience, vec![root.issuer().expect("root issuer")])
                .with_replay_id("demo-replay");

        let encoded = envelope.to_base64().expect("base64 encode");
        let decoded = PublicCredentialEnvelope::from_base64(encoded.as_str()).expect("base64 decode");
        let verified = decoded.verify_with_context(&context(&[&root])).expect("verify root");

        assert_eq!(decoded.schema(), PUBLIC_CREDENTIAL_SCHEMA_V1);
        assert_eq!(decoded.replay_id(), Some("demo-replay"));
        assert_eq!(verified.audience(), decoded.audience());
        assert_eq!(decoded.token_reference(), ucan::proof_reference(decoded.token()));
    }

    #[test]
    fn envelope_supplies_proofs_for_child_delegation() {
        let root = signer(ROOT_KEY_BYTE);
        let child = signer(CHILD_KEY_BYTE);
        let session = signer(SESSION_KEY_BYTE);
        let child_audience = AudienceDid::from(child.issuer().expect("child issuer"));
        let parent = root_token(&root, child_audience);
        let session_audience = AudienceDid::from(session.issuer().expect("session issuer"));
        let child_token = issue_token_with_signer(
            &child,
            &session_audience,
            &CapabilitySet::single(RESOURCE, ABILITY).expect("capability"),
            &ProofCollection::from_tokens(vec![parent.clone()]),
            bounds(),
        )
        .expect("child token");
        let envelope = PublicCredentialEnvelope::new(child_token, vec![parent], session_audience, vec![
            root.issuer().expect("root issuer"),
        ]);

        let verified = envelope.verify_with_context(&context(&[&root, &child])).expect("verify child");

        assert_eq!(verified.proofs().as_slice().len(), 1);
        assert_eq!(verified.effective_delegation().capabilities().as_slice().len(), 1);
    }

    #[test]
    fn envelope_supplies_proofs_for_grandchild_delegation() {
        let root = signer(ROOT_KEY_BYTE);
        let child = signer(CHILD_KEY_BYTE);
        let grandchild = signer(19);
        let session = signer(SESSION_KEY_BYTE);
        let parent = root_token(&root, AudienceDid::from(child.issuer().expect("child issuer")));
        let child_token = issue_token_with_signer(
            &child,
            &AudienceDid::from(grandchild.issuer().expect("grandchild issuer")),
            &CapabilitySet::single(RESOURCE, ABILITY).expect("capability"),
            &ProofCollection::from_tokens(vec![parent.clone()]),
            bounds(),
        )
        .expect("child token");
        let grandchild_token = issue_token_with_signer(
            &grandchild,
            &AudienceDid::from(session.issuer().expect("session issuer")),
            &CapabilitySet::single(RESOURCE, ABILITY).expect("capability"),
            &ProofCollection::from_tokens(vec![child_token.clone()]),
            bounds(),
        )
        .expect("grandchild token");
        let audience = AudienceDid::from(session.issuer().expect("session issuer"));
        let envelope = PublicCredentialEnvelope::new(grandchild_token, vec![child_token, parent], audience, vec![
            root.issuer().expect("root issuer"),
        ]);

        let verified =
            envelope.verify_with_context(&context(&[&root, &child, &grandchild])).expect("verify grandchild");

        assert_eq!(verified.issuer(), &grandchild.issuer().expect("grandchild issuer"));
        assert_eq!(envelope.proofs().len(), 2);
    }

    #[test]
    fn missing_proof_in_child_delegation_is_rejected() {
        let root = signer(ROOT_KEY_BYTE);
        let child = signer(CHILD_KEY_BYTE);
        let session = signer(SESSION_KEY_BYTE);
        let parent = root_token(&root, AudienceDid::from(child.issuer().expect("child issuer")));
        let audience = AudienceDid::from(session.issuer().expect("session issuer"));
        let child_token = issue_token_with_signer(
            &child,
            &audience,
            &CapabilitySet::single(RESOURCE, ABILITY).expect("capability"),
            &ProofCollection::from_tokens(vec![parent]),
            bounds(),
        )
        .expect("child token");
        let envelope =
            PublicCredentialEnvelope::new(child_token, Vec::new(), audience, vec![root.issuer().expect("root issuer")]);

        let error = envelope.verify_with_context(&context(&[&root, &child])).expect_err("missing proof should reject");

        assert!(matches!(error, PublicCredentialError::Verify { .. } | PublicCredentialError::MissingProof { .. }));
    }

    #[test]
    fn widened_child_delegation_fails_closed() {
        let root = signer(ROOT_KEY_BYTE);
        let child = signer(CHILD_KEY_BYTE);
        let session = signer(SESSION_KEY_BYTE);
        let parent = root_token(&root, AudienceDid::from(child.issuer().expect("child issuer")));
        let widened_resource = "clankers:session/other";
        let audience = AudienceDid::from(session.issuer().expect("session issuer"));
        let child_token = issue_token_with_signer(
            &child,
            &audience,
            &CapabilitySet::single(widened_resource, ABILITY).expect("widened capability"),
            &ProofCollection::from_tokens(vec![parent.clone()]),
            bounds(),
        )
        .expect("child token");
        let envelope = PublicCredentialEnvelope::new(child_token, vec![parent], audience, vec![
            root.issuer().expect("root issuer"),
        ]);

        match envelope.verify_with_context(&context(&[&root, &child])) {
            Ok(verified) => assert!(
                !verified.authorize(widened_resource, ABILITY).is_allowed(),
                "widened child grant must not authorize beyond parent"
            ),
            Err(_error) => {}
        }
    }

    #[test]
    fn malformed_base64_and_json_are_rejected() {
        let bad_base64 = PublicCredentialEnvelope::from_base64("not public ucan").expect_err("bad base64");
        assert!(matches!(bad_base64, PublicCredentialError::Decode { .. }));

        let bad_json = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(br#"{"schema":42}"#);
        let error = PublicCredentialEnvelope::from_base64(bad_json.as_str()).expect_err("bad json");
        assert!(matches!(error, PublicCredentialError::Decode { .. }));
    }

    #[test]
    fn expired_and_not_before_tokens_are_rejected() {
        let root = signer(ROOT_KEY_BYTE);
        let session = signer(SESSION_KEY_BYTE);
        let audience = AudienceDid::from(session.issuer().expect("session issuer"));
        let expired = issue_token_with_signer(
            &root,
            &audience,
            &CapabilitySet::single(RESOURCE, ABILITY).expect("capability"),
            &ProofCollection::empty(),
            TokenTimeBounds::new(1, 2).expect("expired bounds"),
        )
        .expect("expired token");
        let expired_envelope = PublicCredentialEnvelope::new(expired, Vec::new(), audience.clone(), vec![
            root.issuer().expect("root issuer"),
        ]);
        let future = issue_token_with_signer(
            &root,
            &audience,
            &CapabilitySet::single(RESOURCE, ABILITY).expect("capability"),
            &ProofCollection::empty(),
            TokenTimeBounds::new(VERIFY_AT_SECONDS + 10, VERIFY_AT_SECONDS + 20).expect("future bounds"),
        )
        .expect("future token");
        let future_envelope =
            PublicCredentialEnvelope::new(future, Vec::new(), audience, vec![root.issuer().expect("root issuer")]);

        assert!(expired_envelope.verify_with_context(&context(&[&root])).is_err());
        assert!(future_envelope.verify_with_context(&context(&[&root])).is_err());
    }

    #[test]
    fn unsupported_schema_is_rejected() {
        let root = signer(ROOT_KEY_BYTE);
        let session = signer(SESSION_KEY_BYTE);
        let audience = AudienceDid::from(session.issuer().expect("session issuer"));
        let token = root_token(&root, audience.clone());
        let mut envelope = PublicCredentialEnvelope::new(token, Vec::new(), audience, Vec::new());
        envelope.schema = "clankers.ucan.credential.v0".to_owned();

        let error = envelope.encode().expect_err("unsupported schema");

        assert!(matches!(error, PublicCredentialError::UnsupportedVersion { .. }));
    }

    #[test]
    fn wrong_audience_metadata_is_rejected_after_verification() {
        let root = signer(ROOT_KEY_BYTE);
        let session = signer(SESSION_KEY_BYTE);
        let other = signer(CHILD_KEY_BYTE);
        let token_audience = AudienceDid::from(session.issuer().expect("session issuer"));
        let token = root_token(&root, token_audience);
        let wrong_audience = AudienceDid::from(other.issuer().expect("other issuer"));
        let envelope =
            PublicCredentialEnvelope::new(token, Vec::new(), wrong_audience, vec![root.issuer().expect("root issuer")]);

        let error = envelope.verify_with_context(&context(&[&root])).expect_err("audience mismatch");

        assert!(matches!(error, PublicCredentialError::AudienceMismatch { .. }));
    }

    #[test]
    fn did_key_resolver_verifies_without_external_key_context() {
        let root = signer(ROOT_KEY_BYTE);
        let session = signer(SESSION_KEY_BYTE);
        let audience = AudienceDid::from(session.issuer().expect("session issuer"));
        let token = root_token(&root, audience.clone());
        let envelope =
            PublicCredentialEnvelope::new(token, Vec::new(), audience, vec![root.issuer().expect("root issuer")]);

        let verified = envelope
            .verify_with_did_keys(VerificationTime::from_unix_seconds(VERIFY_AT_SECONDS))
            .expect("verify with did:key resolver");

        assert_eq!(verified.issuer(), &root.issuer().expect("root issuer"));
    }

    #[test]
    fn untrusted_root_is_rejected() {
        let root = signer(ROOT_KEY_BYTE);
        let other = signer(CHILD_KEY_BYTE);
        let session = signer(SESSION_KEY_BYTE);
        let audience = AudienceDid::from(session.issuer().expect("session issuer"));
        let token = root_token(&root, audience.clone());
        let envelope =
            PublicCredentialEnvelope::new(token, Vec::new(), audience, vec![other.issuer().expect("other issuer")]);

        let error = envelope.verify_with_context(&context(&[&root])).expect_err("untrusted root");

        assert!(matches!(error, PublicCredentialError::UntrustedRoot { .. }));
    }

    #[test]
    fn unreferenced_trusted_proof_does_not_anchor_untrusted_leaf() {
        let root = signer(ROOT_KEY_BYTE);
        let other = signer(CHILD_KEY_BYTE);
        let session = signer(SESSION_KEY_BYTE);
        let audience = AudienceDid::from(session.issuer().expect("session issuer"));
        let untrusted_leaf = root_token(&other, audience.clone());
        let unrelated_trusted_proof = root_token(&root, AudienceDid::from(root.issuer().expect("root issuer")));
        let envelope = PublicCredentialEnvelope::new(untrusted_leaf, vec![unrelated_trusted_proof], audience, vec![
            root.issuer().expect("root issuer"),
        ]);

        let error = envelope
            .verify_with_context(&context(&[&root, &other]))
            .expect_err("unreferenced proof must not satisfy trust root");

        assert!(matches!(error, PublicCredentialError::UntrustedRoot { .. }));
    }

    #[test]
    fn legacy_clanker_auth_credential_base64_is_not_public_ucan_envelope() {
        let legacy_key = iroh::SecretKey::from([31u8; 32]);
        let token = crate::TokenBuilder::new(legacy_key)
            .with_capability(crate::Capability::Prompt)
            .build_at(1_700_000_000)
            .expect("legacy token");
        let legacy = crate::Credential::from_root(token).to_base64().expect("legacy base64");

        let error = PublicCredentialEnvelope::from_base64(legacy.as_str()).expect_err("legacy rejected");

        assert!(matches!(error, PublicCredentialError::Decode { .. }));
    }
}
