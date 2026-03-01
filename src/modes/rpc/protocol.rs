//! Wire protocol for clankers peer-to-peer communication.
//!
//! Each QUIC bidirectional stream carries one request/response exchange.
//! All frames are length-prefixed JSON: `[4-byte big-endian length][JSON payload]`.
//!
//! The stream itself is the correlation — no IDs needed.

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
