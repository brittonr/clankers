//! Public UCAN issuer helpers for daemon-owner credentials.
//!
//! The daemon owns an iroh Ed25519 identity.  This adapter reuses the same seed
//! through OnixResearch `ucan::TokenSigner` so remote credentials can be issued
//! as public UCAN compact tokens instead of legacy `clanker-auth` credentials.

use std::fmt;
use std::time::Duration;

use crate::public_credential::PublicCredentialEnvelope;

#[derive(Debug, Clone)]
pub struct PublicUcanIssuer {
    signer: ucan::Ed25519InMemorySigner,
}

impl PublicUcanIssuer {
    #[must_use]
    pub fn from_iroh_secret_key(secret_key: &iroh::SecretKey) -> Self {
        Self {
            signer: ucan::Ed25519InMemorySigner::from_seed_bytes(secret_key.to_bytes()),
        }
    }

    #[must_use]
    pub const fn from_signer(signer: ucan::Ed25519InMemorySigner) -> Self {
        Self { signer }
    }

    pub fn issuer(&self) -> PublicIssuerResult<ucan::IssuerDid> {
        ucan::TokenSigner::issuer(&self.signer).map_err(PublicIssuerError::from)
    }

    pub fn audience(&self) -> PublicIssuerResult<ucan::AudienceDid> {
        Ok(ucan::AudienceDid::from(self.issuer()?))
    }

    pub fn verification_key(&self) -> PublicIssuerResult<ucan::Ed25519VerificationKey> {
        self.signer.verification_key().map_err(PublicIssuerError::from)
    }

    pub fn issue_root_credential_at(
        &self,
        audience: ucan::AudienceDid,
        capabilities: ucan::CapabilitySet,
        lifetime: Duration,
        issued_at_seconds: u64,
    ) -> PublicIssuerResult<PublicCredentialEnvelope> {
        let bounds = token_time_bounds_at(lifetime, issued_at_seconds)?;
        let token = ucan::issue_token_with_signer(
            &self.signer,
            &audience,
            &capabilities,
            &ucan::ProofCollection::empty(),
            bounds,
        )?;
        Ok(PublicCredentialEnvelope::new(token, Vec::new(), audience, vec![self.issuer()?]))
    }

    pub fn issue_delegated_credential_at(
        &self,
        audience: ucan::AudienceDid,
        capabilities: ucan::CapabilitySet,
        proofs: Vec<ucan::CompactToken>,
        lifetime: Duration,
        issued_at_seconds: u64,
    ) -> PublicIssuerResult<PublicCredentialEnvelope> {
        let bounds = token_time_bounds_at(lifetime, issued_at_seconds)?;
        let trusted_roots = trusted_roots_from_proofs(&proofs)?;
        let token = ucan::issue_token_with_signer(
            &self.signer,
            &audience,
            &capabilities,
            &ucan::ProofCollection::from_tokens(proofs.clone()),
            bounds,
        )?;
        Ok(PublicCredentialEnvelope::new(token, proofs, audience, trusted_roots))
    }

    pub fn delegate_to_at(
        &self,
        delegate: &PublicUcanIssuer,
        capabilities: ucan::CapabilitySet,
        lifetime: Duration,
        issued_at_seconds: u64,
    ) -> PublicIssuerResult<PublicCredentialEnvelope> {
        self.issue_root_credential_at(delegate.audience()?, capabilities, lifetime, issued_at_seconds)
    }

    pub fn issue_child_from_parent_at(
        &self,
        parent: &PublicCredentialEnvelope,
        audience: ucan::AudienceDid,
        capabilities: ucan::CapabilitySet,
        lifetime: Duration,
        issued_at_seconds: u64,
    ) -> PublicIssuerResult<PublicCredentialEnvelope> {
        let mut proofs = Vec::with_capacity(parent.proofs().len().saturating_add(1));
        proofs.push(parent.token().clone());
        proofs.extend_from_slice(parent.proofs());
        let mut envelope = self.issue_delegated_credential_at(audience, capabilities, proofs, lifetime, issued_at_seconds)?;
        envelope.set_trusted_roots(parent.trusted_roots().to_vec());
        Ok(envelope)
    }
}

pub fn decode_public_credential_base64(input: &str) -> PublicIssuerResult<PublicCredentialEnvelope> {
    PublicCredentialEnvelope::from_base64(input).map_err(PublicIssuerError::Credential)
}

pub fn encode_public_credential_base64(envelope: &PublicCredentialEnvelope) -> PublicIssuerResult<String> {
    envelope.to_base64().map_err(PublicIssuerError::Credential)
}

#[must_use]
pub fn revocation_reference_for(envelope: &PublicCredentialEnvelope) -> ucan::ProofReference {
    envelope.token_reference()
}

fn token_time_bounds_at(lifetime: Duration, issued_at_seconds: u64) -> PublicIssuerResult<ucan::TokenTimeBounds> {
    ucan::TokenTimeBounds::from_unix_seconds_and_duration(issued_at_seconds, lifetime).map_err(|source| {
        PublicIssuerError::Time {
            message: source.to_string(),
        }
    })
}

fn trusted_roots_from_proofs(proofs: &[ucan::CompactToken]) -> PublicIssuerResult<Vec<ucan::IssuerDid>> {
    let Some(first) = proofs.last() else {
        return Err(PublicIssuerError::MissingParentProof);
    };
    let decoded = ucan::parse_compact_token(first)?;
    let valid = ucan::validate_decoded_token(decoded)?;
    let issuer = ucan::IssuerDid::new(valid.claims().issuer_did().to_owned())?;
    Ok(vec![issuer])
}

#[derive(Debug)]
pub enum PublicIssuerError {
    Credential(crate::public_credential::PublicCredentialError),
    MissingParentProof,
    Time { message: String },
    Token { source: ucan::TokenError },
}

impl fmt::Display for PublicIssuerError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Credential(error) => write!(formatter, "{error}"),
            Self::MissingParentProof => formatter.write_str("delegated public UCAN credential needs a parent proof"),
            Self::Time { message } => write!(formatter, "failed to build public UCAN time bounds: {message}"),
            Self::Token { source } => write!(formatter, "public UCAN issuance failed: {source}"),
        }
    }
}

impl std::error::Error for PublicIssuerError {}

impl From<ucan::TokenError> for PublicIssuerError {
    fn from(source: ucan::TokenError) -> Self {
        Self::Token { source }
    }
}

pub type PublicIssuerResult<T> = Result<T, PublicIssuerError>;

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use ucan::CapabilitySet;
    use ucan::VerificationTime;

    use super::*;

    const ROOT_KEY_BYTE: u8 = 41;
    const CHILD_KEY_BYTE: u8 = 43;
    const SESSION_KEY_BYTE: u8 = 47;
    const ISSUED_AT_SECONDS: u64 = 1_000;
    const VERIFY_AT_SECONDS: u64 = 1_001;
    const RESOURCE: &str = "clankers:session/demo";
    const ABILITY: &str = "session/attach";

    fn issuer(byte: u8) -> PublicUcanIssuer {
        PublicUcanIssuer::from_signer(ucan::Ed25519InMemorySigner::from_seed_bytes(
            [byte; ucan::ED25519_SECRET_KEY_BYTES],
        ))
    }

    fn caps() -> CapabilitySet {
        CapabilitySet::single(RESOURCE, ABILITY).expect("capability")
    }

    #[test]
    fn iroh_secret_key_maps_to_same_did_as_ucan_signer() {
        let secret = iroh::SecretKey::from([ROOT_KEY_BYTE; 32]);
        let from_iroh = PublicUcanIssuer::from_iroh_secret_key(&secret);
        let from_ucan = issuer(ROOT_KEY_BYTE);

        assert_eq!(from_iroh.issuer().expect("iroh issuer"), from_ucan.issuer().expect("ucan issuer"));
    }

    #[test]
    fn root_credential_round_trips_and_verifies() {
        let root = issuer(ROOT_KEY_BYTE);
        let session = issuer(SESSION_KEY_BYTE);
        let envelope = root
            .issue_root_credential_at(
                session.audience().expect("session audience"),
                caps(),
                Duration::from_secs(60),
                ISSUED_AT_SECONDS,
            )
            .expect("root credential");
        let encoded = encode_public_credential_base64(&envelope).expect("encode");
        let decoded = decode_public_credential_base64(encoded.as_str()).expect("decode");

        let verified = decoded
            .verify_with_did_keys(VerificationTime::from_unix_seconds(VERIFY_AT_SECONDS))
            .expect("verify");

        assert_eq!(verified.audience(), decoded.audience());
        assert_eq!(decoded.trusted_roots(), &[root.issuer().expect("root issuer")]);
    }

    #[test]
    fn child_credential_is_anchored_to_parent_root() {
        let root = issuer(ROOT_KEY_BYTE);
        let child = issuer(CHILD_KEY_BYTE);
        let session = issuer(SESSION_KEY_BYTE);
        let parent = root
            .delegate_to_at(&child, caps(), Duration::from_secs(60), ISSUED_AT_SECONDS)
            .expect("parent credential");
        let child_envelope = child
            .issue_child_from_parent_at(
                &parent,
                session.audience().expect("session audience"),
                caps(),
                Duration::from_secs(60),
                ISSUED_AT_SECONDS,
            )
            .expect("child credential");

        let verified = child_envelope
            .verify_with_did_keys(VerificationTime::from_unix_seconds(VERIFY_AT_SECONDS))
            .expect("verify child");

        assert_eq!(verified.issuer(), &child.issuer().expect("child issuer"));
        assert_eq!(child_envelope.trusted_roots(), parent.trusted_roots());
        assert_eq!(child_envelope.proofs().len(), 1);
    }
}
