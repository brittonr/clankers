//! Capability token structure and encoding.

use std::marker::PhantomData;

use iroh::PublicKey;
use serde::Deserialize;
use serde::Serialize;

use crate::Cap;
use crate::constants::MAX_TOKEN_SIZE;
use crate::constants::MAX_TOKEN_SIZE_USIZE;
use crate::error::AuthError;

/// Who can use this token.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum Audience {
    /// Only this specific public key can use the token.
    Key(#[serde(with = "public_key_serde")] PublicKey),
    /// Anyone holding the token can use it (bearer token).
    Bearer,
}

/// A capability token granting specific permissions.
///
/// Generic over the capability type `C`. The token handles signing,
/// encoding, and hash computation. The capability semantics (what
/// operations are authorized, what delegation subsets are valid)
/// are defined by the `Cap` implementation.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(bound = "C: Cap")]
pub struct CapabilityToken<C: Cap> {
    pub version: u8,
    #[serde(with = "public_key_serde")]
    pub issuer: PublicKey,
    pub audience: Audience,
    pub capabilities: Vec<C>,
    pub issued_at: u64,
    pub expires_at: u64,
    pub nonce: Option<[u8; 16]>,
    pub proof: Option<[u8; 32]>,
    #[serde(default)]
    pub delegation_depth: u8,
    #[serde(with = "signature_serde")]
    pub signature: [u8; 64],
    #[serde(skip)]
    _marker: PhantomData<C>,
}

pub(crate) struct CapabilityTokenParts<C: Cap> {
    pub(crate) version: u8,
    pub(crate) issuer: PublicKey,
    pub(crate) audience: Audience,
    pub(crate) capabilities: Vec<C>,
    pub(crate) issued_at: u64,
    pub(crate) expires_at: u64,
    pub(crate) nonce: Option<[u8; 16]>,
    pub(crate) proof: Option<[u8; 32]>,
    pub(crate) delegation_depth: u8,
    pub(crate) signature: [u8; 64],
}

impl<C: Cap> CapabilityToken<C> {
    /// Create a new token (used by the builder).
    pub(crate) fn from_parts(parts: CapabilityTokenParts<C>) -> Self {
        assert!(parts.version > 0);
        assert!(parts.expires_at >= parts.issued_at);
        Self {
            version: parts.version,
            issuer: parts.issuer,
            audience: parts.audience,
            capabilities: parts.capabilities,
            issued_at: parts.issued_at,
            expires_at: parts.expires_at,
            nonce: parts.nonce,
            proof: parts.proof,
            delegation_depth: parts.delegation_depth,
            signature: parts.signature,
            _marker: PhantomData,
        }
    }

    /// Encode token to bytes.
    pub fn encode(&self) -> Result<Vec<u8>, AuthError> {
        let bytes = postcard::to_allocvec(self).map_err(|e| AuthError::EncodingError(e.to_string()))?;
        if bytes.len() > MAX_TOKEN_SIZE_USIZE {
            return Err(AuthError::TokenTooLarge {
                size_bytes: bytes.len() as u64,
                max_bytes: u64::from(MAX_TOKEN_SIZE),
            });
        }
        Ok(bytes)
    }

    /// Decode token from bytes.
    pub fn decode(bytes: &[u8]) -> Result<Self, AuthError> {
        if bytes.len() > MAX_TOKEN_SIZE_USIZE {
            return Err(AuthError::TokenTooLarge {
                size_bytes: bytes.len() as u64,
                max_bytes: u64::from(MAX_TOKEN_SIZE),
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

    /// Compute BLAKE3 hash of this token.
    pub fn hash(&self) -> Result<[u8; 32], AuthError> {
        let bytes = self.encode()?;
        assert!(!bytes.is_empty());
        Ok(*blake3::hash(&bytes).as_bytes())
    }
}

/// Serde helper for PublicKey.
mod public_key_serde {
    use iroh::PublicKey;
    use serde::Deserialize;
    use serde::Deserializer;
    use serde::Serialize;
    use serde::Serializer;

    pub fn serialize<S: Serializer>(key: &PublicKey, s: S) -> Result<S::Ok, S::Error> {
        key.as_bytes().serialize(s)
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<PublicKey, D::Error> {
        let bytes: [u8; 32] = Deserialize::deserialize(d)?;
        PublicKey::try_from(&bytes[..]).map_err(serde::de::Error::custom)
    }
}

/// Serde helper for Ed25519 signatures (64 bytes).
mod signature_serde {
    use serde::Deserialize;
    use serde::Deserializer;
    use serde::Serializer;

    pub fn serialize<S: Serializer>(sig: &[u8; 64], s: S) -> Result<S::Ok, S::Error> {
        s.serialize_bytes(sig)
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<[u8; 64], D::Error> {
        let bytes: Vec<u8> = Deserialize::deserialize(d)?;
        if bytes.len() != 64 {
            return Err(serde::de::Error::custom(format!("expected 64 bytes, got {}", bytes.len())));
        }
        let mut sig = [0u8; 64];
        sig.copy_from_slice(&bytes);
        Ok(sig)
    }
}
