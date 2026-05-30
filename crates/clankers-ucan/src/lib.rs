//! Clankers-specific capability vocabulary and UCAN auth adapters.
//!
//! Provides:
//! - `Capability` enum with legacy Clankers-specific variants
//! - public UCAN credential envelopes for daemon/remote auth migration
//! - Basalt-backed public UCAN admission receipts
//! - `RedbRevocationStore` for persistent revocation
//! - legacy `clanker-auth` type aliases retained during migration
#![allow(unexpected_cfgs)]
#![cfg_attr(dylint_lib = "tigerstyle", feature(register_tool), register_tool(tigerstyle))]
#![cfg_attr(
    dylint_lib = "tigerstyle",
    allow(
        tigerstyle::assertion_density,
        tigerstyle::ambiguous_params,
        tigerstyle::raw_arithmetic_overflow,
        tigerstyle::unbounded_collection_growth,
        tigerstyle::ignored_result,
        reason = "UCAN vocabulary and capability APIs are compatibility surfaces with focused authorization tests"
    )
)]

pub mod basalt_authority;
mod capability;
pub mod constants;
pub mod effect_vocabulary;
#[cfg(feature = "external-ucan")]
pub mod external_adapter;
#[cfg(feature = "external-ucan")]
pub mod external_caveats;
pub mod public_credential;
pub mod public_issuer;
pub mod public_store;
pub mod revocation;
#[cfg(feature = "runtime-admission")]
pub mod runtime_admission;
pub mod utils;

// Re-export generic infrastructure
// Domain-specific types
pub use basalt_authority::BasaltAdmissionReceipt;
pub use basalt_authority::BasaltAdmissionRequest;
pub use basalt_authority::BasaltUcanAuthority;
pub use basalt_authority::CLANKERS_DAEMON_AUTH_POLICY_JSON;
pub use basalt_authority::clankers_daemon_auth_policy;
pub use capability::Capability;
pub use capability::Operation;
pub use clanker_auth::Audience;
pub use clanker_auth::AuthError;
pub use clanker_auth::Cap;
pub use clanker_auth::MAX_CREDENTIAL_SIZE;
pub use clanker_auth::RevocationStore;
pub use clanker_auth::bytes_to_sign;
pub use effect_vocabulary::EffectCapability;
pub use effect_vocabulary::EffectKind;
pub use public_credential::PublicCredentialEnvelope;
pub use public_credential::PublicCredentialError;
pub use public_issuer::PublicIssuerError;
pub use public_issuer::PublicUcanIssuer;
pub use public_issuer::decode_public_credential_base64;
pub use public_issuer::encode_public_credential_base64;
pub use public_issuer::revocation_reference_for;
pub use public_store::RedbPublicCredentialStore;
pub use public_store::ReplayAdmissionStatus;
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

#[cfg(test)]
mod dependency_source_tests {
    const CRATE_MANIFEST: &str = include_str!("../Cargo.toml");
    const WORKSPACE_MANIFEST: &str = include_str!("../../../Cargo.toml");

    #[test]
    fn public_ucan_dependency_uses_workspace_remote_pin_not_sibling_path() {
        assert!(!CRATE_MANIFEST.contains("../../../ucan"));
        assert!(CRATE_MANIFEST.contains("ucan = { workspace = true }"));
        assert!(WORKSPACE_MANIFEST.contains("git = \"ssh://git@github.com/OnixResearch/ucan.git\""));
        assert!(WORKSPACE_MANIFEST.contains("rev = \"ad61b53e89fa45f9bf7d313ce14c45de645bf53d\""));
    }
}
