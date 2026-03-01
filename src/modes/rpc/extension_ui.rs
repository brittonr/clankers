//! Extension UI sub-protocol for RPC
//!
//! Allows RPC clients (like IDE extensions) to send/receive
//! UI widget updates.

use serde::Deserialize;
use serde::Serialize;

use crate::plugin::ui::Widget;

/// UI update from host to client
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UiUpdate {
    pub plugin: String,
    pub widget: Widget,
}

/// UI event from client to host
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UiEvent {
    pub plugin: String,
    pub event_type: String,
    pub data: serde_json::Value,
}
