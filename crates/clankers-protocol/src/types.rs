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

#[cfg(test)]
mod tests {
    use super::*;

    // ── SessionKey Display ──────────────────────────────────────────

    #[test]
    fn session_key_iroh_display_truncates() {
        let key = SessionKey::Iroh("abcdef1234567890".to_string());
        let display = format!("{key}");
        assert_eq!(display, "iroh:abcdef123456");
    }

    #[test]
    fn session_key_iroh_display_short_id() {
        let key = SessionKey::Iroh("abc".to_string());
        let display = format!("{key}");
        assert_eq!(display, "iroh:abc");
    }

    #[test]
    fn session_key_matrix_display() {
        let key = SessionKey::Matrix {
            user_id: "@alice:matrix.org".to_string(),
            room_id: "!room123:matrix.org".to_string(),
        };
        let display = format!("{key}");
        assert_eq!(display, "matrix:@alice:matrix.org@!room123:matrix.org");
    }

    // ── SessionKey dir_name ─────────────────────────────────────────

    #[test]
    fn session_key_iroh_dir_name() {
        let key = SessionKey::Iroh("abcdef1234567890".to_string());
        assert_eq!(key.dir_name(), "daemon_iroh_abcdef123456");
    }

    #[test]
    fn session_key_matrix_dir_name_sanitizes() {
        let key = SessionKey::Matrix {
            user_id: "@alice:matrix.org".to_string(),
            room_id: "!room123:matrix.org".to_string(),
        };
        let dir = key.dir_name();
        // @ and : and ! should be stripped/replaced
        assert!(!dir.contains('@'));
        assert!(!dir.contains(':'));
        assert!(!dir.contains('!'));
        assert!(dir.starts_with("daemon_matrix_"));
    }

    // ── SessionKey matrix_room_id ───────────────────────────────────

    #[test]
    fn session_key_iroh_no_room_id() {
        let key = SessionKey::Iroh("abc".to_string());
        assert!(key.matrix_room_id().is_none());
    }

    #[test]
    fn session_key_matrix_has_room_id() {
        let key = SessionKey::Matrix {
            user_id: "@bob:example.com".to_string(),
            room_id: "!room:example.com".to_string(),
        };
        assert_eq!(key.matrix_room_id(), Some("!room:example.com"));
    }

    // ── SessionKey equality and hashing ─────────────────────────────

    #[test]
    fn session_key_eq_same() {
        let a = SessionKey::Iroh("abc".to_string());
        let b = SessionKey::Iroh("abc".to_string());
        assert_eq!(a, b);
    }

    #[test]
    fn session_key_ne_different_ids() {
        let a = SessionKey::Iroh("abc".to_string());
        let b = SessionKey::Iroh("def".to_string());
        assert_ne!(a, b);
    }

    #[test]
    fn session_key_ne_different_variants() {
        let a = SessionKey::Iroh("abc".to_string());
        let b = SessionKey::Matrix {
            user_id: "abc".to_string(),
            room_id: "room".to_string(),
        };
        assert_ne!(a, b);
    }

    #[test]
    fn session_key_hashable() {
        use std::collections::HashSet;
        let mut set = HashSet::new();
        set.insert(SessionKey::Iroh("a".to_string()));
        set.insert(SessionKey::Iroh("a".to_string()));
        set.insert(SessionKey::Iroh("b".to_string()));
        assert_eq!(set.len(), 2);
    }

    // ── SessionKey serde roundtrip ──────────────────────────────────

    #[test]
    fn session_key_iroh_serde() {
        let key = SessionKey::Iroh("node123".to_string());
        let json = serde_json::to_string(&key).unwrap();
        let decoded: SessionKey = serde_json::from_str(&json).unwrap();
        assert_eq!(key, decoded);
    }

    #[test]
    fn session_key_matrix_serde() {
        let key = SessionKey::Matrix {
            user_id: "@user:host".to_string(),
            room_id: "!room:host".to_string(),
        };
        let json = serde_json::to_string(&key).unwrap();
        let decoded: SessionKey = serde_json::from_str(&json).unwrap();
        assert_eq!(key, decoded);
    }

    // ── ProcessState serde ──────────────────────────────────────────

    #[test]
    fn process_state_roundtrip() {
        for state in [ProcessState::Running, ProcessState::ShuttingDown, ProcessState::Dead] {
            let json = serde_json::to_string(&state).unwrap();
            let decoded: ProcessState = serde_json::from_str(&json).unwrap();
            assert_eq!(state, decoded);
        }
    }

    // ── ProcessInfo serde ───────────────────────────────────────────

    #[test]
    fn process_info_roundtrip() {
        let info = ProcessInfo {
            id: 42,
            name: Some("agent-session".to_string()),
            parent: Some(1),
            children: vec![43, 44],
            state: ProcessState::Running,
            uptime_secs: 123.5,
        };
        let json = serde_json::to_string(&info).unwrap();
        let decoded: ProcessInfo = serde_json::from_str(&json).unwrap();
        assert_eq!(info, decoded);
    }

    #[test]
    fn process_info_no_parent_no_name() {
        let info = ProcessInfo {
            id: 1,
            name: None,
            parent: None,
            children: vec![],
            state: ProcessState::Dead,
            uptime_secs: 0.0,
        };
        let json = serde_json::to_string(&info).unwrap();
        let decoded: ProcessInfo = serde_json::from_str(&json).unwrap();
        assert_eq!(info, decoded);
    }

    // ── ImageData serde ─────────────────────────────────────────────

    #[test]
    fn image_data_roundtrip() {
        let img = ImageData {
            data: "iVBORw0KGgo=".to_string(),
            media_type: "image/png".to_string(),
        };
        let json = serde_json::to_string(&img).unwrap();
        let decoded: ImageData = serde_json::from_str(&json).unwrap();
        assert_eq!(img, decoded);
    }

    // ── Handshake serde ─────────────────────────────────────────────

    #[test]
    fn handshake_minimal() {
        let hs = Handshake {
            protocol_version: PROTOCOL_VERSION,
            client_name: "test".to_string(),
            token: None,
            session_id: None,
        };
        let json = serde_json::to_string(&hs).unwrap();
        let decoded: Handshake = serde_json::from_str(&json).unwrap();
        assert_eq!(hs, decoded);
    }

    #[test]
    fn handshake_full() {
        let hs = Handshake {
            protocol_version: PROTOCOL_VERSION,
            client_name: "clankers-tui/0.2.0".to_string(),
            token: Some("ucan-base64-token".to_string()),
            session_id: Some("sess-abc123".to_string()),
        };
        let json = serde_json::to_string(&hs).unwrap();
        let decoded: Handshake = serde_json::from_str(&json).unwrap();
        assert_eq!(hs, decoded);
    }

    // ── DaemonRequest serde (internally tagged) ─────────────────────

    #[test]
    fn daemon_request_control_serde() {
        let req = DaemonRequest::Control {
            command: ControlCommand::Status,
        };
        let json = serde_json::to_string(&req).unwrap();
        assert!(json.contains(r#""type":"Control""#));
        let decoded: DaemonRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(req, decoded);
    }

    #[test]
    fn daemon_request_attach_serde() {
        let req = DaemonRequest::Attach {
            handshake: Handshake {
                protocol_version: 1,
                client_name: "test".to_string(),
                token: None,
                session_id: None,
            },
        };
        let json = serde_json::to_string(&req).unwrap();
        assert!(json.contains(r#""type":"Attach""#));
        let decoded: DaemonRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(req, decoded);
    }

    // ── AttachResponse serde (internally tagged) ────────────────────

    #[test]
    fn attach_response_ok_serde() {
        let resp = AttachResponse::Ok {
            session_id: "sess-1".to_string(),
        };
        let json = serde_json::to_string(&resp).unwrap();
        assert!(json.contains(r#""type":"Ok""#));
        let decoded: AttachResponse = serde_json::from_str(&json).unwrap();
        assert_eq!(resp, decoded);
    }

    #[test]
    fn attach_response_error_serde() {
        let resp = AttachResponse::Error {
            message: "no such session".to_string(),
        };
        let json = serde_json::to_string(&resp).unwrap();
        let decoded: AttachResponse = serde_json::from_str(&json).unwrap();
        assert_eq!(resp, decoded);
    }

    // ── ALPN constants ──────────────────────────────────────────────

    #[test]
    fn alpn_constants_are_utf8() {
        assert_eq!(std::str::from_utf8(ALPN_SESSION).unwrap(), "clankers/session/1");
        assert_eq!(std::str::from_utf8(ALPN_DAEMON).unwrap(), "clankers/daemon/1");
    }

    #[test]
    fn protocol_version_is_1() {
        assert_eq!(PROTOCOL_VERSION, 1);
    }

    // ── SerializedMessage serde ─────────────────────────────────────

    #[test]
    fn serialized_message_roundtrip() {
        let msg = SerializedMessage {
            role: "user".to_string(),
            content: "hello agent".to_string(),
            model: Some("sonnet".to_string()),
            timestamp: Some("2026-03-21T12:00:00Z".to_string()),
        };
        let json = serde_json::to_string(&msg).unwrap();
        let decoded: SerializedMessage = serde_json::from_str(&json).unwrap();
        assert_eq!(msg, decoded);
    }

    #[test]
    fn serialized_message_minimal() {
        let msg = SerializedMessage {
            role: "assistant".to_string(),
            content: "hi".to_string(),
            model: None,
            timestamp: None,
        };
        let json = serde_json::to_string(&msg).unwrap();
        let decoded: SerializedMessage = serde_json::from_str(&json).unwrap();
        assert_eq!(msg, decoded);
    }
}
