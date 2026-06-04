use chrono::DateTime;
use chrono::Utc;
use serde::Deserialize;
use serde::Serialize;
use serde_json::Value;

const HOOK_PREVIEW_MAX_CHARS: usize = 160;
const HOOK_ERROR_MAX_CHARS: usize = 240;
const HOOK_ERROR_KIND_MAX_CHARS: usize = 64;
const REDACTED_SECRET_TEXT: &str = "[redacted secret-like text]";

fn default_empty_string() -> String {
    String::new()
}

fn default_json_value() -> Value {
    Value::Null
}

fn default_none_json_value() -> Option<Value> {
    None
}

fn default_none_string() -> Option<String> {
    None
}

fn default_empty_string_vec() -> Vec<String> {
    Vec::new()
}

fn default_prompt_id() -> String {
    String::new()
}

fn default_prompt_preview() -> String {
    String::new()
}

fn default_prompt_digest() -> String {
    String::new()
}

fn default_hook_status() -> HookStatus {
    HookStatus::Pending
}

fn default_zero_u64() -> u64 {
    0
}

fn default_none_hook_error() -> Option<HookSafeError> {
    None
}

fn default_none_hook_usage() -> Option<HookUsage> {
    None
}

#[cfg_attr(
    dylint_lib = "tigerstyle",
    allow(
        ambient_clock,
        reason = "hook payload timestamps are captured at the hook dispatch boundary"
    )
)]
fn payload_timestamp() -> DateTime<Utc> {
    Utc::now()
}

fn prompt_digest(text: &str) -> String {
    blake3::hash(text.as_bytes()).to_string()
}

fn contains_secret_marker(text: &str) -> bool {
    let lower = text.to_ascii_lowercase();
    [
        "api_key",
        "apikey",
        "authorization:",
        "bearer ",
        "password",
        "secret",
        "token=",
        "token:",
        "sk-",
    ]
    .iter()
    .any(|marker| lower.contains(marker))
}

fn truncate_chars(text: &str, max_chars: usize) -> String {
    assert!(max_chars > 0);
    let mut chars = text.chars();
    let mut truncated: String = chars.by_ref().take(max_chars).collect();
    if chars.next().is_some() {
        truncated.push('…');
    }
    truncated
}

fn safe_preview(text: &str) -> String {
    if contains_secret_marker(text) {
        return REDACTED_SECRET_TEXT.to_string();
    }
    truncate_chars(text, HOOK_PREVIEW_MAX_CHARS)
}

fn safe_error_message(message: &str) -> String {
    if contains_secret_marker(message) {
        return REDACTED_SECRET_TEXT.to_string();
    }
    truncate_chars(message, HOOK_ERROR_MAX_CHARS)
}

/// Prompt/turn outcome status exposed to hook handlers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HookStatus {
    /// Hook fired before an outcome is known.
    #[default]
    Pending,
    /// Prompt or turn finished successfully.
    Success,
    /// A blocking hook or policy denied execution.
    Denied,
    /// Execution was cancelled.
    Cancelled,
    /// Execution failed for another reason.
    Error,
}

/// Safe, bounded error metadata for post hooks.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HookSafeError {
    pub message: String,
    #[serde(default = "default_none_string", skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
}

impl HookSafeError {
    pub fn new(message: impl AsRef<str>, kind: Option<&str>) -> Self {
        Self {
            message: safe_error_message(message.as_ref()),
            kind: kind.map(|value| truncate_chars(value, HOOK_ERROR_KIND_MAX_CHARS)),
        }
    }
}

/// Token usage metadata for post-turn hooks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct HookUsage {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cache_creation_input_tokens: u64,
    pub cache_read_input_tokens: u64,
}

/// Payload delivered to every hook handler.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HookPayload {
    /// Hook point name (e.g. "pre_tool")
    pub hook: String,
    /// Current session ID
    #[serde(default = "default_empty_string")]
    pub session_id: String,
    /// When the hook fired
    pub timestamp: DateTime<Utc>,
    /// Hook-specific data
    #[serde(flatten)]
    pub data: HookData,
}

/// Hook-specific payload data, tagged by kind.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind")]
pub enum HookData {
    /// Tool pre/post hook data
    #[serde(rename = "tool")]
    Tool {
        tool_name: String,
        call_id: String,
        #[serde(default = "default_json_value")]
        input: Value,
        #[serde(default = "default_none_json_value", skip_serializing_if = "Option::is_none")]
        result: Option<Value>,
    },
    /// Prompt pre/post hook data
    #[serde(rename = "prompt")]
    Prompt {
        text: String,
        #[serde(default = "default_none_string", skip_serializing_if = "Option::is_none")]
        system_prompt: Option<String>,
        #[serde(default = "default_prompt_id")]
        prompt_id: String,
        #[serde(default = "default_prompt_preview")]
        prompt_preview: String,
        #[serde(default = "default_prompt_digest")]
        prompt_digest: String,
        #[serde(default = "default_hook_status")]
        status: HookStatus,
        #[serde(default = "default_none_hook_error", skip_serializing_if = "Option::is_none")]
        error: Option<HookSafeError>,
    },
    /// Prompt-level agent turn hook data.
    #[serde(rename = "turn")]
    Turn {
        #[serde(default = "default_prompt_id")]
        prompt_id: String,
        model: String,
        #[serde(default = "default_prompt_preview")]
        prompt_preview: String,
        #[serde(default = "default_prompt_digest")]
        prompt_digest: String,
        #[serde(default = "default_zero_u64")]
        message_count: u64,
        #[serde(default = "default_zero_u64")]
        tool_call_count: u64,
        #[serde(default = "default_hook_status")]
        status: HookStatus,
        #[serde(default = "default_none_hook_error", skip_serializing_if = "Option::is_none")]
        error: Option<HookSafeError>,
        #[serde(default = "default_none_hook_usage", skip_serializing_if = "Option::is_none")]
        usage: Option<HookUsage>,
    },
    /// Session lifecycle data
    #[serde(rename = "session")]
    Session { session_id: String },
    /// Git operation data
    #[serde(rename = "git")]
    Git {
        action: String,
        #[serde(default = "default_none_string", skip_serializing_if = "Option::is_none")]
        hash: Option<String>,
        #[serde(default = "default_none_string", skip_serializing_if = "Option::is_none")]
        message: Option<String>,
        #[serde(default = "default_empty_string_vec")]
        files: Vec<String>,
    },
    /// Error data
    #[serde(rename = "error")]
    Error {
        message: String,
        #[serde(default = "default_none_string", skip_serializing_if = "Option::is_none")]
        source: Option<String>,
    },
    /// Model change data
    #[serde(rename = "model_change")]
    ModelChange { from: String, to: String, reason: String },
    /// Minimal / no data (e.g. turn start/end)
    #[serde(rename = "empty")]
    Empty {},
}

impl HookPayload {
    /// Create a tool hook payload.
    #[cfg_attr(
        dylint_lib = "tigerstyle",
        allow(too_many_parameters, reason = "hook payload builder mirrors stable hook wire fields")
    )]
    pub fn tool(
        hook: &str,
        session_id: &str,
        tool_name: &str,
        call_id: &str,
        input: Value,
        result: Option<Value>,
    ) -> Self {
        Self {
            hook: hook.to_string(),
            session_id: session_id.to_string(),
            timestamp: payload_timestamp(),
            data: HookData::Tool {
                tool_name: tool_name.to_string(),
                call_id: call_id.to_string(),
                input,
                result,
            },
        }
    }

    /// Create a prompt hook payload.
    pub fn prompt(hook: &str, session_id: &str, text: &str, system_prompt: Option<&str>) -> Self {
        let digest = prompt_digest(text);
        let prompt_id = format!("prompt:{digest}");
        Self::prompt_with_metadata(hook, session_id, &prompt_id, text, system_prompt, HookStatus::Pending, None)
    }

    /// Create a prompt hook payload with correlation and outcome metadata.
    #[cfg_attr(
        dylint_lib = "tigerstyle",
        allow(too_many_parameters, reason = "hook payload builder mirrors stable hook wire fields")
    )]
    pub fn prompt_with_metadata(
        hook: &str,
        session_id: &str,
        prompt_id: &str,
        text: &str,
        system_prompt: Option<&str>,
        status: HookStatus,
        error: Option<HookSafeError>,
    ) -> Self {
        Self {
            hook: hook.to_string(),
            session_id: session_id.to_string(),
            timestamp: payload_timestamp(),
            data: HookData::Prompt {
                text: text.to_string(),
                system_prompt: system_prompt.map(String::from),
                prompt_id: prompt_id.to_string(),
                prompt_preview: safe_preview(text),
                prompt_digest: prompt_digest(text),
                status,
                error,
            },
        }
    }

    /// Create a prompt-level agent turn hook payload.
    #[cfg_attr(
        dylint_lib = "tigerstyle",
        allow(too_many_parameters, reason = "hook payload builder mirrors stable hook wire fields")
    )]
    pub fn turn(
        hook: &str,
        session_id: &str,
        prompt_id: &str,
        model: &str,
        prompt_text: &str,
        message_count: u64,
        tool_call_count: u64,
        status: HookStatus,
        error: Option<HookSafeError>,
        usage: Option<HookUsage>,
    ) -> Self {
        Self {
            hook: hook.to_string(),
            session_id: session_id.to_string(),
            timestamp: payload_timestamp(),
            data: HookData::Turn {
                prompt_id: prompt_id.to_string(),
                model: model.to_string(),
                prompt_preview: safe_preview(prompt_text),
                prompt_digest: prompt_digest(prompt_text),
                message_count,
                tool_call_count,
                status,
                error,
                usage,
            },
        }
    }

    /// Create a session hook payload.
    pub fn session(hook: &str, session_id: &str) -> Self {
        Self {
            hook: hook.to_string(),
            session_id: session_id.to_string(),
            timestamp: payload_timestamp(),
            data: HookData::Session {
                session_id: session_id.to_string(),
            },
        }
    }

    /// Create a git hook payload.
    #[cfg_attr(
        dylint_lib = "tigerstyle",
        allow(too_many_parameters, reason = "hook payload builder mirrors stable hook wire fields")
    )]
    pub fn git(
        hook: &str,
        session_id: &str,
        action: &str,
        hash: Option<&str>,
        message: Option<&str>,
        files: Vec<String>,
    ) -> Self {
        Self {
            hook: hook.to_string(),
            session_id: session_id.to_string(),
            timestamp: payload_timestamp(),
            data: HookData::Git {
                action: action.to_string(),
                hash: hash.map(String::from),
                message: message.map(String::from),
                files,
            },
        }
    }

    /// Create an error hook payload.
    pub fn error(hook: &str, session_id: &str, message: &str, source: Option<&str>) -> Self {
        Self {
            hook: hook.to_string(),
            session_id: session_id.to_string(),
            timestamp: payload_timestamp(),
            data: HookData::Error {
                message: message.to_string(),
                source: source.map(String::from),
            },
        }
    }

    /// Create a model change hook payload.
    pub fn model_change(hook: &str, session_id: &str, from: &str, to: &str, reason: &str) -> Self {
        Self {
            hook: hook.to_string(),
            session_id: session_id.to_string(),
            timestamp: payload_timestamp(),
            data: HookData::ModelChange {
                from: from.to_string(),
                to: to.to_string(),
                reason: reason.to_string(),
            },
        }
    }

    /// Create an empty hook payload (e.g. turn start).
    pub fn empty(hook: &str, session_id: &str) -> Self {
        Self {
            hook: hook.to_string(),
            session_id: session_id.to_string(),
            timestamp: payload_timestamp(),
            data: HookData::Empty {},
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prompt_payload_includes_correlation_preview_digest_and_status() {
        let payload = HookPayload::prompt_with_metadata(
            "pre-prompt",
            "session-1",
            "prompt-1",
            "explain rust lifetimes",
            Some("system prompt"),
            HookStatus::Pending,
            None,
        );
        let json = serde_json::to_value(&payload).expect("payload serializes");

        assert_eq!(json["kind"], "prompt");
        assert_eq!(json["prompt_id"], "prompt-1");
        assert_eq!(json["prompt_preview"], "explain rust lifetimes");
        assert_eq!(json["status"], "pending");
        assert_eq!(json["text"], "explain rust lifetimes");
        assert!(json["prompt_digest"].as_str().expect("digest string").len() >= 32);
    }

    #[test]
    fn prompt_payload_redacts_secret_like_preview_without_changing_raw_prompt_contract() {
        let payload = HookPayload::prompt_with_metadata(
            "pre-prompt",
            "session-1",
            "prompt-1",
            "use token=super-secret-value",
            None,
            HookStatus::Pending,
            None,
        );
        let json = serde_json::to_value(&payload).expect("payload serializes");

        assert_eq!(json["text"], "use token=super-secret-value");
        assert_eq!(json["prompt_preview"], REDACTED_SECRET_TEXT);
        assert!(!json["prompt_digest"].as_str().expect("digest string").contains("super-secret-value"));
    }

    #[test]
    fn turn_payload_has_safe_fields_and_no_raw_prompt_or_system_prompt() {
        let payload = HookPayload::turn(
            "post-turn",
            "session-1",
            "prompt-1",
            "model-a",
            "deploy with password=hunter2",
            12,
            3,
            HookStatus::Error,
            Some(HookSafeError::new("provider saw token=hidden", Some("provider_streaming"))),
            Some(HookUsage {
                input_tokens: 10,
                output_tokens: 20,
                cache_creation_input_tokens: 1,
                cache_read_input_tokens: 2,
            }),
        );
        let json = serde_json::to_value(&payload).expect("payload serializes");

        assert_eq!(json["kind"], "turn");
        assert_eq!(json["prompt_id"], "prompt-1");
        assert_eq!(json["model"], "model-a");
        assert_eq!(json["message_count"], 12);
        assert_eq!(json["tool_call_count"], 3);
        assert_eq!(json["status"], "error");
        assert_eq!(json["prompt_preview"], REDACTED_SECRET_TEXT);
        assert_eq!(json["error"]["message"], REDACTED_SECRET_TEXT);
        assert_eq!(json["usage"]["input_tokens"], 10);
        assert!(json.get("text").is_none());
        assert!(json.get("system_prompt").is_none());
    }

    #[test]
    fn previews_are_bounded_on_character_boundaries() {
        let prompt = "é".repeat(HOOK_PREVIEW_MAX_CHARS + 3);
        let payload = HookPayload::prompt_with_metadata(
            "pre-prompt",
            "session-1",
            "prompt-1",
            &prompt,
            None,
            HookStatus::Pending,
            None,
        );
        let HookData::Prompt { prompt_preview, .. } = payload.data else {
            panic!("expected prompt data");
        };

        assert_eq!(prompt_preview.chars().count(), HOOK_PREVIEW_MAX_CHARS + 1);
        assert!(prompt_preview.ends_with('…'));
    }
}
