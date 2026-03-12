//! Shared types used across protocol messages.

use serde::Deserialize;
use serde::Serialize;

/// Image payload as base64-encoded data with a media type.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ImageData {
    /// Base64-encoded image data.
    pub data: String,
    /// MIME type (e.g., "image/png").
    pub media_type: String,
}

/// Initial handshake sent by client on connection.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Handshake {
    /// Protocol version (starts at 1).
    pub protocol_version: u32,
    /// Client identifier (e.g., "clankers-tui/0.1.0").
    pub client_name: String,
    /// Optional UCAN token for auth (required for remote connections).
    pub token: Option<String>,
    /// Optional session ID to attach to an existing session.
    pub session_id: Option<String>,
}

/// A serialized agent message for seeding / history replay.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SerializedMessage {
    pub role: String,
    pub content: String,
    pub model: Option<String>,
    pub timestamp: Option<String>,
}

/// Information about a process in the actor tree.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ProcessInfo {
    pub id: u64,
    pub name: Option<String>,
    pub parent: Option<u64>,
    pub children: Vec<u64>,
    pub state: ProcessState,
    pub uptime_secs: f64,
}

/// State of a process in the actor tree.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum ProcessState {
    Running,
    ShuttingDown,
    Dead,
}

/// Current protocol version.
pub const PROTOCOL_VERSION: u32 = 1;

/// ALPN identifier for session-level QUIC connections.
pub const ALPN_SESSION: &[u8] = b"clankers/session/1";
