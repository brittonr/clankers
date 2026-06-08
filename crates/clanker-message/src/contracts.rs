//! Plain LLM contract types shared by message, router, provider, and engine crates.
//!
//! This module intentionally contains only serde-friendly data contracts. It must
//! not depend on provider implementations, router runtime services, async runtimes,
//! databases, network clients, daemon protocols, or UI crates.

use std::time::Duration;

use serde::Deserialize;
use serde::Serialize;
use serde_json::Value;

use crate::content::Content;

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

const RUNTIME_RETRY_DELAY_MS_MAX: u64 = 365 * 24 * 60 * 60 * 1000;

/// Role for provider messages exchanged with host model adapters.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderMessageRole {
    User,
    Assistant,
    Tool,
    System,
}

/// Provider message exchanged with host model adapters.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderMessage {
    pub role: ProviderMessageRole,
    pub content: Vec<Content>,
    pub id: Option<String>,
    pub model: Option<String>,
    pub call_id: Option<String>,
    pub tool_name: Option<String>,
    pub is_error: bool,
}

impl ProviderMessage {
    #[must_use]
    pub fn user_text(prompt: impl Into<String>) -> Self {
        Self {
            role: ProviderMessageRole::User,
            content: vec![Content::Text { text: prompt.into() }],
            id: None,
            model: None,
            call_id: None,
            tool_name: None,
            is_error: false,
        }
    }

    #[must_use]
    pub fn assistant(content: Vec<Content>, model: Option<String>) -> Self {
        Self {
            role: ProviderMessageRole::Assistant,
            content,
            id: None,
            model,
            call_id: None,
            tool_name: None,
            is_error: false,
        }
    }

    #[must_use]
    pub fn tool_result(
        call_id: impl Into<String>,
        tool_name: impl Into<String>,
        content: Vec<Content>,
        is_error: bool,
    ) -> Self {
        Self {
            role: ProviderMessageRole::Tool,
            content,
            id: None,
            model: None,
            call_id: Some(call_id.into()),
            tool_name: Some(tool_name.into()),
            is_error,
        }
    }
}

/// Provider stream event exchanged with host model adapters.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ProviderStreamEvent {
    MessageStart {
        model: String,
        role: String,
    },
    ContentBlockStart {
        index: usize,
        content: Content,
    },
    TextDelta {
        index: usize,
        text: String,
    },
    ThinkingDelta {
        index: usize,
        thinking: String,
    },
    ToolInputJsonDelta {
        index: usize,
        partial_json: String,
    },
    SignatureDelta {
        index: usize,
        signature: String,
    },
    ContentBlockStop {
        index: usize,
    },
    Usage {
        stop_reason: Option<crate::content::StopReason>,
        usage: Usage,
    },
    MessageStop,
    Error {
        message: String,
    },
}

/// Provider model call status exchanged with host model adapters.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderModelStatus {
    Completed,
    RetryableFailure,
    TerminalFailure,
    Cancelled,
}

/// Provider model failure details exchanged with host model adapters.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProviderModelFailure {
    pub message: String,
    pub status: Option<u16>,
    pub retryable: bool,
}

impl ProviderModelFailure {
    #[must_use]
    pub fn retryable(message: impl Into<String>, status: Option<u16>) -> Self {
        Self {
            message: sanitize_short_public_value(message.into()),
            status,
            retryable: true,
        }
    }

    #[must_use]
    pub fn terminal(message: impl Into<String>, status: Option<u16>) -> Self {
        Self {
            message: sanitize_short_public_value(message.into()),
            status,
            retryable: false,
        }
    }
}

fn sanitize_short_public_value(value: String) -> String {
    let lower = value.to_ascii_lowercase();
    let contains_secret = [
        "token",
        "secret",
        "password",
        "api_key",
        "authorization",
        "bearer",
        "cookie",
    ]
    .iter()
    .any(|marker| lower.contains(marker));
    if contains_secret {
        "[REDACTED]".to_string()
    } else {
        value.chars().take(160).collect()
    }
}

/// Extension execution status returned by host extension adapters.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExtensionStatus {
    Succeeded,
    Failed,
    Disabled,
    Unavailable,
}

/// Runtime tool execution status returned by host tool adapters.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeToolStatus {
    Succeeded,
    Failed,
    Missing,
    Denied,
    Cancelled,
}

/// Runtime tool response returned by host tool adapters.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeToolResponse {
    pub status: RuntimeToolStatus,
    #[serde(default)]
    pub content: Vec<Content>,
    #[serde(default)]
    pub details: Value,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

impl RuntimeToolResponse {
    #[must_use]
    pub fn succeeded(content: Vec<Content>, details: Value) -> Self {
        Self {
            status: RuntimeToolStatus::Succeeded,
            content,
            details,
            message: None,
        }
    }

    #[must_use]
    pub fn failed(message: impl Into<String>) -> Self {
        let message = message.into();
        Self {
            status: RuntimeToolStatus::Failed,
            content: vec![Content::Text { text: message.clone() }],
            details: Value::Null,
            message: Some(message),
        }
    }
}

/// Runtime retry request passed to host retry adapters.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeRetryRequest {
    pub request_id: String,
    pub delay_ms: u64,
}

impl RuntimeRetryRequest {
    #[must_use]
    pub fn new(request_id: impl Into<String>, delay: Duration) -> Self {
        Self {
            request_id: request_id.into(),
            delay_ms: runtime_retry_delay_ms(delay),
        }
    }
}

fn runtime_retry_delay_ms(delay: Duration) -> u64 {
    let delay_ms = delay.as_millis();
    let delay_ms_max = u128::from(RUNTIME_RETRY_DELAY_MS_MAX);
    if delay_ms > delay_ms_max {
        return RUNTIME_RETRY_DELAY_MS_MAX;
    }
    match u64::try_from(delay_ms) {
        Ok(value) => value,
        Err(_) => RUNTIME_RETRY_DELAY_MS_MAX,
    }
}

/// Runtime usage observation emitted by model/streaming adapters.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeUsageObservation {
    pub kind: RuntimeUsageObservationKind,
    pub usage: Usage,
}

/// Kind of runtime usage observation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeUsageObservationKind {
    StreamDelta,
    FinalSummary,
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

    #[test]
    fn provider_message_tool_result_preserves_call_metadata() {
        let message = ProviderMessage::tool_result(
            "call-1",
            "read",
            vec![Content::Text {
                text: "result".to_string(),
            }],
            true,
        );
        assert_eq!(message.role, ProviderMessageRole::Tool);
        assert_eq!(message.call_id.as_deref(), Some("call-1"));
        assert_eq!(message.tool_name.as_deref(), Some("read"));
        assert!(message.is_error);
    }

    #[test]
    fn provider_stream_event_usage_roundtrip_preserves_snake_case_type() {
        let event = ProviderStreamEvent::Usage {
            stop_reason: Some(crate::content::StopReason::Stop),
            usage: Usage {
                input_tokens: 1,
                output_tokens: 2,
                cache_creation_input_tokens: 3,
                cache_read_input_tokens: 4,
            },
        };
        let json = serde_json::to_string(&event).expect("event should serialize");
        assert!(json.contains(r#""type":"usage""#));
        let parsed: ProviderStreamEvent = serde_json::from_str(&json).expect("event should deserialize");
        assert!(matches!(parsed, ProviderStreamEvent::Usage { stop_reason: Some(crate::content::StopReason::Stop), .. }));
    }

    #[test]
    fn provider_model_failure_helpers_sanitize_and_mark_retryability() {
        let retryable = ProviderModelFailure::retryable("bearer token leaked", Some(429));
        assert_eq!(retryable.message, "[REDACTED]");
        assert_eq!(retryable.status, Some(429));
        assert!(retryable.retryable);

        let terminal = ProviderModelFailure::terminal("permanent failure", Some(400));
        assert_eq!(terminal.message, "permanent failure");
        assert_eq!(terminal.status, Some(400));
        assert!(!terminal.retryable);
    }

    #[test]
    fn provider_model_status_roundtrip_preserves_snake_case() {
        let json = serde_json::to_string(&ProviderModelStatus::RetryableFailure)
            .expect("status should serialize");
        assert_eq!(json, r#""retryable_failure""#);
        let parsed: ProviderModelStatus = serde_json::from_str(&json).expect("status should deserialize");
        assert_eq!(parsed, ProviderModelStatus::RetryableFailure);
    }

    #[test]
    fn extension_status_roundtrip_preserves_snake_case() {
        let json = serde_json::to_string(&ExtensionStatus::Unavailable).expect("status should serialize");
        assert_eq!(json, r#""unavailable""#);
        let parsed: ExtensionStatus = serde_json::from_str(&json).expect("status should deserialize");
        assert_eq!(parsed, ExtensionStatus::Unavailable);
    }

    #[test]
    fn runtime_tool_response_failed_helper_preserves_message() {
        let response = RuntimeToolResponse::failed("tool unavailable");
        assert_eq!(response.status, RuntimeToolStatus::Failed);
        assert_eq!(response.message.as_deref(), Some("tool unavailable"));
        assert!(matches!(response.content.first(), Some(Content::Text { text }) if text == "tool unavailable"));
    }

    #[test]
    fn runtime_tool_response_roundtrip_preserves_status_and_details() {
        let response = RuntimeToolResponse::succeeded(
            vec![Content::Text {
                text: "done".to_string(),
            }],
            serde_json::json!({"exit_code":0}),
        );
        let json = serde_json::to_string(&response).expect("response should serialize");
        assert!(json.contains("succeeded"));
        let parsed: RuntimeToolResponse = serde_json::from_str(&json).expect("response should deserialize");
        assert_eq!(parsed.status, RuntimeToolStatus::Succeeded);
        assert_eq!(parsed.details["exit_code"], 0);
        assert!(matches!(parsed.content.first(), Some(Content::Text { text }) if text == "done"));
    }

    #[test]
    fn runtime_retry_request_clamps_large_delays() {
        let request = RuntimeRetryRequest::new("retry-1", Duration::from_secs(u64::MAX));
        assert_eq!(request.request_id, "retry-1");
        assert_eq!(request.delay_ms, 365 * 24 * 60 * 60 * 1000);
    }

    #[test]
    fn runtime_retry_request_roundtrip_preserves_delay() {
        let request = RuntimeRetryRequest::new("retry-2", Duration::from_millis(42));
        let json = serde_json::to_string(&request).expect("request should serialize");
        let parsed: RuntimeRetryRequest = serde_json::from_str(&json).expect("request should deserialize");
        assert_eq!(parsed.request_id, "retry-2");
        assert_eq!(parsed.delay_ms, 42);
    }

    #[test]
    fn runtime_usage_observation_roundtrip_preserves_kind_and_usage() {
        let observation = RuntimeUsageObservation {
            kind: RuntimeUsageObservationKind::FinalSummary,
            usage: Usage {
                input_tokens: 3,
                output_tokens: 5,
                cache_creation_input_tokens: 7,
                cache_read_input_tokens: 11,
            },
        };
        let json = serde_json::to_string(&observation).expect("observation should serialize");
        assert!(json.contains("final_summary"));
        let parsed: RuntimeUsageObservation = serde_json::from_str(&json).expect("observation should deserialize");
        assert_eq!(parsed.kind, RuntimeUsageObservationKind::FinalSummary);
        assert_eq!(parsed.usage.input_tokens, 3);
        assert_eq!(parsed.usage.output_tokens, 5);
        assert_eq!(parsed.usage.cache_creation_input_tokens, 7);
        assert_eq!(parsed.usage.cache_read_input_tokens, 11);
    }
}
