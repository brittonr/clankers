//! Bridge between plugin system and agent events

#![allow(unexpected_cfgs)]
#![cfg_attr(dylint_lib = "tigerstyle", allow(compound_assertion, ignored_result, no_unwrap, no_panic, no_todo, unjustified_no_todo_allow, no_recursion, unchecked_narrowing, unchecked_division, unbounded_loop, catch_all_on_enum, explicit_defaults, unbounded_channel, unbounded_collection_growth, assertion_density, raw_arithmetic_overflow, sentinel_fallback, acronym_style, bool_naming, negated_predicate, numeric_units, float_for_currency, function_length, nested_conditionals, platform_dependent_cast, usize_in_public_api, too_many_parameters, compound_condition, unjustified_allow, ambiguous_params, ambient_clock, verified_purity, contradictory_time, multi_lock_ordering, reason = "full workspace tigerstyle audit gate: legacy debt documented locally while cleanup proceeds incrementally"))]

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
    /// A schedule fired — payload contains the schedule's action data.
    ScheduleFire,
}

impl PluginEvent {
    /// Parse from string (as used in plugin.json "events" array)
    // r[impl plugin.event.parse-matches-agree]
    // r[impl plugin.event.parse-complete]
    // r[impl plugin.event.unknown-rejects]
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
            "schedule_fire" => Some(Self::ScheduleFire),
            _ => None,
        }
    }

    /// Check if an event kind string matches this plugin event type.
    // r[impl plugin.event.parse-matches-agree]
    pub fn matches_event_kind(&self, kind: &str) -> bool {
        matches!(
            (self, kind),
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
                | (Self::SessionCompaction, "session_compaction")
                | (Self::ScheduleFire, "schedule_fire")
        )
    }
}

/// Parse UI actions from a plugin's event handler response.
/// The response JSON may contain a `"ui"` key with one or more UI actions.
pub fn parse_ui_actions(plugin_name: &str, response: &serde_json::Value) -> Vec<super::ui::PluginUiAction> {
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
                obj.insert("plugin".to_string(), serde_json::Value::String(plugin_name.to_string()));
            }
            if let Ok(action) = serde_json::from_value::<super::ui::PluginUiAction>(item) {
                actions.push(action);
            }
        }
    }

    actions
}
