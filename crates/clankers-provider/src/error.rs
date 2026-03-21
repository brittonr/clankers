//! Provider-specific error types.

use std::fmt;

/// Provider error type.
#[derive(Debug)]
pub struct ProviderError {
    pub message: String,
    pub kind: ProviderErrorKind,
    /// HTTP status code, if the error originated from an HTTP response.
    /// Preserved through error conversions so retry/fallback logic can
    /// inspect it without parsing the message string.
    pub status: Option<u16>,
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
        Self {
            message: e.to_string(),
            kind: ProviderErrorKind::Io(e),
            status: None,
        }
    }
}

impl From<serde_json::Error> for ProviderError {
    fn from(e: serde_json::Error) -> Self {
        Self {
            message: e.to_string(),
            kind: ProviderErrorKind::Json(e),
            status: None,
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
        Self {
            message: e.to_string(),
            kind,
            status,
        }
    }
}

impl ProviderError {
    /// Whether this error is likely transient and the request could succeed
    /// against a different provider or after a delay.
    pub fn is_retryable(&self) -> bool {
        // Prefer the structured status code
        if let Some(code) = self.status {
            return clanker_router::retry::is_retryable_status(code);
        }
        // Fall back to message parsing
        match &self.kind {
            ProviderErrorKind::Api => clanker_router::retry::is_retryable_error(&self.message),
            ProviderErrorKind::Streaming => clanker_router::retry::is_retryable_error(&self.message),
            ProviderErrorKind::Auth | ProviderErrorKind::Io(_) | ProviderErrorKind::Json(_) => false,
        }
    }

    /// Get the HTTP status code, if available.
    pub fn status_code(&self) -> Option<u16> {
        self.status
    }
}

/// Convenience constructors.
pub fn provider_err(msg: impl Into<String>) -> ProviderError {
    ProviderError {
        message: msg.into(),
        kind: ProviderErrorKind::Api,
        status: None,
    }
}

pub fn provider_err_with_status(status: u16, msg: impl Into<String>) -> ProviderError {
    ProviderError {
        message: msg.into(),
        kind: ProviderErrorKind::Api,
        status: Some(status),
    }
}

pub fn auth_err(msg: impl Into<String>) -> ProviderError {
    ProviderError {
        message: msg.into(),
        kind: ProviderErrorKind::Auth,
        status: None,
    }
}

pub fn streaming_err(msg: impl Into<String>) -> ProviderError {
    ProviderError {
        message: msg.into(),
        kind: ProviderErrorKind::Streaming,
        status: None,
    }
}

pub type Result<T> = std::result::Result<T, ProviderError>;
