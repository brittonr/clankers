//! Clankers-specific capability tokens over clanker-auth generic infrastructure.
//!
//! Re-exports the generic token types and provides:
//! - `Capability` enum with clankers-specific variants
//! - `Operation` enum for authorization checks
//! - `RedbRevocationStore` for persistent revocation
//! - `generate_root_token()` for bootstrap
//! - Type aliases: `CapabilityToken`, `TokenBuilder`, `TokenVerifier`

mod capability;
pub mod constants;
pub mod revocation;
pub mod utils;

// Re-export generic infrastructure
// Domain-specific types
pub use capability::Capability;
pub use capability::Operation;
pub use clanker_auth::Audience;
pub use clanker_auth::AuthError;
pub use clanker_auth::Cap;
pub use clanker_auth::MAX_CREDENTIAL_SIZE;
pub use clanker_auth::RevocationStore;
pub use clanker_auth::bytes_to_sign;
pub use revocation::RedbRevocationStore;

// Type aliases — callers use these without specifying the generic parameter
pub type CapabilityToken = clanker_auth::CapabilityToken<Capability>;
pub type TokenBuilder = clanker_auth::TokenBuilder<Capability>;
pub type TokenVerifier = clanker_auth::TokenVerifier<Capability>;
pub type Credential = clanker_auth::Credential<Capability>;

/// Generate a root capability token with full clankers agent access.
pub fn generate_root_token(
    secret_key: &iroh::SecretKey,
    lifetime: std::time::Duration,
) -> Result<CapabilityToken, AuthError> {
    use rand::RngCore;
    TokenBuilder::new(secret_key.clone())
        .with_capability(Capability::Prompt)
        .with_capability(Capability::ToolUse {
            tool_pattern: "*".into(),
        })
        .with_capability(Capability::ShellExecute {
            command_pattern: "*".into(),
            working_dir: None,
        })
        .with_capability(Capability::FileAccess {
            prefix: "/".into(),
            read_only: false,
        })
        .with_capability(Capability::BotCommand {
            command_pattern: "*".into(),
        })
        .with_capability(Capability::SessionManage)
        .with_capability(Capability::ModelSwitch)
        .with_capability(Capability::Delegate)
        .with_lifetime(lifetime)
        .with_nonce({
            let mut nonce = [0u8; 16];
            rand::rng().fill_bytes(&mut nonce);
            nonce
        })
        .build()
}

#[cfg(test)]
mod tests;
