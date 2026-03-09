//! Common imports for TUI components.
//!
//! Instead of repeating the same 8–12 ratatui + crate imports in every
//! component file, add `use super::prelude::*;` at the top.

// Re-export ratatui essentials
pub use ratatui::Frame;
pub use ratatui::layout::Rect;
pub use ratatui::style::Color;
pub use ratatui::style::Modifier;
pub use ratatui::style::Style;
pub use ratatui::text::Line;
pub use ratatui::text::Span;
pub use ratatui::widgets::Block;
pub use ratatui::widgets::Borders;
pub use ratatui::widgets::Clear;
pub use ratatui::widgets::Paragraph;
pub use ratatui::widgets::Wrap;

// Re-export crate-local TUI plumbing
pub use crate::tui::panel::{DrawContext, Panel, PanelAction, PanelId};
pub use crate::tui::theme::Theme;
