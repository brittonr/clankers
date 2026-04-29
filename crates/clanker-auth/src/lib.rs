//! UCAN-inspired capability tokens over iroh Ed25519 identity.
//!
//! Generic token infrastructure for signing, verifying, and delegating
//! capability tokens. The capability type itself is a generic parameter —
//! consumers define their own capability enums and implement the [`Cap`] trait.
//!
//! # Usage
//!
//! ```rust,ignore
//! use clanker_auth::{Cap, TokenBuilder, TokenVerifier, Audience};
//!
//! // Define your capability type
//! #[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
//! enum MyCap { Read, Write, Admin }
//!
//! impl Cap for MyCap {
//!     type Operation = MyOp;
//!     fn authorizes(&self, op: &MyOp) -> bool { /* ... */ }
//!     fn contains(&self, other: &MyCap) -> bool { /* ... */ }
//!     fn is_delegate(&self) -> bool { matches!(self, MyCap::Admin) }
//! }
//!
//! // Create and verify tokens
//! let token = TokenBuilder::<MyCap>::new(secret_key)
//!     .with_capability(MyCap::Read)
//!     .build()?;
//!
//! let verifier = TokenVerifier::<MyCap>::new();
//! verifier.verify(&token, None)?;
//! ```

mod builder;
pub mod constants;
mod credential;
mod error;
mod token;
pub mod utils;
mod verifier;

use std::fmt::Debug;

pub use builder::TokenBuilder;
pub use builder::bytes_to_sign;
pub use credential::Credential;
pub use credential::MAX_CREDENTIAL_SIZE;
pub use error::AuthError;
use serde::Serialize;
use serde::de::DeserializeOwned;
pub use token::Audience;
pub use token::CapabilityToken;
pub use verifier::TokenVerifier;

/// Trait for capability types used in tokens.
///
/// Implement this for your domain-specific capability enum. The token
/// infrastructure handles signing, verification, expiry, revocation,
/// and delegation chains — this trait defines what "authorized" and
/// "delegation subset" mean for your domain.
pub trait Cap: Serialize + DeserializeOwned + Clone + Debug + PartialEq + Send + Sync {
    /// The operation type that capabilities authorize.
    type Operation: Debug;

    /// Does this capability authorize the given operation?
    fn authorizes(&self, op: &Self::Operation) -> bool;

    /// Can this capability be delegated to produce `child`?
    ///
    /// Returns true if `child` is a subset of (or equal to) `self`.
    /// Used during delegation to prevent privilege escalation.
    fn contains(&self, child: &Self) -> bool;

    /// Is this the "delegate" capability that permits creating child tokens?
    ///
    /// At least one capability in the parent token must return true here
    /// for delegation to be allowed.
    fn is_delegate(&self) -> bool;
}

/// Storage backend for token revocation list.
///
/// Implementations provide persistent storage for revoked token hashes,
/// ensuring revocations survive restarts.
pub trait RevocationStore: Send + Sync {
    /// Check if a token hash is revoked.
    fn is_revoked(&self, token_hash: &[u8; 32]) -> bool;

    /// Add a token hash to the revocation list.
    fn revoke(&self, hash: [u8; 32], timestamp: u64);

    /// Load all revoked token hashes from storage.
    fn load_all(&self) -> Vec<[u8; 32]>;
}

#[cfg(test)]
mod tests;
