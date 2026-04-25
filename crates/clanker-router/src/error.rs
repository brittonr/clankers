//! Error types for the router crate

use std::fmt;

/// Router error type
#[derive(Debug)]
pub enum Error {
    /// Provider returned an error during completion
    Provider {
        message: String,
        /// HTTP status code, if the error originated from an HTTP response
        #[doc(hidden)]
        status: Option<u16>,
    },
    /// Authentication/credential error
    Auth { message: String },
    /// Streaming error
    Streaming { message: String },
    /// No provider available for the requested model
    NoProvider { model: String },
    /// Configuration error
    Config { message: String },
    /// I/O error (file operations, etc.)
    Io(std::io::Error),
    /// JSON serialization/deserialization error
    Json(serde_json::Error),
    /// HTTP request error
    Http(reqwest::Error),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Provider { message, .. } => write!(f, "provider error: {}", message),
            Error::Auth { message } => write!(f, "auth error: {}", message),
            Error::Streaming { message } => write!(f, "streaming error: {}", message),
            Error::NoProvider { model } => write!(f, "no provider available for model: {}", model),
            Error::Config { message } => write!(f, "config error: {}", message),
            Error::Io(e) => write!(f, "I/O error: {}", e),
            Error::Json(e) => write!(f, "JSON error: {}", e),
            Error::Http(e) => write!(f, "HTTP error: {}", e),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Error::Io(e) => Some(e),
            Error::Json(e) => Some(e),
            Error::Http(e) => Some(e),
            _ => None,
        }
    }
}

impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Self {
        Error::Io(e)
    }
}

impl From<serde_json::Error> for Error {
    fn from(e: serde_json::Error) -> Self {
        Error::Json(e)
    }
}

impl From<reqwest::Error> for Error {
    fn from(e: reqwest::Error) -> Self {
        Error::Http(e)
    }
}

impl Error {
    /// Create a provider error with an associated HTTP status code.
    pub fn provider_with_status(status: u16, message: impl Into<String>) -> Self {
        Error::Provider {
            message: message.into(),
            status: Some(status),
        }
    }

    /// Whether this error is likely transient and the request could succeed
    /// against a different provider or after a delay.
    pub fn is_retryable(&self) -> bool {
        match self {
            Error::Provider { message, status } => {
                // Prefer the structured status code field
                if let Some(code) = status {
                    return crate::retry::is_retryable_status(*code);
                }
                // Fall back to parsing the message string
                if let Some(code) = extract_status(message) {
                    return crate::retry::is_retryable_status(code);
                }
                crate::retry::is_retryable_error(message)
            }
            Error::Http(e) => {
                if e.is_timeout() || e.is_connect() {
                    return true;
                }
                if let Some(status) = e.status() {
                    return crate::retry::is_retryable_status(status.as_u16());
                }
                false
            }
            Error::Streaming { message } => {
                // Connection-level stream failures are retryable
                crate::retry::is_retryable_error(message)
            }
            // Auth, config, no-provider, JSON, I/O errors are not retryable
            _ => false,
        }
    }

    /// Try to extract an HTTP status code from this error.
    pub fn status_code(&self) -> Option<u16> {
        match self {
            Error::Provider { status, message } => status.or_else(|| extract_status(message)),
            Error::Http(e) => e.status().map(|s| s.as_u16()),
            _ => None,
        }
    }
}

/// Extract an HTTP status code from an error message string.
fn extract_status(msg: &str) -> Option<u16> {
    // Match patterns like "HTTP 429:", "API error 429:", "error 429:"
    for prefix in &["HTTP ", "API error ", "error "] {
        if let Some(pos) = msg.find(prefix) {
            let start = pos + prefix.len();
            let code_str: String = msg[start..].chars().take_while(|c| c.is_ascii_digit()).collect();
            if let Ok(code) = code_str.parse::<u16>()
                && (400..600).contains(&code)
            {
                return Some(code);
            }
        }
    }
    None
}

pub type Result<T, E = Error> = std::result::Result<T, E>;
