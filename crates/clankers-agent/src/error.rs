//! Agent-specific error types

#![allow(unexpected_cfgs)]
#![cfg_attr(dylint_lib = "tigerstyle", allow(compound_assertion, ignored_result, no_unwrap, no_panic, no_todo, unjustified_no_todo_allow, no_recursion, unchecked_narrowing, unchecked_division, unbounded_loop, catch_all_on_enum, explicit_defaults, unbounded_channel, unbounded_collection_growth, assertion_density, raw_arithmetic_overflow, sentinel_fallback, acronym_style, bool_naming, negated_predicate, numeric_units, float_for_currency, function_length, nested_conditionals, platform_dependent_cast, usize_in_public_api, too_many_parameters, compound_condition, unjustified_allow, ambiguous_params, ambient_clock, verified_purity, contradictory_time, multi_lock_ordering, reason = "full workspace tigerstyle audit gate: legacy debt documented locally while cleanup proceeds incrementally"))]

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
