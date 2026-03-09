//! Shared types for the clankers TUI crate boundary.
//!
//! This crate contains types that flow between the TUI rendering layer and
//! the rest of the application (tools, modes, slash commands, config). It has
//! no dependency on ratatui or crossterm — purely data types.

pub mod actions;
pub mod block;
pub mod completion;
pub mod cost;
pub mod events;
pub mod display;
pub mod menu;
pub mod merge;
pub mod panel;
pub mod peers;
pub mod plugin;
pub mod process;
pub mod progress;
pub mod registry;
pub mod subagent;
pub mod syntax;

// Re-export all public types at the crate root for convenience.
pub use actions::*;
pub use block::*;
pub use completion::*;
pub use cost::*;
pub use display::*;
pub use events::*;
pub use menu::*;
pub use merge::*;
pub use panel::*;
pub use peers::*;
pub use plugin::*;
pub use process::*;
pub use progress::*;
pub use registry::*;
pub use subagent::*;
pub use syntax::*;
