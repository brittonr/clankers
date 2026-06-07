//! Plugin UI widget types — declarative widget protocol for plugins.
//!
//! The canonical DTOs live in `clanker-message`; this module re-exports them
//! for display-edge compatibility.

pub use clanker_message::Direction;
pub use clanker_message::PluginNotification;
pub use clanker_message::PluginSummary;
pub use clanker_message::PluginUiState;
pub use clanker_message::StatusSegment;
pub use clanker_message::Widget;
