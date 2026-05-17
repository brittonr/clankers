//! Deterministic Clankers caveat policies for the public `ucan` adapter.

use std::fmt;

use ucan::AuthorizationRequest;
use ucan::CaveatDecision;
use ucan::CaveatDocument;
use ucan::CaveatIdentifier;
use ucan::CaveatPolicy;
use ucan::CaveatPolicySet;

pub const CLANKERS_CAVEAT_DOMAIN: &str = "clankers";
pub const CAVEAT_PATH_PREFIX: &str = "path-prefix";
pub const CAVEAT_COMMAND_PREFIX: &str = "command-prefix";
pub const CAVEAT_TIMEOUT_MS_AT_MOST: &str = "timeout-ms-at-most";
pub const CAVEAT_MAX_BYTES_AT_MOST: &str = "max-bytes-at-most";
pub const CAVEAT_NETWORK_HOST: &str = "network-host";
pub const CAVEAT_NETWORK_SCHEME: &str = "network-scheme";
pub const CAVEAT_PROVIDER: &str = "provider";
pub const CAVEAT_MODEL_PREFIX: &str = "model-prefix";

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct EffectCaveatContext {
    path: Option<String>,
    command: Option<String>,
    timeout_ms: Option<u64>,
    max_bytes: Option<u64>,
    network_host: Option<String>,
    network_scheme: Option<String>,
    provider: Option<String>,
    model: Option<String>,
}

impl EffectCaveatContext {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            path: None,
            command: None,
            timeout_ms: None,
            max_bytes: None,
            network_host: None,
            network_scheme: None,
            provider: None,
            model: None,
        }
    }

    #[must_use]
    pub fn with_path(mut self, path: impl Into<String>) -> Self {
        self.path = Some(path.into());
        self
    }

    #[must_use]
    pub fn with_command(mut self, command: impl Into<String>) -> Self {
        self.command = Some(command.into());
        self
    }

    #[must_use]
    pub const fn with_timeout_ms(mut self, timeout_ms: u64) -> Self {
        self.timeout_ms = Some(timeout_ms);
        self
    }

    #[must_use]
    pub const fn with_max_bytes(mut self, max_bytes: u64) -> Self {
        self.max_bytes = Some(max_bytes);
        self
    }

    #[must_use]
    pub fn with_network_host(mut self, host: impl Into<String>) -> Self {
        self.network_host = Some(host.into());
        self
    }

    #[must_use]
    pub fn with_network_scheme(mut self, scheme: impl Into<String>) -> Self {
        self.network_scheme = Some(scheme.into());
        self
    }

    #[must_use]
    pub fn with_provider(mut self, provider: impl Into<String>) -> Self {
        self.provider = Some(provider.into());
        self
    }

    #[must_use]
    pub fn with_model(mut self, model: impl Into<String>) -> Self {
        self.model = Some(model.into());
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CaveatHookError {
    InvalidPayloadUtf8,
    InvalidInteger { payload: String },
    UnsupportedCaveat { caveat_type: String },
}

impl fmt::Display for CaveatHookError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidPayloadUtf8 => formatter.write_str("caveat payload is not valid UTF-8"),
            Self::InvalidInteger { payload } => write!(formatter, "caveat payload is not a u64 integer: {payload}"),
            Self::UnsupportedCaveat { caveat_type } => write!(formatter, "unsupported Clankers caveat: {caveat_type}"),
        }
    }
}

impl std::error::Error for CaveatHookError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClankersCaveatPolicySet {
    context: EffectCaveatContext,
}

impl ClankersCaveatPolicySet {
    #[must_use]
    pub const fn new(context: EffectCaveatContext) -> Self {
        Self { context }
    }

    #[must_use]
    pub const fn context(&self) -> &EffectCaveatContext {
        &self.context
    }
}

impl CaveatPolicySet for ClankersCaveatPolicySet {
    fn policy_for(&self, caveat: &CaveatIdentifier) -> Option<&dyn CaveatPolicy> {
        if caveat.domain() == CLANKERS_CAVEAT_DOMAIN && is_supported_caveat(caveat.caveat_type()) {
            Some(self)
        } else {
            None
        }
    }
}

impl CaveatPolicy for ClankersCaveatPolicySet {
    fn evaluate(&self, caveat: &CaveatDocument, _request: &AuthorizationRequest) -> CaveatDecision {
        match evaluate_known_caveat(&self.context, caveat) {
            Ok(()) => CaveatDecision::Satisfied,
            Err(error) => CaveatDecision::Rejected {
                message: error.to_string(),
            },
        }
    }
}

pub fn path_prefix_caveat(prefix: impl Into<String>) -> ucan::Result<CaveatDocument> {
    caveat(CAVEAT_PATH_PREFIX, "path", prefix.into())
}

pub fn command_prefix_caveat(prefix: impl Into<String>) -> ucan::Result<CaveatDocument> {
    caveat(CAVEAT_COMMAND_PREFIX, "command", prefix.into())
}

pub fn timeout_ms_at_most_caveat(limit_ms: u64) -> ucan::Result<CaveatDocument> {
    caveat(CAVEAT_TIMEOUT_MS_AT_MOST, "timeout_ms", limit_ms.to_string())
}

pub fn max_bytes_at_most_caveat(limit: u64) -> ucan::Result<CaveatDocument> {
    caveat(CAVEAT_MAX_BYTES_AT_MOST, "max_bytes", limit.to_string())
}

pub fn network_host_caveat(host: impl Into<String>) -> ucan::Result<CaveatDocument> {
    caveat(CAVEAT_NETWORK_HOST, "host", host.into())
}

pub fn network_scheme_caveat(scheme: impl Into<String>) -> ucan::Result<CaveatDocument> {
    caveat(CAVEAT_NETWORK_SCHEME, "scheme", scheme.into())
}

pub fn provider_caveat(provider: impl Into<String>) -> ucan::Result<CaveatDocument> {
    caveat(CAVEAT_PROVIDER, "provider", provider.into())
}

pub fn model_prefix_caveat(prefix: impl Into<String>) -> ucan::Result<CaveatDocument> {
    caveat(CAVEAT_MODEL_PREFIX, "model", prefix.into())
}

fn caveat(caveat_type: &str, key: &str, payload: String) -> ucan::Result<CaveatDocument> {
    CaveatDocument::new(CLANKERS_CAVEAT_DOMAIN.to_owned(), caveat_type.to_owned(), key.to_owned(), payload.into_bytes())
}

fn is_supported_caveat(caveat_type: &str) -> bool {
    matches!(
        caveat_type,
        CAVEAT_PATH_PREFIX
            | CAVEAT_COMMAND_PREFIX
            | CAVEAT_TIMEOUT_MS_AT_MOST
            | CAVEAT_MAX_BYTES_AT_MOST
            | CAVEAT_NETWORK_HOST
            | CAVEAT_NETWORK_SCHEME
            | CAVEAT_PROVIDER
            | CAVEAT_MODEL_PREFIX
    )
}

fn evaluate_known_caveat(context: &EffectCaveatContext, caveat: &CaveatDocument) -> Result<(), CaveatHookError> {
    match caveat.caveat_type() {
        CAVEAT_PATH_PREFIX => string_starts_with(context.path.as_deref(), payload_text(caveat)?, "path"),
        CAVEAT_COMMAND_PREFIX => string_starts_with(context.command.as_deref(), payload_text(caveat)?, "command"),
        CAVEAT_TIMEOUT_MS_AT_MOST => numeric_at_most(context.timeout_ms, payload_u64(caveat)?, "timeout_ms"),
        CAVEAT_MAX_BYTES_AT_MOST => numeric_at_most(context.max_bytes, payload_u64(caveat)?, "max_bytes"),
        CAVEAT_NETWORK_HOST => string_equals(context.network_host.as_deref(), payload_text(caveat)?, "network_host"),
        CAVEAT_NETWORK_SCHEME => {
            string_equals(context.network_scheme.as_deref(), payload_text(caveat)?, "network_scheme")
        }
        CAVEAT_PROVIDER => string_equals(context.provider.as_deref(), payload_text(caveat)?, "provider"),
        CAVEAT_MODEL_PREFIX => string_starts_with(context.model.as_deref(), payload_text(caveat)?, "model"),
        other => Err(CaveatHookError::UnsupportedCaveat {
            caveat_type: other.to_owned(),
        }),
    }
}

fn string_starts_with(actual: Option<&str>, expected_prefix: &str, label: &'static str) -> Result<(), CaveatHookError> {
    let actual = actual.ok_or_else(|| CaveatHookError::UnsupportedCaveat {
        caveat_type: format!("missing-{label}"),
    })?;
    if actual.starts_with(expected_prefix) {
        Ok(())
    } else {
        Err(CaveatHookError::UnsupportedCaveat {
            caveat_type: format!("{label}-prefix-mismatch"),
        })
    }
}

fn string_equals(actual: Option<&str>, expected: &str, label: &'static str) -> Result<(), CaveatHookError> {
    let actual = actual.ok_or_else(|| CaveatHookError::UnsupportedCaveat {
        caveat_type: format!("missing-{label}"),
    })?;
    if actual == expected {
        Ok(())
    } else {
        Err(CaveatHookError::UnsupportedCaveat {
            caveat_type: format!("{label}-mismatch"),
        })
    }
}

fn numeric_at_most(actual: Option<u64>, limit: u64, label: &'static str) -> Result<(), CaveatHookError> {
    let actual = actual.ok_or_else(|| CaveatHookError::UnsupportedCaveat {
        caveat_type: format!("missing-{label}"),
    })?;
    if actual <= limit {
        Ok(())
    } else {
        Err(CaveatHookError::UnsupportedCaveat {
            caveat_type: format!("{label}-limit-exceeded"),
        })
    }
}

fn payload_text(caveat: &CaveatDocument) -> Result<&str, CaveatHookError> {
    std::str::from_utf8(caveat.payload()).map_err(|_| CaveatHookError::InvalidPayloadUtf8)
}

fn payload_u64(caveat: &CaveatDocument) -> Result<u64, CaveatHookError> {
    let payload = payload_text(caveat)?;
    payload.parse::<u64>().map_err(|_| CaveatHookError::InvalidInteger {
        payload: payload.to_owned(),
    })
}

#[cfg(test)]
mod tests {
    use ucan::CapabilityDocument;

    use super::*;

    const RESOURCE: &str = "clankers:file:/workspace/project/src/lib.rs";
    const ABILITY: &str = "file/read";

    fn request() -> AuthorizationRequest {
        AuthorizationRequest::new(RESOURCE, ABILITY).expect("authorization request")
    }

    fn evaluate(context: EffectCaveatContext, caveat: &CaveatDocument) -> CaveatDecision {
        ClankersCaveatPolicySet::new(context).evaluate(caveat, &request())
    }

    fn satisfied(decision: CaveatDecision) -> bool {
        matches!(decision, CaveatDecision::Satisfied)
    }

    #[test]
    fn path_prefix_hook_accepts_matching_path() {
        let caveat = path_prefix_caveat("/workspace/project").expect("caveat");
        let context = EffectCaveatContext::new().with_path("/workspace/project/src/lib.rs");

        assert!(satisfied(evaluate(context, &caveat)));
    }

    #[test]
    fn path_prefix_hook_rejects_mismatched_path() {
        let caveat = path_prefix_caveat("/workspace/project").expect("caveat");
        let context = EffectCaveatContext::new().with_path("/tmp/other.rs");

        assert!(!satisfied(evaluate(context, &caveat)));
    }

    #[test]
    fn command_prefix_hook_accepts_matching_command() {
        let caveat = command_prefix_caveat("cargo test").expect("caveat");
        let context = EffectCaveatContext::new().with_command("cargo test -p clankers-ucan");

        assert!(satisfied(evaluate(context, &caveat)));
    }

    #[test]
    fn command_prefix_hook_rejects_mismatched_command() {
        let caveat = command_prefix_caveat("cargo test").expect("caveat");
        let context = EffectCaveatContext::new().with_command("rm -rf target");

        assert!(!satisfied(evaluate(context, &caveat)));
    }

    #[test]
    fn timeout_and_max_bytes_hooks_enforce_upper_bounds() {
        let timeout = timeout_ms_at_most_caveat(1_000).expect("timeout caveat");
        let max_bytes = max_bytes_at_most_caveat(4_096).expect("max bytes caveat");
        let context = EffectCaveatContext::new().with_timeout_ms(999).with_max_bytes(4_096);

        assert!(satisfied(evaluate(context.clone(), &timeout)));
        assert!(satisfied(evaluate(context, &max_bytes)));
    }

    #[test]
    fn timeout_and_max_bytes_hooks_reject_excess() {
        let timeout = timeout_ms_at_most_caveat(1_000).expect("timeout caveat");
        let max_bytes = max_bytes_at_most_caveat(4_096).expect("max bytes caveat");

        assert!(!satisfied(evaluate(EffectCaveatContext::new().with_timeout_ms(1_001), &timeout)));
        assert!(!satisfied(evaluate(EffectCaveatContext::new().with_max_bytes(4_097), &max_bytes)));
    }

    #[test]
    fn network_and_provider_hooks_enforce_exact_scopes() {
        let host = network_host_caveat("api.openai.com").expect("host caveat");
        let scheme = network_scheme_caveat("https").expect("scheme caveat");
        let provider = provider_caveat("openai").expect("provider caveat");
        let model = model_prefix_caveat("gpt-").expect("model caveat");
        let context = EffectCaveatContext::new()
            .with_network_host("api.openai.com")
            .with_network_scheme("https")
            .with_provider("openai")
            .with_model("gpt-4o");

        assert!(satisfied(evaluate(context.clone(), &host)));
        assert!(satisfied(evaluate(context.clone(), &scheme)));
        assert!(satisfied(evaluate(context.clone(), &provider)));
        assert!(satisfied(evaluate(context, &model)));
    }

    #[test]
    fn network_and_provider_hooks_reject_mismatches() {
        let host = network_host_caveat("api.openai.com").expect("host caveat");
        let scheme = network_scheme_caveat("https").expect("scheme caveat");
        let provider = provider_caveat("openai").expect("provider caveat");
        let model = model_prefix_caveat("gpt-").expect("model caveat");
        let context = EffectCaveatContext::new()
            .with_network_host("example.com")
            .with_network_scheme("http")
            .with_provider("anthropic")
            .with_model("claude-sonnet");

        assert!(!satisfied(evaluate(context.clone(), &host)));
        assert!(!satisfied(evaluate(context.clone(), &scheme)));
        assert!(!satisfied(evaluate(context.clone(), &provider)));
        assert!(!satisfied(evaluate(context, &model)));
    }

    #[test]
    fn policy_set_returns_none_for_unknown_caveats() {
        let caveat = CaveatDocument::new(
            CLANKERS_CAVEAT_DOMAIN.to_owned(),
            "unknown".to_owned(),
            "key".to_owned(),
            b"value".to_vec(),
        )
        .expect("unknown caveat");
        let identifier = CaveatIdentifier::from(&caveat);
        let policies = ClankersCaveatPolicySet::new(EffectCaveatContext::new());

        assert!(policies.policy_for(&identifier).is_none());
    }

    #[test]
    fn caveat_documents_attach_to_public_capabilities() {
        let caveat = path_prefix_caveat("/workspace/project").expect("caveat");
        let document = CapabilityDocument::with_caveats(RESOURCE.to_owned(), ABILITY.to_owned(), vec![caveat])
            .expect("capability document");

        assert_eq!(document.caveats.len(), 1);
    }
}
