//! Agent-specific error types

use std::fmt;

/// Errors produced by agent operations (turn loop, compaction, etc.)
#[derive(Debug)]
pub enum AgentError {
    /// Operation was cancelled via `CancellationToken`
    Cancelled,
    /// Provider streaming error
    ProviderStreaming {
        message: String,
        /// HTTP status code from the originating response, if any.
        status: Option<u16>,
        /// Whether the error is likely transient and could succeed on retry.
        retryable: bool,
    },
    /// General agent error
    Agent { message: String },
}

impl fmt::Display for AgentError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Cancelled => write!(f, "operation cancelled"),
            Self::ProviderStreaming { message, .. } => write!(f, "provider streaming error: {message}"),
            Self::Agent { message } => write!(f, "agent error: {message}"),
        }
    }
}

impl std::error::Error for AgentError {}

impl AgentError {
    /// Whether this error is likely transient and could succeed on retry.
    pub fn is_retryable(&self) -> bool {
        match self {
            Self::ProviderStreaming { retryable, .. } => *retryable,
            Self::Cancelled | Self::Agent { .. } => false,
        }
    }

    /// HTTP status code, if the error originated from an HTTP response.
    pub fn status_code(&self) -> Option<u16> {
        match self {
            Self::ProviderStreaming { status, .. } => *status,
            _ => None,
        }
    }
}

impl From<clanker_router::Error> for AgentError {
    fn from(e: clanker_router::Error) -> Self {
        let status = e.status_code();
        let retryable = e.is_retryable();
        Self::ProviderStreaming {
            message: e.to_string(),
            status,
            retryable,
        }
    }
}

impl From<clankers_provider::error::ProviderError> for AgentError {
    fn from(e: clankers_provider::error::ProviderError) -> Self {
        let status = e.status;
        let retryable = e.is_retryable() && !e.should_compress();
        Self::ProviderStreaming {
            message: e.message,
            status,
            retryable,
        }
    }
}

pub type Result<T> = std::result::Result<T, AgentError>;
