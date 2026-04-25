//! Structured API error classification with recovery hints.

#![allow(unexpected_cfgs)]
#![cfg_attr(dylint_lib = "tigerstyle", allow(compound_assertion, ignored_result, no_unwrap, no_panic, no_todo, unjustified_no_todo_allow, no_recursion, unchecked_narrowing, unchecked_division, unbounded_loop, catch_all_on_enum, explicit_defaults, unbounded_channel, unbounded_collection_growth, assertion_density, raw_arithmetic_overflow, sentinel_fallback, acronym_style, bool_naming, negated_predicate, numeric_units, float_for_currency, function_length, nested_conditionals, platform_dependent_cast, usize_in_public_api, too_many_parameters, compound_condition, unjustified_allow, ambiguous_params, ambient_clock, verified_purity, contradictory_time, multi_lock_ordering, reason = "full workspace tigerstyle audit gate: legacy debt documented locally while cleanup proceeds incrementally"))]

const STATUS_BAD_REQUEST: u16 = 400;
const STATUS_AUTH_UNAUTHORIZED: u16 = 401;
const STATUS_PAYMENT_REQUIRED: u16 = 402;
const STATUS_AUTH_FORBIDDEN: u16 = 403;
const STATUS_NOT_FOUND: u16 = 404;
const STATUS_PAYLOAD_TOO_LARGE: u16 = 413;
const STATUS_RATE_LIMITED: u16 = 429;
const STATUS_SERVER_ERROR: u16 = 500;
const STATUS_BAD_GATEWAY: u16 = 502;
const STATUS_SERVICE_UNAVAILABLE: u16 = 503;
const STATUS_CLOUDFLARE_OVERLOADED: u16 = 529;

const PROVIDER_UNKNOWN: &str = "unknown";
const PROVIDER_ANTHROPIC: &str = "anthropic";
const PROVIDER_OPENAI: &str = "openai";
const PROVIDER_OPENROUTER: &str = "openrouter";
const PROVIDER_OPENAI_CODEX: &str = "openai-codex";
const PROVIDER_VLLM: &str = "vllm";
const PROVIDER_OLLAMA: &str = "ollama";
const PROVIDER_LLAMA_CPP: &str = "llama.cpp";

const TRANSIENT_QUOTA_SIGNALS: [&str; 5] = ["try again", "retry", "resets at", "reset at", "per minute"];

const BILLING_PATTERNS: [&str; 8] = [
    "insufficient_quota",
    "insufficient quota",
    "insufficient credits",
    "out of credits",
    "billing",
    "payment required",
    "credit balance is too low",
    "quota exceeded",
];

const RATE_LIMIT_PATTERNS: [&str; 9] = [
    "rate limit",
    "too many requests",
    "try again later",
    "requests per min",
    "tokens per min",
    "please slow down",
    "retry after",
    "resets at",
    "429",
];

const CONTEXT_OVERFLOW_PATTERNS: [&str; 11] = [
    "context length",
    "maximum context length",
    "prompt is too long",
    "too many tokens",
    "token limit exceeded",
    "context window",
    "requested tokens exceed",
    "input is too long",
    "413 payload too large",
    "context overflow",
    "reduce the length",
];

const MODEL_NOT_FOUND_PATTERNS: [&str; 8] = [
    "model_not_found",
    "model not found",
    "does not exist",
    "unknown model",
    "unsupported model",
    "invalid model",
    "no such model",
    "unrecognized model",
];

const TIMEOUT_PATTERNS: [&str; 4] = ["timed out", "timeout", "deadline exceeded", "read timeout"];

const AUTH_PATTERNS: [&str; 8] = [
    "invalid api key",
    "incorrect api key",
    "unauthorized",
    "forbidden",
    "authentication",
    "invalid token",
    "expired token",
    "access denied",
];

const OVERLOADED_PATTERNS: [&str; 6] = [
    "overloaded",
    "over capacity",
    "server overloaded",
    "temporarily unavailable",
    "engine is overloaded",
    "529",
];

const FORMAT_ERROR_PATTERNS: [&str; 7] = [
    "invalid_request_error",
    "invalid request",
    "malformed",
    "schema",
    "json",
    "tool schema",
    "thinking block",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FailoverReason {
    Auth,
    AuthPermanent,
    Billing,
    RateLimit,
    Overloaded,
    ServerError,
    Timeout,
    ContextOverflow,
    ModelNotFound,
    FormatError,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClassifiedError {
    pub reason: FailoverReason,
    pub status_code: Option<u16>,
    pub provider: String,
    pub message: String,
    pub retryable: bool,
    pub should_compress: bool,
    pub should_rotate_credential: bool,
    pub should_fallback: bool,
}

pub const fn recovery_hints(reason: FailoverReason) -> (bool, bool, bool, bool) {
    match reason {
        FailoverReason::Auth => (false, false, true, true),
        FailoverReason::AuthPermanent => (false, false, false, true),
        FailoverReason::Billing => (false, false, true, true),
        FailoverReason::RateLimit => (true, false, true, true),
        FailoverReason::Overloaded => (true, false, false, true),
        FailoverReason::ServerError => (true, false, false, true),
        FailoverReason::Timeout => (true, false, false, true),
        FailoverReason::ContextOverflow => (true, true, false, false),
        FailoverReason::ModelNotFound => (false, false, false, true),
        FailoverReason::FormatError => (false, false, false, false),
        FailoverReason::Unknown => (true, false, false, false),
    }
}

pub fn classify_api_error(status_code: Option<u16>, body: &str, provider: &str) -> ClassifiedError {
    build_classified_error(
        status_code,
        body,
        provider,
        classify_reason(status_code, &body.trim().to_ascii_lowercase(), &provider.trim().to_ascii_lowercase()),
    )
}

pub fn classify_transport_error(message: &str, provider: &str) -> ClassifiedError {
    let normalized_message = message.trim().to_ascii_lowercase();
    let reason = if contains_any(&normalized_message, timeout_patterns(provider.trim().to_ascii_lowercase().as_str()))
        || contains_any(&normalized_message, &TIMEOUT_PATTERNS)
    {
        FailoverReason::Timeout
    } else {
        FailoverReason::Unknown
    };
    build_classified_error(None, message, provider, reason)
}

fn build_classified_error(
    status_code: Option<u16>,
    message: &str,
    provider: &str,
    reason: FailoverReason,
) -> ClassifiedError {
    let normalized_provider = provider.trim().to_ascii_lowercase();
    let (retryable, should_compress, should_rotate_credential, should_fallback) = recovery_hints(reason);

    ClassifiedError {
        reason,
        status_code,
        provider: normalized_provider,
        message: message.trim().to_string(),
        retryable,
        should_compress,
        should_rotate_credential,
        should_fallback,
    }
}

fn classify_reason(status_code: Option<u16>, body: &str, provider: &str) -> FailoverReason {
    let _provider_is_unknown = provider == PROVIDER_UNKNOWN;
    if let Some(reason) = classify_from_status(status_code) {
        return reason;
    }

    classify_from_body(body, provider).unwrap_or(FailoverReason::Unknown)
}

fn classify_from_status(status_code: Option<u16>) -> Option<FailoverReason> {
    match status_code {
        Some(STATUS_BAD_REQUEST) => Some(FailoverReason::FormatError),
        Some(STATUS_AUTH_UNAUTHORIZED | STATUS_AUTH_FORBIDDEN) => Some(FailoverReason::Auth),
        Some(STATUS_PAYMENT_REQUIRED) => Some(FailoverReason::Billing),
        Some(STATUS_NOT_FOUND) => Some(FailoverReason::ModelNotFound),
        Some(STATUS_PAYLOAD_TOO_LARGE) => Some(FailoverReason::ContextOverflow),
        Some(STATUS_RATE_LIMITED) => Some(FailoverReason::RateLimit),
        Some(STATUS_CLOUDFLARE_OVERLOADED | STATUS_SERVICE_UNAVAILABLE) => Some(FailoverReason::Overloaded),
        Some(STATUS_SERVER_ERROR | STATUS_BAD_GATEWAY) => Some(FailoverReason::ServerError),
        Some(_) | None => None,
    }
}

fn classify_from_body(body: &str, provider: &str) -> Option<FailoverReason> {
    if body.is_empty() {
        return None;
    }

    if contains_any(body, timeout_patterns(provider)) {
        return Some(FailoverReason::Timeout);
    }

    if contains_any(body, auth_permanent_patterns(provider)) {
        return Some(FailoverReason::AuthPermanent);
    }

    if contains_any(body, auth_patterns(provider)) {
        return Some(FailoverReason::Auth);
    }

    if contains_any(body, context_overflow_patterns(provider)) {
        return Some(FailoverReason::ContextOverflow);
    }

    if contains_any(body, model_not_found_patterns(provider)) {
        return Some(FailoverReason::ModelNotFound);
    }

    if contains_any(body, overloaded_patterns(provider)) {
        return Some(FailoverReason::Overloaded);
    }

    if contains_any(body, rate_limit_patterns(provider)) {
        return Some(FailoverReason::RateLimit);
    }

    if contains_any(body, billing_patterns(provider)) {
        return Some(disambiguate_quota_reason(body));
    }

    if contains_any(body, format_error_patterns(provider)) {
        return Some(FailoverReason::FormatError);
    }

    if body.contains("server error") || body.contains("internal error") || body.contains("bad gateway") {
        return Some(FailoverReason::ServerError);
    }

    None
}

fn disambiguate_quota_reason(body: &str) -> FailoverReason {
    if contains_any(body, &TRANSIENT_QUOTA_SIGNALS) {
        return FailoverReason::RateLimit;
    }

    FailoverReason::Billing
}

fn contains_any(body: &str, patterns: &[&str]) -> bool {
    patterns.iter().any(|pattern| body.contains(pattern))
}

fn timeout_patterns(provider: &str) -> &'static [&'static str] {
    match provider {
        PROVIDER_OPENROUTER => &["timed out", "timeout", "request timeout", "upstream timeout"],
        _ => &["timed out", "timeout", "deadline exceeded", "read timeout"],
    }
}

fn auth_permanent_patterns(provider: &str) -> &'static [&'static str] {
    match provider {
        PROVIDER_OPENAI | PROVIDER_OPENAI_CODEX => &["organization disabled", "account deactivated"],
        _ => &["account disabled", "account suspended"],
    }
}

fn auth_patterns(provider: &str) -> &'static [&'static str] {
    match provider {
        PROVIDER_ANTHROPIC => &["invalid x-api-key", "authentication_error", "permission denied"],
        PROVIDER_OPENAI | PROVIDER_OPENAI_CODEX => &["invalid_api_key", "incorrect api key", "organization"],
        _ => &AUTH_PATTERNS,
    }
}

fn billing_patterns(provider: &str) -> &'static [&'static str] {
    match provider {
        PROVIDER_OPENAI | PROVIDER_OPENAI_CODEX => {
            &["insufficient_quota", "billing_hard_limit_reached", "quota exceeded"]
        }
        PROVIDER_OPENROUTER => &["credits", "insufficient credits", "payment required", "quota exceeded"],
        _ => &BILLING_PATTERNS,
    }
}

fn rate_limit_patterns(provider: &str) -> &'static [&'static str] {
    match provider {
        PROVIDER_ANTHROPIC => &[
            "rate limit",
            "too many requests",
            "retry after",
            "request was throttled",
        ],
        PROVIDER_OPENROUTER => &[
            "rate limited",
            "too many requests",
            "retry after",
            "upstream rate limit",
        ],
        _ => &RATE_LIMIT_PATTERNS,
    }
}

fn context_overflow_patterns(provider: &str) -> &'static [&'static str] {
    match provider {
        PROVIDER_VLLM => &[
            "maximum context length",
            "context length exceeded",
            "prompt is too long",
        ],
        PROVIDER_OLLAMA => &["input length exceeds context", "context window", "too many tokens"],
        PROVIDER_LLAMA_CPP => &[
            "requested tokens exceed context window",
            "context overflow",
            "too many tokens",
        ],
        _ => &CONTEXT_OVERFLOW_PATTERNS,
    }
}

fn model_not_found_patterns(provider: &str) -> &'static [&'static str] {
    match provider {
        PROVIDER_OPENROUTER => &[
            "model not found",
            "no endpoints found",
            "provider routing failed",
            "unsupported model",
        ],
        _ => &MODEL_NOT_FOUND_PATTERNS,
    }
}

fn overloaded_patterns(provider: &str) -> &'static [&'static str] {
    match provider {
        PROVIDER_OPENROUTER => &[
            "upstream provider error",
            "provider is overloaded",
            "temporarily unavailable",
            "overloaded",
        ],
        _ => &OVERLOADED_PATTERNS,
    }
}

fn format_error_patterns(provider: &str) -> &'static [&'static str] {
    match provider {
        PROVIDER_ANTHROPIC => &["thinking block", "invalid request", "malformed", "tool schema"],
        _ => &FORMAT_ERROR_PATTERNS,
    }
}

#[cfg(test)]
mod tests {
    use super::ClassifiedError;
    use super::FailoverReason;
    use super::classify_api_error;
    use super::classify_transport_error;
    use super::recovery_hints;

    fn assert_reason(
        classified: ClassifiedError,
        expected_reason: FailoverReason,
        expected_retryable: bool,
        expected_compress: bool,
        expected_rotate: bool,
        expected_fallback: bool,
    ) {
        assert_eq!(classified.reason, expected_reason);
        assert_eq!(classified.retryable, expected_retryable);
        assert_eq!(classified.should_compress, expected_compress);
        assert_eq!(classified.should_rotate_credential, expected_rotate);
        assert_eq!(classified.should_fallback, expected_fallback);
    }

    #[test]
    fn recovery_hints_match_taxonomy() {
        let cases = [
            (FailoverReason::Auth, (false, false, true, true)),
            (FailoverReason::AuthPermanent, (false, false, false, true)),
            (FailoverReason::Billing, (false, false, true, true)),
            (FailoverReason::RateLimit, (true, false, true, true)),
            (FailoverReason::Overloaded, (true, false, false, true)),
            (FailoverReason::ServerError, (true, false, false, true)),
            (FailoverReason::Timeout, (true, false, false, true)),
            (FailoverReason::ContextOverflow, (true, true, false, false)),
            (FailoverReason::ModelNotFound, (false, false, false, true)),
            (FailoverReason::FormatError, (false, false, false, false)),
            (FailoverReason::Unknown, (true, false, false, false)),
        ];

        for (reason, expected) in cases {
            assert_eq!(recovery_hints(reason), expected, "wrong hints for {reason:?}");
        }
    }

    #[test]
    fn status_code_classification_has_priority() {
        assert_reason(
            classify_api_error(Some(429), "invalid api key", "openai"),
            FailoverReason::RateLimit,
            true,
            false,
            true,
            true,
        );
        assert_reason(
            classify_api_error(Some(402), "anything", "openrouter"),
            FailoverReason::Billing,
            false,
            false,
            true,
            true,
        );
        assert_reason(
            classify_api_error(Some(401), "overloaded", "anthropic"),
            FailoverReason::Auth,
            false,
            false,
            true,
            true,
        );
        assert_reason(
            classify_api_error(Some(404), "unknown model", "openai"),
            FailoverReason::ModelNotFound,
            false,
            false,
            false,
            true,
        );
        assert_reason(
            classify_api_error(Some(500), "bad request", "openai"),
            FailoverReason::ServerError,
            true,
            false,
            false,
            true,
        );
        assert_reason(
            classify_api_error(Some(502), "bad request", "openai"),
            FailoverReason::ServerError,
            true,
            false,
            false,
            true,
        );
        assert_reason(
            classify_api_error(Some(503), "bad request", "openai"),
            FailoverReason::Overloaded,
            true,
            false,
            false,
            true,
        );
        assert_reason(
            classify_api_error(Some(529), "bad request", "anthropic"),
            FailoverReason::Overloaded,
            true,
            false,
            false,
            true,
        );
    }

    #[test]
    fn body_pattern_classification_covers_shared_and_provider_specific_patterns() {
        assert_eq!(classify_api_error(None, "invalid_api_key: no key", "openai").reason, FailoverReason::Auth);
        assert_eq!(classify_api_error(None, "insufficient_quota on account", "openai").reason, FailoverReason::Billing);
        assert_eq!(
            classify_api_error(None, "request exceeded maximum context length", "vllm").reason,
            FailoverReason::ContextOverflow
        );
        assert_eq!(
            classify_api_error(None, "provider routing failed: no endpoints found for model", "openrouter").reason,
            FailoverReason::ModelNotFound
        );
        assert_eq!(
            classify_api_error(None, "server is overloaded, please retry", "anthropic").reason,
            FailoverReason::Overloaded
        );
        assert_eq!(
            classify_api_error(None, "thinking block signature missing", "anthropic").reason,
            FailoverReason::FormatError
        );
        assert_eq!(
            classify_api_error(None, "upstream timeout contacting provider", "openrouter").reason,
            FailoverReason::Timeout
        );
    }

    #[test]
    fn quota_disambiguation_prefers_rate_limit_when_transient_signal_exists() {
        assert_eq!(
            classify_api_error(None, "quota exceeded try again in 5 minutes", "openai").reason,
            FailoverReason::RateLimit
        );
        assert_eq!(classify_api_error(None, "quota exceeded", "openai").reason, FailoverReason::Billing);
    }

    #[test]
    fn unknown_errors_default_to_retryable_unknown() {
        let classified = classify_api_error(None, "completely novel failure", "custom");
        assert_reason(classified, FailoverReason::Unknown, true, false, false, false);
    }

    #[test]
    fn provider_specific_body_patterns_cover_remaining_contract_cases() {
        assert_eq!(
            classify_api_error(None, "thinking block signature missing", "anthropic").reason,
            FailoverReason::FormatError
        );
        assert_eq!(
            classify_api_error(None, "upstream provider error: provider is overloaded", "openrouter").reason,
            FailoverReason::Overloaded
        );
        assert_eq!(
            classify_api_error(None, "provider routing failed: no endpoints found", "openrouter").reason,
            FailoverReason::ModelNotFound
        );
        assert_eq!(
            classify_api_error(None, "input is too long for the context window", "ollama").reason,
            FailoverReason::ContextOverflow
        );
    }

    #[test]
    fn transport_timeout_and_status_precedence_are_classified_correctly() {
        assert_eq!(
            classify_transport_error("request timed out while contacting upstream", "openrouter").reason,
            FailoverReason::Timeout
        );
        assert_eq!(classify_api_error(Some(429), "invalid api key", "openai").reason, FailoverReason::RateLimit);
        assert_eq!(classify_api_error(Some(429), "invalid api key", "unknown").reason, FailoverReason::RateLimit);
    }
}
