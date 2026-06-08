//! Plain LLM contract types shared by message, router, provider, and engine crates.
//!
//! This module intentionally contains only serde-friendly data contracts. It must
//! not depend on provider implementations, router runtime services, async runtimes,
//! databases, network clients, daemon protocols, or UI crates.

use serde::Deserialize;
use serde::Serialize;

/// Tool definition for function calling.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    pub input_schema: serde_json::Value,
}

/// Metadata about an available tool for inventory/projection surfaces.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ToolInfo {
    pub name: String,
    pub description: String,
    /// Source of the tool: "built-in" or plugin name.
    #[serde(default)]
    pub source: String,
}

/// Minimal serialized message used for seeding and replaying session history.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SerializedMessage {
    pub role: String,
    pub content: String,
    pub model: Option<String>,
    pub timestamp: Option<String>,
}

/// Identifies a daemon session by transport and sender.
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

/// Summary of an active daemon session.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SessionSummary {
    pub session_id: String,
    pub model: String,
    pub turn_count: usize,
    pub last_active: String,
    pub client_count: usize,
    pub socket_path: String,
    /// Lifecycle state: "active", "suspended", or "recovering".
    #[serde(default = "default_session_state")]
    pub state: String,
}

fn default_session_state() -> String {
    "active".to_string()
}

/// Daemon runtime status.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DaemonStatus {
    pub uptime_secs: f64,
    pub session_count: usize,
    pub total_clients: usize,
    pub pid: u32,
}

/// Named thinking budget levels shared by provider, controller, and display edges.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ThinkingLevel {
    /// Thinking disabled.
    Off,
    /// Quick reasoning (~5k tokens).
    Low,
    /// Moderate reasoning (~10k tokens).
    Medium,
    /// Deep reasoning (~32k tokens).
    High,
    /// Maximum reasoning (~128k tokens).
    Max,
}

impl ThinkingLevel {
    /// Token budget for this level (None for Off).
    pub const fn budget_tokens(self) -> Option<u32> {
        match self {
            Self::Off => None,
            Self::Low => Some(5_000),
            Self::Medium => Some(10_000),
            Self::High => Some(32_000),
            Self::Max => Some(128_000),
        }
    }

    /// Whether thinking is enabled at this level.
    pub const fn is_enabled(self) -> bool {
        !matches!(self, Self::Off)
    }

    /// Cycle to the next level.
    pub const fn next(self) -> Self {
        match self {
            Self::Off => Self::Low,
            Self::Low => Self::Medium,
            Self::Medium => Self::High,
            Self::High => Self::Max,
            Self::Max => Self::Off,
        }
    }

    /// Display name.
    pub const fn label(self) -> &'static str {
        match self {
            Self::Off => "off",
            Self::Low => "low",
            Self::Medium => "medium",
            Self::High => "high",
            Self::Max => "max",
        }
    }

    /// Parse from a string level name.
    pub fn from_str_or_budget(s: &str) -> Option<Self> {
        match s.trim().to_lowercase().as_str() {
            "off" | "none" | "disable" | "disabled" => Some(Self::Off),
            "low" | "lo" | "l" => Some(Self::Low),
            "medium" | "med" | "m" => Some(Self::Medium),
            "high" | "hi" | "h" => Some(Self::High),
            "xhigh" | "x-high" | "extra-high" | "max" | "maximum" | "full" | "default" => Some(Self::Max),
            _ => None,
        }
    }

    /// Find the closest level for a raw token budget.
    pub const fn from_budget(tokens: u32) -> Self {
        if tokens == 0 {
            Self::Off
        } else if tokens <= 5_000 {
            Self::Low
        } else if tokens <= 10_000 {
            Self::Medium
        } else if tokens <= 32_000 {
            Self::High
        } else {
            Self::Max
        }
    }

    /// All levels in order.
    pub const fn all() -> &'static [Self] {
        &[Self::Off, Self::Low, Self::Medium, Self::High, Self::Max]
    }
}

/// Configuration for extended thinking mode.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThinkingConfig {
    /// Whether extended thinking is enabled.
    pub enabled: bool,
    /// Maximum tokens for thinking.
    pub budget_tokens: Option<usize>,
}

/// Token usage statistics for a completion.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Usage {
    pub input_tokens: usize,
    pub output_tokens: usize,
    pub cache_creation_input_tokens: usize,
    pub cache_read_input_tokens: usize,
}

impl Usage {
    #[cfg_attr(
        dylint_lib = "tigerstyle",
        allow(
            tigerstyle::usize_in_public_api,
            reason = "Usage token counts mirror existing usize fields and internal UI metrics."
        )
    )]
    pub fn total_tokens(&self) -> usize {
        self.input_tokens.saturating_add(self.output_tokens)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tool_info_defaults_missing_source_for_legacy_wire_events() {
        let info: ToolInfo = serde_json::from_str(r#"{"name":"read","description":"Read files"}"#)
            .expect("tool info should deserialize");
        assert_eq!(info.source, "");
    }

    #[test]
    fn serialized_message_roundtrip_preserves_optional_fields() {
        let message = SerializedMessage {
            role: "assistant".to_string(),
            content: "hello".to_string(),
            model: Some("model".to_string()),
            timestamp: None,
        };
        let json = serde_json::to_string(&message).expect("message should serialize");
        let parsed: SerializedMessage = serde_json::from_str(&json).expect("message should deserialize");
        assert_eq!(parsed, message);
    }

    #[test]
    fn session_key_matrix_dir_name_sanitizes() {
        let key = SessionKey::Matrix {
            user_id: "@alice:matrix.org".to_string(),
            room_id: "!room123:matrix.org".to_string(),
        };
        let dir = key.dir_name();
        assert!(!dir.contains('@'));
        assert!(!dir.contains(':'));
        assert!(!dir.contains('!'));
        assert!(dir.starts_with("daemon_matrix_"));
    }

    #[test]
    fn session_key_roundtrip_preserves_matrix_identity() {
        let key = SessionKey::Matrix {
            user_id: "@user:host".to_string(),
            room_id: "!room:host".to_string(),
        };
        let json = serde_json::to_string(&key).expect("key should serialize");
        let parsed: SessionKey = serde_json::from_str(&json).expect("key should deserialize");
        assert_eq!(parsed, key);
        assert_eq!(parsed.matrix_room_id(), Some("!room:host"));
    }

    #[test]
    fn session_summary_defaults_missing_state_for_legacy_wire_events() {
        let summary: SessionSummary = serde_json::from_str(
            r#"{"session_id":"s1","model":"model","turn_count":2,"last_active":"now","client_count":1,"socket_path":"/tmp/sock"}"#,
        )
        .expect("summary should deserialize");
        assert_eq!(summary.state, "active");
    }

    #[test]
    fn daemon_status_roundtrip_preserves_counters() {
        let status = DaemonStatus {
            uptime_secs: 4.5,
            session_count: 2,
            total_clients: 3,
            pid: 42,
        };
        let json = serde_json::to_string(&status).expect("status should serialize");
        let parsed: DaemonStatus = serde_json::from_str(&json).expect("status should deserialize");
        assert_eq!(parsed, status);
    }
}
