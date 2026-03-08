//! Common imports for TUI components.
//!
//! Instead of repeating the same 8–12 ratatui + crate imports in every
//! component file, add `use super::prelude::*;` at the top.

// Re-export ratatui essentials
pub use ratatui::Frame;
pub use ratatui::layout::Rect;
pub use ratatui::style::{Color, Modifier, Style};
pub use ratatui::text::{Line, Span};
pub use ratatui::widgets::{Block, Borders, Clear, Paragraph, Wrap};

// Re-export crate-local TUI plumbing
pub use crate::tui::panel::{DrawContext, Panel, PanelAction, PanelId};
pub use crate::tui::theme::Theme;
