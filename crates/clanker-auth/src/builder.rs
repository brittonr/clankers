//! Token builder for creating capability tokens.

use std::time::Duration;

use iroh::PublicKey;
use iroh::SecretKey;
use rand::RngCore;

use crate::Cap;
use crate::constants::MAX_CAPABILITIES_PER_TOKEN;
use crate::constants::MAX_DELEGATION_DEPTH;
use crate::error::AuthError;
use crate::token::Audience;
use crate::token::CapabilityToken;
use crate::utils::current_time_secs;

/// Builder for creating capability tokens.
pub struct TokenBuilder<C: Cap> {
    issuer_key: SecretKey,
    audience: Audience,
    capabilities: Vec<C>,
    lifetime: Duration,
    nonce: Option<[u8; 16]>,
    parent: Option<CapabilityToken<C>>,
}

impl<C: Cap> TokenBuilder<C> {
    /// Create a new token builder.
    pub fn new(issuer_key: SecretKey) -> Self {
        Self {
            issuer_key,
            audience: Audience::Bearer,
            capabilities: Vec::new(),
            lifetime: Duration::from_secs(3600),
            nonce: None,
            parent: None,
        }
    }

    /// Set the audience.
    pub fn for_audience(mut self, audience: Audience) -> Self {
        self.audience = audience;
        self
    }

    /// Set audience to a specific public key.
    pub fn for_key(mut self, key: PublicKey) -> Self {
        self.audience = Audience::Key(key);
        self
    }

    /// Add a capability.
    pub fn with_capability(mut self, cap: C) -> Self {
        self.capabilities.push(cap);
        self
    }

    /// Add multiple capabilities.
    pub fn with_capabilities(mut self, caps: impl IntoIterator<Item = C>) -> Self {
        self.capabilities.extend(caps);
        self
    }

    /// Set token lifetime.
    pub fn with_lifetime(mut self, lifetime: Duration) -> Self {
        self.lifetime = lifetime;
        self
    }

    /// Set a specific nonce.
    pub fn with_nonce(mut self, nonce: [u8; 16]) -> Self {
        self.nonce = Some(nonce);
        self
    }

    /// Generate a random nonce.
    pub fn with_random_nonce(mut self) -> Self {
        let mut nonce = [0u8; 16];
        rand::rng().fill_bytes(&mut nonce);
        self.nonce = Some(nonce);
        self
    }

    /// Delegate from a parent token.
    pub fn delegated_from(mut self, parent: CapabilityToken<C>) -> Self {
        self.parent = Some(parent);
        self
    }

    /// Build and sign the token.
    pub fn build(self) -> Result<CapabilityToken<C>, AuthError> {
        if self.capabilities.len() > MAX_CAPABILITIES_PER_TOKEN as usize {
            return Err(AuthError::TooManyCapabilities {
                count: self.capabilities.len() as u32,
                max: MAX_CAPABILITIES_PER_TOKEN,
            });
        }

        let delegation_depth = if let Some(ref parent) = self.parent {
            // r[impl auth.build.depth-bound]
            let new_depth = parent.delegation_depth.saturating_add(1);
            if new_depth > MAX_DELEGATION_DEPTH {
                return Err(AuthError::DelegationTooDeep {
                    depth: new_depth,
                    max: MAX_DELEGATION_DEPTH,
                });
            }

            // r[impl auth.build.no-escalation]
            // r[impl auth.delegation.transitivity]
            for cap in &self.capabilities {
                if !parent.capabilities.iter().any(|p| p.contains(cap)) {
                    return Err(AuthError::CapabilityEscalation {
                        requested: format!("{:?}", cap),
                    });
                }
            }

            // r[impl auth.build.delegate-required]
            if !parent.capabilities.iter().any(Cap::is_delegate) {
                return Err(AuthError::DelegationNotAllowed);
            }

            new_depth
        } else {
            0
        };

        let now = current_time_secs();

        let mut token = CapabilityToken::new(
            1,
            self.issuer_key.public(),
            self.audience,
            self.capabilities,
            now,
            now + self.lifetime.as_secs(),
            self.nonce,
            self.parent.as_ref().map(|p| p.hash()).transpose()?,
            delegation_depth,
            [0u8; 64],
        );

        let sign_bytes = bytes_to_sign(&token);
        let signature = self.issuer_key.sign(&sign_bytes);
        token.signature = signature.to_bytes();

        Ok(token)
    }
}

/// Compute bytes to sign for a token.
///
/// Signs everything except the signature field itself.
pub fn bytes_to_sign<C: Cap>(token: &CapabilityToken<C>) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(256);

    bytes.push(token.version);
    bytes.extend_from_slice(token.issuer.as_bytes());

    let audience_bytes = postcard::to_allocvec(&token.audience).expect("audience must be serializable");
    bytes.extend_from_slice(&audience_bytes);

    let cap_bytes = postcard::to_allocvec(&token.capabilities).expect("capabilities must be serializable");
    bytes.extend_from_slice(&cap_bytes);

    bytes.extend_from_slice(&token.issued_at.to_le_bytes());
    bytes.extend_from_slice(&token.expires_at.to_le_bytes());

    if let Some(nonce) = token.nonce {
        bytes.extend_from_slice(&nonce);
    }
    if let Some(proof) = token.proof {
        bytes.extend_from_slice(&proof);
    }

    bytes.push(token.delegation_depth);

    assert!(bytes.len() > 33 + 16);

    bytes
}
