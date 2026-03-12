use chrono::DateTime;
use chrono::Utc;
use serde::Deserialize;
use serde::Serialize;
use serde_json::Value;

/// Payload delivered to every hook handler.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HookPayload {
    /// Hook point name (e.g. "pre_tool")
    pub hook: String,
    /// Current session ID
    #[serde(default)]
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
        #[serde(default)]
        input: Value,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        result: Option<Value>,
    },
    /// Prompt pre/post hook data
    #[serde(rename = "prompt")]
    Prompt {
        text: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        system_prompt: Option<String>,
    },
    /// Session lifecycle data
    #[serde(rename = "session")]
    Session { session_id: String },
    /// Git operation data
    #[serde(rename = "git")]
    Git {
        action: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        hash: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        message: Option<String>,
        #[serde(default)]
        files: Vec<String>,
    },
    /// Error data
    #[serde(rename = "error")]
    Error {
        message: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
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
            timestamp: Utc::now(),
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
        Self {
            hook: hook.to_string(),
            session_id: session_id.to_string(),
            timestamp: Utc::now(),
            data: HookData::Prompt {
                text: text.to_string(),
                system_prompt: system_prompt.map(String::from),
            },
        }
    }

    /// Create a session hook payload.
    pub fn session(hook: &str, session_id: &str) -> Self {
        Self {
            hook: hook.to_string(),
            session_id: session_id.to_string(),
            timestamp: Utc::now(),
            data: HookData::Session {
                session_id: session_id.to_string(),
            },
        }
    }

    /// Create a git hook payload.
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
            timestamp: Utc::now(),
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
            timestamp: Utc::now(),
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
            timestamp: Utc::now(),
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
            timestamp: Utc::now(),
            data: HookData::Empty {},
        }
    }
}
