//! Bridge between plugin system and agent events

/// Events that plugins can subscribe to
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PluginEvent {
    /// Fired once when the plugin is loaded and the TUI is ready
    PluginInit,
    ToolCall,
    ToolResult,
    ToolExecutionStart,
    AgentStart,
    AgentEnd,
    TurnStart,
    TurnEnd,
    MessageUpdate,
    UserInput,
    UserCancel,
    SessionStart,
    SessionEnd,
    ModelChange,
    UsageUpdate,
    SessionBranch,
    SessionCompaction,
}

impl PluginEvent {
    /// Parse from string (as used in plugin.json "events" array)
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "plugin_init" => Some(Self::PluginInit),
            "tool_call" => Some(Self::ToolCall),
            "tool_result" => Some(Self::ToolResult),
            "tool_execution_start" => Some(Self::ToolExecutionStart),
            "agent_start" => Some(Self::AgentStart),
            "agent_end" => Some(Self::AgentEnd),
            "turn_start" => Some(Self::TurnStart),
            "turn_end" => Some(Self::TurnEnd),
            "message_update" => Some(Self::MessageUpdate),
            "user_input" => Some(Self::UserInput),
            "user_cancel" => Some(Self::UserCancel),
            "session_start" => Some(Self::SessionStart),
            "session_end" => Some(Self::SessionEnd),
            "model_change" => Some(Self::ModelChange),
            "usage_update" => Some(Self::UsageUpdate),
            "session_branch" => Some(Self::SessionBranch),
            "session_compaction" => Some(Self::SessionCompaction),
            _ => None,
        }
    }

    /// Check if an event kind string matches this plugin event type.
    pub fn matches_event_kind(&self, kind: &str) -> bool {
        match (self, kind) {
            (Self::ToolCall, "tool_call")
            | (Self::ToolResult, "tool_result")
            | (Self::ToolExecutionStart, "tool_execution_start")
            | (Self::AgentStart, "agent_start")
            | (Self::AgentEnd, "agent_end")
            | (Self::TurnStart, "turn_start")
            | (Self::TurnEnd, "turn_end")
            | (Self::MessageUpdate, "message_update")
            | (Self::UserInput, "user_input")
            | (Self::UserCancel, "user_cancel")
            | (Self::SessionStart, "session_start")
            | (Self::SessionEnd, "session_end")
            | (Self::ModelChange, "model_change")
            | (Self::UsageUpdate, "usage_update")
            | (Self::SessionBranch, "session_branch")
            | (Self::SessionCompaction, "session_compaction") => true,
            _ => false,
        }
    }
}

/// Parse UI actions from a plugin's event handler response.
/// The response JSON may contain a `"ui"` key with one or more UI actions.
pub fn parse_ui_actions(
    plugin_name: &str,
    response: &serde_json::Value,
) -> Vec<super::ui::PluginUIAction> {
    let mut actions = Vec::new();

    // Check for "ui" key — can be a single action object or an array
    if let Some(ui_val) = response.get("ui") {
        let items = if ui_val.is_array() {
            ui_val.as_array().cloned().unwrap_or_default()
        } else {
            vec![ui_val.clone()]
        };

        for mut item in items {
            // Inject the plugin name if not already set
            if item.get("plugin").is_none()
                && let Some(obj) = item.as_object_mut()
            {
                obj.insert(
                    "plugin".to_string(),
                    serde_json::Value::String(plugin_name.to_string()),
                );
            }
            if let Ok(action) = serde_json::from_value::<super::ui::PluginUIAction>(item) {
                actions.push(action);
            }
        }
    }

    actions
}
