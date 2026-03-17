//! Shared types used across protocol messages.

use serde::Deserialize;
use serde::Serialize;

use crate::control::ControlCommand;

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

/// Identifies a session by transport + sender.
///
/// Used to map incoming connections (iroh peers, Matrix users) to persistent
/// agent sessions. Shared across daemon modules so both the control plane
/// and transport bridges can look up sessions by key.
#[derive(Debug, Clone, Hash, Eq, PartialEq, Serialize, Deserialize)]
pub enum SessionKey {
    /// iroh peer identified by public key.
    Iroh(String),
    /// Matrix user in a room.
    Matrix { user_id: String, room_id: String },
}

impl std::fmt::Display for SessionKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Iroh(id) => write!(f, "iroh:{}", &id[..12.min(id.len())]),
            Self::Matrix { user_id, room_id } => write!(f, "matrix:{}@{}", user_id, room_id),
        }
    }
}

impl SessionKey {
    /// Deterministic directory name for this session's working files.
    pub fn dir_name(&self) -> String {
        match self {
            Self::Iroh(id) => format!("daemon_iroh_{}", &id[..12.min(id.len())]),
            Self::Matrix { user_id, room_id } => {
                let user = user_id.replace(':', "_").replace('@', "");
                let room = room_id.replace(':', "_").replace('!', "");
                format!("daemon_matrix_{}_{}", user, room)
            }
        }
    }

    /// Extract the Matrix room_id if this is a Matrix session.
    pub fn matrix_room_id(&self) -> Option<&str> {
        match self {
            Self::Matrix { room_id, .. } => Some(room_id),
            _ => None,
        }
    }
}

/// Current protocol version.
// r[impl protocol.handshake.version-field]
pub const PROTOCOL_VERSION: u32 = 1;

/// ALPN identifier for session-level QUIC connections.
pub const ALPN_SESSION: &[u8] = b"clankers/session/1";

/// ALPN identifier for daemon control plane over QUIC.
///
/// Carries both control commands (list/create/kill sessions) and session
/// attach streams using the same framing as Unix domain sockets. The first
/// frame on each bi-stream is a [`DaemonRequest`] that selects the mode.
pub const ALPN_DAEMON: &[u8] = b"clankers/daemon/1";

/// First frame on a `clankers/daemon/1` QUIC bi-stream.
///
/// Tells the daemon whether this stream is a one-shot control command
/// or a long-lived session attach.
// r[impl protocol.serde.request-discriminant]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type")]
pub enum DaemonRequest {
    /// One-shot control command (list sessions, create, kill, status).
    Control { command: ControlCommand },
    /// Attach to a session. Followed by the normal SessionCommand/DaemonEvent
    /// bidirectional flow, identical to the Unix socket session protocol.
    Attach { handshake: Handshake },
}

/// Response to a `DaemonRequest::Attach`.
///
/// Sent once after the daemon processes the attach request, before the
/// bidirectional event stream begins.
// r[impl protocol.serde.attach-response-discriminant]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type")]
pub enum AttachResponse {
    /// Successfully attached. The stream now carries DaemonEvent/SessionCommand frames.
    Ok { session_id: String },
    /// Attach failed.
    Error { message: String },
}
