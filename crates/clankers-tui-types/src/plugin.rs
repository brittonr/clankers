//! Plugin UI widget types — declarative widget protocol for plugins.

use std::collections::HashMap;

use serde::Deserialize;
use serde::Serialize;

/// Widget tree that plugins can send to the host for rendering.
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
    /// Progress bar (0.0 to 1.0).
    Progress {
        #[serde(default)]
        label: String,
        value: f64,
        #[serde(default)]
        color: Option<String>,
    },
    /// Key-value table.
    Table {
        rows: Vec<Vec<String>>,
        #[serde(default)]
        headers: Vec<String>,
    },
}

/// Layout direction for Box widgets.
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

/// State for all plugin-contributed UI elements.
#[derive(Debug, Default, Clone)]
pub struct PluginUIState {
    /// Widget panels keyed by plugin name.
    pub widgets: HashMap<String, Widget>,
    /// Status bar segments keyed by plugin name.
    pub status_segments: HashMap<String, StatusSegment>,
    /// Pending notifications to display.
    pub notifications: Vec<PluginNotification>,
}

impl PluginUIState {
    pub fn new() -> Self {
        Self::default()
    }

    /// Remove expired notifications (older than 5 seconds).
    pub fn gc_notifications(&mut self) {
        let ttl = std::time::Duration::from_secs(5);
        self.notifications.retain(|n| n.created.elapsed() < ttl);
    }

    /// Whether any plugin has active UI elements.
    pub fn has_content(&self) -> bool {
        !self.widgets.is_empty() || !self.status_segments.is_empty()
    }
}

/// A plugin's status bar segment.
#[derive(Debug, Clone)]
pub struct StatusSegment {
    pub text: String,
    pub color: Option<String>,
}

/// A plugin notification (toast).
#[derive(Debug, Clone)]
pub struct PluginNotification {
    pub plugin: String,
    pub message: String,
    pub level: String,
    pub created: std::time::Instant,
}
