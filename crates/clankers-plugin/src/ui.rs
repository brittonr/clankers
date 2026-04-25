//! Declarative UI widget protocol for plugins

#![allow(unexpected_cfgs)]
#![cfg_attr(dylint_lib = "tigerstyle", allow(compound_assertion, ignored_result, no_unwrap, no_panic, no_todo, unjustified_no_todo_allow, no_recursion, unchecked_narrowing, unchecked_division, unbounded_loop, catch_all_on_enum, explicit_defaults, unbounded_channel, unbounded_collection_growth, assertion_density, raw_arithmetic_overflow, sentinel_fallback, acronym_style, bool_naming, negated_predicate, numeric_units, float_for_currency, function_length, nested_conditionals, platform_dependent_cast, usize_in_public_api, too_many_parameters, compound_condition, unjustified_allow, ambiguous_params, ambient_clock, verified_purity, contradictory_time, multi_lock_ordering, reason = "full workspace tigerstyle audit gate: legacy debt documented locally while cleanup proceeds incrementally"))]

// Widget types re-exported from clanker-tui-types (canonical definitions).
pub use clanker_tui_types::Direction;
pub use clanker_tui_types::PluginNotification;
pub use clanker_tui_types::PluginUiState;
pub use clanker_tui_types::StatusSegment;
pub use clanker_tui_types::Widget;
use serde::Deserialize;
use serde::Serialize;

/// Actions that a plugin's event handler can return to modify the UI.
/// Parsed from the JSON response of `on_event` / `on_ui_event`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "action")]
pub enum PluginUiAction {
    /// Set or replace the plugin's persistent widget panel
    #[serde(rename = "set_widget")]
    SetWidget { plugin: String, widget: Widget },
    /// Remove the plugin's widget panel
    #[serde(rename = "clear_widget")]
    ClearWidget { plugin: String },
    /// Set the plugin's status bar segment
    #[serde(rename = "set_status")]
    SetStatus {
        plugin: String,
        text: String,
        #[serde(default)]
        color: Option<String>,
    },
    /// Clear the plugin's status bar segment
    #[serde(rename = "clear_status")]
    ClearStatus { plugin: String },
    /// Show a toast notification
    #[serde(rename = "notify")]
    Notify {
        plugin: String,
        message: String,
        #[serde(default = "default_info")]
        level: String,
    },
}

fn default_info() -> String {
    "info".to_string()
}

/// Apply a UI action from a plugin to the shared state.
pub fn apply_ui_action(state: &mut PluginUiState, action: PluginUiAction) {
    match action {
        PluginUiAction::SetWidget { plugin, widget } => {
            state.widgets.insert(plugin, widget);
        }
        PluginUiAction::ClearWidget { plugin } => {
            state.widgets.remove(&plugin);
        }
        PluginUiAction::SetStatus { plugin, text, color } => {
            state.status_segments.insert(plugin, StatusSegment { text, color });
        }
        PluginUiAction::ClearStatus { plugin } => {
            state.status_segments.remove(&plugin);
        }
        PluginUiAction::Notify { plugin, message, level } => {
            state.notifications.push(PluginNotification {
                plugin,
                message,
                level,
                created: std::time::Instant::now(),
            });
        }
    }
}
