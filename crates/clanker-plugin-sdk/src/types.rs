//! Protocol types shared between clankers host and WASM plugins.
//!
//! These types define the JSON wire format for tool calls, events,
//! and plugin metadata. Every clankers plugin speaks this protocol.

use serde::Deserialize;
use serde::Serialize;

// ── Tool call protocol ──────────────────────────────────────────────

/// Inbound tool-call envelope from the host.
///
/// The host sends this when the LLM invokes a plugin-provided tool.
/// `tool` is the tool name (e.g. `"hash_text"`), and `args` is the
/// JSON object the LLM provided as parameters.
#[derive(Debug, Clone, Deserialize)]
pub struct ToolCall {
    pub tool: String,
    pub args: serde_json::Value,
}

/// Outbound tool-call result returned to the host.
///
/// `status` should be `"ok"` on success. On error, use a descriptive
/// tag like `"error: missing parameter"`. The `result` field contains
/// the tool's output (plain text or JSON string).
#[derive(Debug, Clone, Serialize)]
pub struct ToolResult {
    pub tool: String,
    pub result: String,
    pub status: String,
}

impl ToolResult {
    /// Create a successful tool result.
    pub fn ok(tool: impl Into<String>, result: impl Into<String>) -> Self {
        Self {
            tool: tool.into(),
            result: result.into(),
            status: "ok".to_string(),
        }
    }

    /// Create an error tool result.
    pub fn error(tool: impl Into<String>, error: impl Into<String>) -> Self {
        let error = error.into();
        Self {
            tool: tool.into(),
            result: error.clone(),
            status: format!("error: {error}"),
        }
    }

    /// Create an "unknown tool" result for unrecognized tool names.
    pub fn unknown(tool: impl Into<String>) -> Self {
        let tool = tool.into();
        Self {
            tool: tool.clone(),
            result: String::new(),
            status: "unknown_tool".to_string(),
        }
    }
}

// ── Event protocol ──────────────────────────────────────────────────

/// Inbound lifecycle event from the host.
///
/// Events are fired at key moments in the agent lifecycle:
/// `agent_start`, `agent_end`, `tool_call`, `tool_result`,
/// `turn_start`, `turn_end`, `message_update`, `user_input`.
///
/// The `data` field carries event-specific context (may be empty `{}`).
#[derive(Debug, Clone, Deserialize)]
pub struct Event {
    pub event: String,
    #[serde(default)]
    pub data: serde_json::Value,
}

/// Outbound event response returned to the host.
///
/// `handled` indicates whether the plugin processed the event.
/// `message` is an optional human-readable note (logged/displayed by host).
#[derive(Debug, Clone, Serialize)]
pub struct EventResult {
    pub event: String,
    pub handled: bool,
    pub message: String,
}

impl EventResult {
    /// Create a "handled" event response.
    pub fn handled(event: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            event: event.into(),
            handled: true,
            message: message.into(),
        }
    }

    /// Create an "unhandled" event response (for events the plugin doesn't care about).
    pub fn unhandled(event: impl Into<String>) -> Self {
        let event = event.into();
        Self {
            message: format!("Unhandled event: {event}"),
            event,
            handled: false,
        }
    }
}

// ── Plugin metadata ─────────────────────────────────────────────────

/// Plugin metadata returned by the `describe` entrypoint.
///
/// This tells the host what the plugin provides. The host also reads
/// `plugin.json` for richer metadata (JSON schemas, permissions, etc),
/// but `describe` serves as a runtime self-description.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginMeta {
    pub name: String,
    pub version: String,
    pub tools: Vec<ToolMeta>,
    #[serde(default)]
    pub commands: Vec<String>,
}

/// Descriptor for a single tool exposed by the plugin.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolMeta {
    pub name: String,
    pub description: String,
}

impl PluginMeta {
    /// Build plugin metadata.
    ///
    /// `tools` is a slice of `(name, description)` pairs.
    /// `commands` is a slice of slash-command names the plugin provides.
    ///
    /// # Example
    /// ```ignore
    /// PluginMeta::new("my-plugin", "0.1.0", &[
    ///     ("my_tool", "Does something useful"),
    /// ], &[])
    /// ```
    pub fn new(name: impl Into<String>, version: impl Into<String>, tools: &[(&str, &str)], commands: &[&str]) -> Self {
        Self {
            name: name.into(),
            version: version.into(),
            tools: tools
                .iter()
                .map(|(n, d)| ToolMeta {
                    name: n.to_string(),
                    description: d.to_string(),
                })
                .collect(),
            commands: commands.iter().map(|s| s.to_string()).collect(),
        }
    }
}
