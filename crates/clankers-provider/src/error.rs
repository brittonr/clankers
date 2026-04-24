//! Provider-specific error types.

use std::fmt;

use crate::error_classifier::ClassifiedError;
use crate::error_classifier::FailoverReason;
use crate::error_classifier::classify_api_error;
use crate::error_classifier::classify_transport_error;

/// Provider error type.
#[derive(Debug)]
pub struct ProviderError {
    pub message: String,
    pub kind: ProviderErrorKind,
    /// HTTP status code, if the error originated from an HTTP response.
    /// Preserved through error conversions so retry/fallback logic can
    /// inspect it without parsing the message string.
    pub status: Option<u16>,
    pub classified: ClassifiedError,
}

#[derive(Debug)]
pub enum ProviderErrorKind {
    /// API/network error
    Api,
    /// Authentication error (missing/expired credentials)
    Auth,
    /// Streaming parse error
    Streaming,
    /// IO error
    Io(std::io::Error),
    /// JSON serialization error
    Json(serde_json::Error),
}

impl fmt::Display for ProviderError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.kind {
            ProviderErrorKind::Api => write!(f, "provider error: {}", self.message),
            ProviderErrorKind::Auth => write!(f, "provider auth error: {}", self.message),
            ProviderErrorKind::Streaming => write!(f, "provider streaming error: {}", self.message),
            ProviderErrorKind::Io(e) => write!(f, "provider I/O error: {}: {}", self.message, e),
            ProviderErrorKind::Json(e) => write!(f, "provider JSON error: {}: {}", self.message, e),
        }
    }
}

impl std::error::Error for ProviderError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match &self.kind {
            ProviderErrorKind::Io(e) => Some(e),
            ProviderErrorKind::Json(e) => Some(e),
            _ => None,
        }
    }
}

impl From<std::io::Error> for ProviderError {
    fn from(e: std::io::Error) -> Self {
        let message = e.to_string();
        let classified = classify_transport_error(&message, "unknown");
        Self {
            message,
            kind: ProviderErrorKind::Io(e),
            status: None,
            classified,
        }
    }
}

impl From<serde_json::Error> for ProviderError {
    fn from(e: serde_json::Error) -> Self {
        let message = e.to_string();
        let classified = classify_transport_error(&message, "unknown");
        Self {
            message,
            kind: ProviderErrorKind::Json(e),
            status: None,
            classified,
        }
    }
}

impl From<clanker_router::Error> for ProviderError {
    fn from(e: clanker_router::Error) -> Self {
        let status = e.status_code();
        let kind = match &e {
            clanker_router::Error::Auth { .. } => ProviderErrorKind::Auth,
            clanker_router::Error::Streaming { .. } => ProviderErrorKind::Streaming,
            clanker_router::Error::Io(_) => {
                // Convert the io error — extract from the router error
                ProviderErrorKind::Api
            }
            clanker_router::Error::Json(_) => ProviderErrorKind::Api,
            _ => ProviderErrorKind::Api,
        };
        let message = e.to_string();
        let provider = "router";
        let classified = match status {
            Some(code) => classify_api_error(Some(code), &message, provider),
            None => classify_transport_error(&message, provider),
        };
        Self {
            message,
            kind,
            status,
            classified,
        }
    }
}

impl ProviderError {
    /// Whether this error is likely transient and the request could succeed
    /// against a different provider or after a delay.
    pub fn is_retryable(&self) -> bool {
        self.classified.retryable
    }

    pub fn should_compress(&self) -> bool {
        self.classified.should_compress
    }

    pub fn should_fallback(&self) -> bool {
        self.classified.should_fallback
    }

    pub fn should_rotate_credential(&self) -> bool {
        self.classified.should_rotate_credential
    }

    pub fn failover_reason(&self) -> FailoverReason {
        self.classified.reason
    }

    pub fn classified(&self) -> &ClassifiedError {
        &self.classified
    }

    /// Get the HTTP status code, if available.
    pub fn status_code(&self) -> Option<u16> {
        self.status
    }
}

/// Convenience constructors.
pub fn provider_err(msg: impl Into<String>) -> ProviderError {
    let message = msg.into();
    let classified = classify_api_error(None, &message, "unknown");
    ProviderError {
        message,
        kind: ProviderErrorKind::Api,
        status: None,
        classified,
    }
}

pub fn provider_err_with_status(status: u16, msg: impl Into<String>) -> ProviderError {
    provider_err_with_status_for_provider(status, msg, "unknown")
}

pub fn provider_err_with_status_for_provider(
    status: u16,
    msg: impl Into<String>,
    provider: &str,
) -> ProviderError {
    let message = msg.into();
    let classified = classify_api_error(Some(status), &message, provider);
    ProviderError {
        message,
        kind: ProviderErrorKind::Api,
        status: Some(status),
        classified,
    }
}

pub fn auth_err(msg: impl Into<String>) -> ProviderError {
    let message = msg.into();
    let mut classified = classify_transport_error(&message, "unknown");
    classified.reason = FailoverReason::Auth;
    classified.retryable = false;
    classified.should_compress = false;
    classified.should_rotate_credential = true;
    classified.should_fallback = true;
    ProviderError {
        message,
        kind: ProviderErrorKind::Auth,
        status: None,
        classified,
    }
}

pub fn streaming_err(msg: impl Into<String>) -> ProviderError {
    let message = msg.into();
    let classified = classify_transport_error(&message, "unknown");
    ProviderError {
        message,
        kind: ProviderErrorKind::Streaming,
        status: None,
        classified,
    }
}

pub type Result<T> = std::result::Result<T, ProviderError>;

#[cfg(test)]
mod tests {
    use super::ProviderError;
    use super::auth_err;
    use super::provider_err;
    use super::provider_err_with_status;

    #[test]
    fn provider_error_retains_full_classified_payload() {
        let error = provider_err_with_status(429, "quota exceeded try again in 5 minutes");
        assert_eq!(error.classified.reason, crate::FailoverReason::RateLimit);
        assert!(error.is_retryable());
        assert!(error.should_rotate_credential());
        assert!(error.should_fallback());
        assert_eq!(error.classified.message, "quota exceeded try again in 5 minutes");
    }

    #[test]
    fn auth_error_exposes_auth_classification() {
        let error = auth_err("invalid api key");
        assert_eq!(error.failover_reason(), crate::FailoverReason::Auth);
        assert!(!error.is_retryable());
        assert!(error.should_rotate_credential());
    }

    #[test]
    fn provider_err_classifies_auth_like_messages_without_status() {
        let error = provider_err("Invalid API key");
        assert_eq!(error.failover_reason(), crate::FailoverReason::Auth);
        assert!(!error.is_retryable());
        assert!(error.should_rotate_credential());
        assert!(error.should_fallback());
    }

    #[test]
    fn provider_err_keeps_unknown_messages_retryable() {
        let error = provider_err("completely novel failure");
        assert_eq!(error.failover_reason(), crate::FailoverReason::Unknown);
        assert!(error.is_retryable());
        assert!(!error.should_rotate_credential());
        assert!(!error.should_fallback());
    }

    #[test]
    fn auth_permanent_payload_can_be_observed() {
        let mut error = ProviderError::from(std::io::Error::new(
            std::io::ErrorKind::PermissionDenied,
            "account disabled",
        ));
        let mut classified = crate::classify_transport_error("account disabled", "openai");
        classified.reason = crate::FailoverReason::AuthPermanent;
        classified.retryable = false;
        classified.should_compress = false;
        classified.should_rotate_credential = false;
        classified.should_fallback = true;
        error.classified = classified;
        assert_eq!(error.failover_reason(), crate::FailoverReason::AuthPermanent);
        assert!(!error.is_retryable());
        assert!(error.should_fallback());
    }
}
