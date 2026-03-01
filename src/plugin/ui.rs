//! Declarative UI widget protocol for plugins

use serde::Deserialize;
use serde::Serialize;

/// Widget tree that plugins can send to the host for rendering
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum Widget {
    Text {
        content: String,
        #[serde(default)]
        bold: bool,
        #[serde(default)]
        color: Option<String>,
    },
    Box {
        children: Vec<Widget>,
        #[serde(default)]
        direction: Direction,
    },
    List {
        items: Vec<String>,
        #[serde(default)]
        selected: usize,
    },
    Input {
        value: String,
        #[serde(default)]
        placeholder: String,
    },
    Spacer {
        #[serde(default = "default_one")]
        lines: u16,
    },
    /// Progress bar (0.0 to 1.0)
    Progress {
        #[serde(default)]
        label: String,
        value: f64,
        #[serde(default)]
        color: Option<String>,
    },
    /// Key-value table
    Table {
        rows: Vec<Vec<String>>,
        #[serde(default)]
        headers: Vec<String>,
    },
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Direction {
    #[default]
    Vertical,
    Horizontal,
}

fn default_one() -> u16 {
    1
}

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

/// State for all plugin-contributed UI elements
#[derive(Debug, Default, Clone)]
pub struct PluginUIState {
    /// Widget panels keyed by plugin name
    pub widgets: std::collections::HashMap<String, Widget>,
    /// Status bar segments keyed by plugin name
    pub status_segments: std::collections::HashMap<String, StatusSegment>,
    /// Pending notifications to display
    pub notifications: Vec<PluginNotification>,
}

#[derive(Debug, Clone)]
pub struct StatusSegment {
    pub text: String,
    pub color: Option<String>,
}

#[derive(Debug, Clone)]
pub struct PluginNotification {
    pub plugin: String,
    pub message: String,
    pub level: String,
    pub created: std::time::Instant,
}

impl PluginUIState {
    pub fn new() -> Self {
        Self::default()
    }

    /// Apply a UI action from a plugin
    pub fn apply(&mut self, action: PluginUIAction) {
        match action {
            PluginUIAction::SetWidget { plugin, widget } => {
                self.widgets.insert(plugin, widget);
            }
            PluginUIAction::ClearWidget { plugin } => {
                self.widgets.remove(&plugin);
            }
            PluginUIAction::SetStatus { plugin, text, color } => {
                self.status_segments.insert(plugin, StatusSegment { text, color });
            }
            PluginUIAction::ClearStatus { plugin } => {
                self.status_segments.remove(&plugin);
            }
            PluginUIAction::Notify { plugin, message, level } => {
                self.notifications.push(PluginNotification {
                    plugin,
                    message,
                    level,
                    created: std::time::Instant::now(),
                });
            }
        }
    }

    /// Remove expired notifications (older than 5 seconds)
    pub fn gc_notifications(&mut self) {
        let ttl = std::time::Duration::from_secs(5);
        self.notifications.retain(|n| n.created.elapsed() < ttl);
    }

    /// Whether any plugin has active UI elements
    pub fn has_content(&self) -> bool {
        !self.widgets.is_empty() || !self.status_segments.is_empty()
    }
}
