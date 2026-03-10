//! Provider-specific error types.

use std::fmt;

/// Provider error type.
#[derive(Debug)]
pub struct ProviderError {
    pub message: String,
    pub kind: ProviderErrorKind,
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
        }
    }
}

impl From<serde_json::Error> for ProviderError {
    fn from(e: serde_json::Error) -> Self {
        Self {
            message: e.to_string(),
            kind: ProviderErrorKind::Json(e),
        }
    }
}

impl From<clankers_router::Error> for ProviderError {
    fn from(e: clankers_router::Error) -> Self {
        Self {
            message: e.to_string(),
            kind: ProviderErrorKind::Api,
        }
    }
}



/// Convenience constructors.
pub fn provider_err(msg: impl Into<String>) -> ProviderError {
    ProviderError {
        message: msg.into(),
        kind: ProviderErrorKind::Api,
    }
}

pub fn auth_err(msg: impl Into<String>) -> ProviderError {
    ProviderError {
        message: msg.into(),
        kind: ProviderErrorKind::Auth,
    }
}

pub fn streaming_err(msg: impl Into<String>) -> ProviderError {
    ProviderError {
        message: msg.into(),
        kind: ProviderErrorKind::Streaming,
    }
}

pub type Result<T> = std::result::Result<T, ProviderError>;
