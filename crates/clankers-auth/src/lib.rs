//! Capability-based authorization for clankers agents.
//!
//! This crate implements UCAN-inspired capability tokens that enable
//! decentralized authorization without a central user database.
//!
//! # Design Principles
//!
//! 1. **Reuse Iroh's identity**: `NodeId` is already an Ed25519 public key
//! 2. **Self-contained tokens**: No database lookup needed for authorization
//! 3. **Delegation by default**: Tokens can create child tokens with fewer permissions
//! 4. **Offline verification**: Works without contacting the cluster
//! 5. **Tiger Style**: Fixed bounds on token size, delegation depth, capability count
//!
//! # Usage
//!
//! ```rust,ignore
//! use clankers_auth::{TokenBuilder, TokenVerifier, Capability, Audience};
//!
//! // Create a root token
//! let token = TokenBuilder::new(secret_key)
//!     .with_capability(Capability::Prompt)
//!     .with_capability(Capability::Delegate)
//!     .with_lifetime(Duration::from_secs(3600))
//!     .build()?;
//!
//! // Verify and authorize
//! let verifier = TokenVerifier::new();
//! verifier.authorize(&token, &Operation::Prompt { text: "hello".into() }, None)?;
//! ```

mod builder;
mod capability;
pub mod constants;
mod error;
pub mod revocation;
mod token;
pub mod utils;
mod verifier;

pub use builder::generate_root_token;
pub use builder::TokenBuilder;
pub use capability::Capability;
pub use capability::Operation;
pub use error::AuthError;
pub use revocation::RedbRevocationStore;
pub use revocation::RevocationStore;
pub use token::Audience;
pub use token::CapabilityToken;
pub use verifier::TokenVerifier;

#[cfg(test)]
mod tests;
