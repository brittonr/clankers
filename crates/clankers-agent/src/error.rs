//! Agent-specific error types

use std::fmt;

/// Errors produced by agent operations (turn loop, compaction, etc.)
#[derive(Debug)]
pub enum AgentError {
    /// Operation was cancelled via `CancellationToken`
    Cancelled,
    /// Provider streaming error
    ProviderStreaming { message: String },
    /// General agent error
    Agent { message: String },
}

impl fmt::Display for AgentError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Cancelled => write!(f, "operation cancelled"),
            Self::ProviderStreaming { message } => write!(f, "provider streaming error: {message}"),
            Self::Agent { message } => write!(f, "agent error: {message}"),
        }
    }
}

impl std::error::Error for AgentError {}

impl From<clanker_router::Error> for AgentError {
    fn from(e: clanker_router::Error) -> Self {
        Self::ProviderStreaming { message: e.to_string() }
    }
}

impl From<clankers_provider::error::ProviderError> for AgentError {
    fn from(e: clankers_provider::error::ProviderError) -> Self {
        Self::ProviderStreaming { message: e.message }
    }
}

pub type Result<T> = std::result::Result<T, AgentError>;
