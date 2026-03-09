//! Shared types for the clankers TUI crate boundary.
//!
//! This crate contains types that flow between the TUI rendering layer and
//! the rest of the application (tools, modes, slash commands, config). It has
//! no dependency on ratatui or crossterm — purely data types.

pub mod actions;
pub mod block;
pub mod completion;
pub mod cost;
pub mod display;
pub mod menu;
pub mod panel;
pub mod plugin;
pub mod progress;
pub mod registry;
pub mod subagent;

// Re-export all public types at the crate root for convenience.
pub use actions::*;
pub use block::*;
pub use completion::*;
pub use cost::*;
pub use display::*;
pub use menu::*;
pub use panel::*;
pub use plugin::*;
pub use progress::*;
pub use registry::*;
pub use subagent::*;
