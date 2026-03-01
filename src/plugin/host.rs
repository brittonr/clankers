//! Host functions exposed to WASM plugins
//!
//! Plugins interact with the host by returning structured JSON from their
//! `on_event` handler. The response can include a `"ui"` key containing
//! UI actions that the host applies to the TUI.
//!
//! ## UI Protocol
//!
//! A plugin's `on_event` handler returns JSON like:
//!
//! ```json
//! {
//!   "handled": true,
//!   "message": "optional chat message",
//!   "display": true,
//!   "ui": [
//!     {"action": "set_widget", "widget": {"type": "Text", "content": "Hello", "bold": true}},
//!     {"action": "set_status", "text": "running", "color": "green"},
//!     {"action": "notify", "message": "Build complete!", "level": "info"}
//!   ]
//! }
//! ```
//!
//! ### Available UI Actions
//!
//! - `set_widget` — Set or replace the plugin's widget panel in the TUI
//! - `clear_widget` — Remove the plugin's widget panel
//! - `set_status` — Set the plugin's status bar segment
//! - `clear_status` — Remove the plugin's status bar segment
//! - `notify` — Show a toast notification (level: info/warning/error)
//!
//! ### Available Widget Types
//!
//! - `Text` — Styled text (`content`, `bold`, `color`)
//! - `Box` — Container with children (`children`, `direction`: vertical/horizontal)
//! - `List` — Selectable list (`items`, `selected`)
//! - `Input` — Text input display (`value`, `placeholder`)
//! - `Spacer` — Vertical space (`lines`)
//! - `Progress` — Progress bar (`label`, `value` 0.0–1.0, `color`)
//! - `Table` — Data table (`rows`, `headers`)

/// Host function registry
/// Maps function names to their implementations.
/// Plugins call these via the event response protocol.
pub struct HostFunctions {
    _private: (),
}

impl HostFunctions {
    pub fn new() -> Self {
        Self { _private: () }
    }

    /// List of host functions available to plugins via the UI action protocol
    pub fn available_functions() -> Vec<&'static str> {
        vec!["set_widget", "clear_widget", "set_status", "clear_status", "notify"]
    }
}

impl Default for HostFunctions {
    fn default() -> Self {
        Self::new()
    }
}
