//! Self-contained credential type for offline chain verification.
//!
//! A `Credential` bundles a capability token with its full delegation proof chain,
//! so the verifier can walk root→leaf using only the credential contents.
//! No server-side parent cache or network calls needed.

use std::time::Duration;

use iroh::PublicKey;
use iroh::SecretKey;
use serde::Deserialize;
use serde::Serialize;

use crate::Cap;
use crate::TokenBuilder;
use crate::TokenVerifier;
use crate::constants::MAX_DELEGATION_DEPTH_USIZE;
use crate::constants::MAX_TOKEN_SIZE_USIZE;
use crate::error::AuthError;
use crate::token::Audience;
use crate::token::CapabilityToken;

/// Maximum credential size: bounded by delegation depth × token size.
pub const MAX_CREDENTIAL_SIZE: usize = MAX_DELEGATION_DEPTH_USIZE * MAX_TOKEN_SIZE_USIZE;

/// A self-contained credential for offline chain verification.
///
/// Bundles a leaf capability token with its delegation proof chain
/// (ordered from immediate parent to root). Verifiable using only
/// crypto operations on the credential contents + trusted root keys.
///
/// # Wire encoding
///
/// Uses postcard for compact binary serialization. Size bounded by
/// `MAX_DELEGATION_DEPTH × MAX_TOKEN_SIZE` (~64KB worst case, typically ~3KB).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(bound = "C: Cap")]
pub struct Credential<C: Cap> {
    /// The leaf capability token being presented.
    pub token: CapabilityToken<C>,
    /// Delegation proof chain, ordered from immediate parent to root.
    /// Empty for root tokens (depth 0).
    pub proofs: Vec<CapabilityToken<C>>,
}

impl<C: Cap> Credential<C> {
    /// Create a root credential (depth 0, no delegation chain).
    pub fn from_root(token: CapabilityToken<C>) -> Self {
        Self {
            token,
            proofs: Vec::new(),
        }
    }

    // r[impl auth.credential.self-contained]
    /// Verify the credential's token and full delegation chain.
    ///
    /// Walks from leaf to root, checking signatures, expiry, and
    /// capability attenuation at each level. Stateless — no parent
    /// cache or network calls needed.
    pub fn verify(&self, trusted_roots: &[PublicKey], presenter: Option<&PublicKey>) -> Result<(), AuthError> {
        let mut verifier = TokenVerifier::new();
        for root in trusted_roots {
            verifier = verifier.with_trusted_root(*root);
        }
        verifier.verify_with_chain(&self.token, &self.proofs, presenter)
    }

    /// Delegate this credential to create a child credential.
    ///
    /// The child gets narrower capabilities and a new audience.
    /// The proof chain grows by one (current leaf becomes a proof).
    ///
    /// Fails if delegation depth would exceed `MAX_DELEGATION_DEPTH`,
    /// capabilities aren't a subset, or the current token lacks Delegate.
    pub fn delegate(
        &self,
        issuer_key: &SecretKey,
        audience: PublicKey,
        capabilities: Vec<C>,
        lifetime: Duration,
    ) -> Result<Self, AuthError> {
        let child_token = TokenBuilder::new(issuer_key.clone())
            .for_key(audience)
            .with_capabilities(capabilities)
            .with_lifetime(lifetime)
            .with_random_nonce()
            .delegated_from(self.token.clone())
            .build()?;

        let mut new_proofs = Vec::with_capacity(self.proofs.len().saturating_add(1));
        new_proofs.push(self.token.clone());
        new_proofs.extend(self.proofs.iter().cloned());

        Ok(Self {
            token: child_token,
            proofs: new_proofs,
        })
    }

    /// Delegate with bearer audience (no specific recipient key).
    pub fn delegate_bearer(
        &self,
        issuer_key: &SecretKey,
        capabilities: Vec<C>,
        lifetime: Duration,
    ) -> Result<Self, AuthError> {
        let child_token = TokenBuilder::new(issuer_key.clone())
            .for_audience(Audience::Bearer)
            .with_capabilities(capabilities)
            .with_lifetime(lifetime)
            .with_random_nonce()
            .delegated_from(self.token.clone())
            .build()?;

        let mut new_proofs = Vec::with_capacity(self.proofs.len().saturating_add(1));
        new_proofs.push(self.token.clone());
        new_proofs.extend(self.proofs.iter().cloned());

        Ok(Self {
            token: child_token,
            proofs: new_proofs,
        })
    }

    /// Encode credential to bytes for wire transmission.
    pub fn encode(&self) -> Result<Vec<u8>, AuthError> {
        let bytes = postcard::to_allocvec(self).map_err(|e| AuthError::EncodingError(e.to_string()))?;
        if bytes.len() > MAX_CREDENTIAL_SIZE {
            return Err(AuthError::TokenTooLarge {
                size_bytes: bytes.len() as u64,
                max_bytes: MAX_CREDENTIAL_SIZE as u64,
            });
        }
        Ok(bytes)
    }

    /// Decode credential from bytes.
    pub fn decode(bytes: &[u8]) -> Result<Self, AuthError> {
        if bytes.len() > MAX_CREDENTIAL_SIZE {
            return Err(AuthError::TokenTooLarge {
                size_bytes: bytes.len() as u64,
                max_bytes: MAX_CREDENTIAL_SIZE as u64,
            });
        }
        Ok(postcard::from_bytes(bytes)?)
    }

    /// Encode to base64 for text transmission.
    pub fn to_base64(&self) -> Result<String, AuthError> {
        use base64::Engine;
        Ok(base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(self.encode()?))
    }

    /// Decode from base64.
    pub fn from_base64(s: &str) -> Result<Self, AuthError> {
        use base64::Engine;
        let bytes = base64::engine::general_purpose::URL_SAFE_NO_PAD.decode(s)?;
        Self::decode(&bytes)
    }
}
