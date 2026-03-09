//! Declarative UI widget protocol for plugins

// Widget types re-exported from clankers-tui-types (canonical definitions).
pub use clankers_tui_types::Direction;
pub use clankers_tui_types::PluginNotification;
pub use clankers_tui_types::PluginUIState;
pub use clankers_tui_types::StatusSegment;
pub use clankers_tui_types::Widget;
use serde::Deserialize;
use serde::Serialize;

/// Actions that a plugin's event handler can return to modify the UI.
/// Parsed from the JSON response of `on_event` / `on_ui_event`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "action")]
pub enum PluginUIAction {
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
pub fn apply_ui_action(state: &mut PluginUIState, action: PluginUIAction) {
    match action {
        PluginUIAction::SetWidget { plugin, widget } => {
            state.widgets.insert(plugin, widget);
        }
        PluginUIAction::ClearWidget { plugin } => {
            state.widgets.remove(&plugin);
        }
        PluginUIAction::SetStatus { plugin, text, color } => {
            state.status_segments.insert(plugin, StatusSegment { text, color });
        }
        PluginUIAction::ClearStatus { plugin } => {
            state.status_segments.remove(&plugin);
        }
        PluginUIAction::Notify { plugin, message, level } => {
            state.notifications.push(PluginNotification {
                plugin,
                message,
                level,
                created: std::time::Instant::now(),
            });
        }
    }
}
