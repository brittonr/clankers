//! Wire protocol for clankers peer-to-peer communication.
//!
//! Each QUIC bidirectional stream carries one request/response exchange.
//! All frames are length-prefixed JSON: `[4-byte big-endian length][JSON payload]`.
//!
//! The stream itself is the correlation — no IDs needed.

#![allow(unexpected_cfgs)]
#![cfg_attr(dylint_lib = "tigerstyle", allow(compound_assertion, ignored_result, no_unwrap, no_panic, no_todo, unjustified_no_todo_allow, no_recursion, unchecked_narrowing, unchecked_division, unbounded_loop, catch_all_on_enum, explicit_defaults, unbounded_channel, unbounded_collection_growth, assertion_density, raw_arithmetic_overflow, sentinel_fallback, acronym_style, bool_naming, negated_predicate, numeric_units, float_for_currency, function_length, nested_conditionals, platform_dependent_cast, usize_in_public_api, too_many_parameters, compound_condition, unjustified_allow, ambiguous_params, ambient_clock, verified_purity, contradictory_time, multi_lock_ordering, reason = "full workspace tigerstyle audit gate: legacy debt documented locally while cleanup proceeds incrementally"))]

use serde::Deserialize;
use serde::Serialize;
use serde_json::Value;

/// A request sent on a QUIC bidi stream.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Request {
    /// Method name (ping, version, status, prompt, file.send, file.recv, …)
    pub method: String,
    /// Method parameters (method-specific)
    #[serde(default, skip_serializing_if = "Value::is_null")]
    pub params: Value,
}

impl Request {
    pub fn new(method: impl Into<String>, params: Value) -> Self {
        Self {
            method: method.into(),
            params,
        }
    }

    /// Convenience: param-less request.
    pub fn simple(method: impl Into<String>) -> Self {
        Self {
            method: method.into(),
            params: Value::Null,
        }
    }
}

/// A successful or error response on the same stream.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Response {
    /// Present on success.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ok: Option<Value>,
    /// Present on error.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl Response {
    pub fn success(value: Value) -> Self {
        Self {
            ok: Some(value),
            error: None,
        }
    }

    pub fn error(message: impl Into<String>) -> Self {
        Self {
            ok: None,
            error: Some(message.into()),
        }
    }

    pub fn is_ok(&self) -> bool {
        self.ok.is_some()
    }
}
