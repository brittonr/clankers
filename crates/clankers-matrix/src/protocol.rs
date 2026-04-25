//! Wire protocol for clankers-to-clankers communication over Matrix.
//!
//! Messages use a custom `m.clankers.*` event type namespace in Matrix rooms.
//! This module defines the envelope types serialized into the message body.

#![allow(unexpected_cfgs)]
#![cfg_attr(dylint_lib = "tigerstyle", allow(compound_assertion, ignored_result, no_unwrap, no_panic, no_todo, unjustified_no_todo_allow, no_recursion, unchecked_narrowing, unchecked_division, unbounded_loop, catch_all_on_enum, explicit_defaults, unbounded_channel, unbounded_collection_growth, assertion_density, raw_arithmetic_overflow, sentinel_fallback, acronym_style, bool_naming, negated_predicate, numeric_units, float_for_currency, function_length, nested_conditionals, platform_dependent_cast, usize_in_public_api, too_many_parameters, compound_condition, unjustified_allow, ambiguous_params, ambient_clock, verified_purity, contradictory_time, multi_lock_ordering, reason = "full workspace tigerstyle audit gate: legacy debt documented locally while cleanup proceeds incrementally"))]

use chrono::DateTime;
use chrono::Utc;
use serde::Deserialize;
use serde::Serialize;
use serde_json::Value;

// ── Message type constants ─────────────────────────────────────────

/// Custom Matrix event type prefix for clankers messages.
pub const CLANKERS_EVENT_PREFIX: &str = "m.clankers";

/// Capability announcement — sent on join and periodically.
pub const EVENT_ANNOUNCE: &str = "m.clankers.announce";

/// JSON-RPC request from one clankers to another.
pub const EVENT_RPC_REQUEST: &str = "m.clankers.rpc.request";

/// JSON-RPC response.
pub const EVENT_RPC_RESPONSE: &str = "m.clankers.rpc.response";

/// Free-form chat message between agents.
pub const EVENT_CHAT: &str = "m.clankers.chat";

// ── Announce ───────────────────────────────────────────────────────

/// Capability advertisement broadcast to the room.
///
/// Other clankers instances use this to build their peer registry,
/// similar to how the iroh RPC `status` endpoint works.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Announce {
    /// clankers version string
    pub version: String,

    /// Human-readable instance name
    pub instance_name: String,

    /// Matrix user ID of this instance
    pub user_id: String,

    /// Capability tags (e.g. "gpu", "code-review")
    #[serde(default)]
    pub tags: Vec<String>,

    /// Available agent definitions
    #[serde(default)]
    pub agents: Vec<String>,

    /// Whether this instance accepts prompts
    #[serde(default)]
    pub accepts_prompts: bool,

    /// Available tool names
    #[serde(default)]
    pub tools: Vec<String>,

    /// Current model name
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,

    /// Timestamp of announcement
    pub timestamp: DateTime<Utc>,
}

impl Announce {
    pub fn new(instance_name: impl Into<String>, user_id: impl Into<String>) -> Self {
        Self {
            version: env!("CARGO_PKG_VERSION").to_string(),
            instance_name: instance_name.into(),
            user_id: user_id.into(),
            tags: Vec::new(),
            agents: Vec::new(),
            accepts_prompts: false,
            tools: Vec::new(),
            model: None,
            timestamp: Utc::now(),
        }
    }
}

// ── RPC Request ────────────────────────────────────────────────────

/// JSON-RPC request envelope carried over Matrix.
///
/// Compatible with the iroh RPC protocol — same methods and params.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RpcRequest {
    /// JSON-RPC version (always "2.0")
    pub jsonrpc: String,

    /// Request ID for correlating responses
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<Value>,

    /// RPC method name (ping, version, status, prompt, etc.)
    pub method: String,

    /// Method parameters
    #[serde(default)]
    pub params: Value,

    /// Target user ID (if addressing a specific clankers in the room)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target: Option<String>,

    /// Sender user ID (filled by the bridge)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sender: Option<String>,
}

impl RpcRequest {
    pub fn new(method: impl Into<String>, params: Value) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            id: Some(Value::String(uuid::Uuid::new_v4().to_string())),
            method: method.into(),
            params,
            target: None,
            sender: None,
        }
    }

    /// Address this request to a specific clankers instance.
    pub fn to(mut self, user_id: impl Into<String>) -> Self {
        self.target = Some(user_id.into());
        self
    }
}

// ── RPC Response ───────────────────────────────────────────────────

/// JSON-RPC response envelope.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RpcResponse {
    pub jsonrpc: String,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<Value>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<RpcError>,

    /// The clankers instance that generated this response
    #[serde(skip_serializing_if = "Option::is_none")]
    pub responder: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RpcError {
    pub code: i32,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
}

impl RpcResponse {
    pub fn success(id: Option<Value>, result: Value) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            id,
            result: Some(result),
            error: None,
            responder: None,
        }
    }

    pub fn error(id: Option<Value>, code: i32, message: impl Into<String>) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            id,
            result: None,
            error: Some(RpcError {
                code,
                message: message.into(),
                data: None,
            }),
            responder: None,
        }
    }
}

// ── Chat ───────────────────────────────────────────────────────────

/// Free-form text message between clankers agents.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    /// The message text (may contain markdown)
    pub body: String,

    /// Sender's instance name
    pub instance_name: String,

    /// Sender's Matrix user ID
    pub user_id: String,

    /// Timestamp
    pub timestamp: DateTime<Utc>,

    /// Optional thread/conversation ID for grouping
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thread_id: Option<String>,
}

impl ChatMessage {
    pub fn new(body: impl Into<String>, instance_name: impl Into<String>, user_id: impl Into<String>) -> Self {
        Self {
            body: body.into(),
            instance_name: instance_name.into(),
            user_id: user_id.into(),
            timestamp: Utc::now(),
            thread_id: None,
        }
    }
}

// ── Parsed incoming message ────────────────────────────────────────

/// A parsed Matrix event relevant to clankers.
#[derive(Debug, Clone)]
pub enum ClankersEvent {
    /// Capability announcement from another clankers
    Announce(Announce),
    /// RPC request from another clankers
    RpcRequest(RpcRequest),
    /// RPC response from another clankers
    RpcResponse(RpcResponse),
    /// Chat message (from clankers or human)
    Chat(ChatMessage),
    /// Regular Matrix text message (from a human client)
    Text {
        sender: String,
        body: String,
        room_id: String,
        timestamp: DateTime<Utc>,
    },
    /// Media message (image, file, audio, video)
    Media {
        sender: String,
        room_id: String,
        /// Display name / caption from the message body
        body: String,
        /// Resolved filename (from filename field, falls back to body)
        filename: String,
        /// Media type: "image", "file", "audio", "video"
        media_type: String,
        /// The media source for downloading
        source: ruma::events::room::MediaSource,
        timestamp: DateTime<Utc>,
    },
}
