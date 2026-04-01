//! Theme types and color definitions

use ratatui::style::Color;

/// TUI theme
#[derive(Debug, Clone, PartialEq)]
pub struct Theme {
    pub bg: Color,
    pub fg: Color,
    pub border: Color,
    pub highlight: Color,
    pub user_msg: Color,
    pub assistant_msg: Color,
    pub system_msg: Color,
    pub thinking_msg: Color,
    pub error: Color,
    /// Block border (normal)
    pub block_border: Color,
    /// Block border (focused / active)
    pub block_border_focused: Color,
    /// Block timestamp color
    pub block_timestamp: Color,

    // ── Markdown rendering colors ────────────────────
    /// Fenced code block content
    pub md_code_block: Color,
    /// Code fence / language label
    pub md_code_fence: Color,
    /// Inline `code` foreground
    pub md_inline_code_fg: Color,
    /// Inline `code` background
    pub md_inline_code_bg: Color,
    /// List bullet / number
    pub md_list_marker: Color,
    /// Blockquote text
    pub md_blockquote: Color,
    /// Horizontal rule
    pub md_hrule: Color,

    // ── Search highlight colors ──────────────────────
    /// Background for search matches (all except current)
    pub search_match: Color,
    /// Background for the current/active search match
    pub search_current: Color,
}

impl Theme {
    /// Light theme for terminals with light backgrounds.
    pub fn light() -> Self {
        Self {
            bg: Color::Rgb(255, 255, 255),
            fg: Color::Rgb(30, 30, 30),
            border: Color::Rgb(200, 200, 200),
            highlight: Color::Rgb(0, 120, 215),
            user_msg: Color::Rgb(0, 128, 0),
            assistant_msg: Color::Rgb(40, 40, 40),
            system_msg: Color::Rgb(120, 120, 120),
            thinking_msg: Color::Rgb(100, 80, 160),
            error: Color::Rgb(200, 0, 0),
            block_border: Color::Rgb(220, 220, 220),
            block_border_focused: Color::Rgb(0, 120, 215),
            block_timestamp: Color::Rgb(160, 160, 160),

            md_code_block: Color::Rgb(60, 120, 40),
            md_code_fence: Color::Rgb(160, 160, 160),
            md_inline_code_fg: Color::Rgb(180, 100, 0),
            md_inline_code_bg: Color::Rgb(240, 240, 240),
            md_list_marker: Color::Rgb(160, 160, 160),
            md_blockquote: Color::Rgb(100, 100, 100),
            md_hrule: Color::Rgb(200, 200, 200),

            search_match: Color::Rgb(255, 230, 140),
            search_current: Color::Rgb(255, 165, 0),
        }
    }

    /// Dark theme (default)
    pub fn dark() -> Self {
        Self {
            bg: Color::Rgb(30, 30, 30),
            fg: Color::Rgb(220, 220, 220),
            border: Color::Rgb(80, 80, 80),
            highlight: Color::Rgb(100, 180, 255),
            user_msg: Color::Rgb(120, 200, 120),
            assistant_msg: Color::Rgb(200, 200, 200),
            system_msg: Color::Rgb(150, 150, 150),
            thinking_msg: Color::Rgb(150, 130, 200),
            error: Color::Rgb(255, 100, 100),
            block_border: Color::Rgb(60, 60, 60),
            block_border_focused: Color::Rgb(100, 180, 255),
            block_timestamp: Color::Rgb(100, 100, 100),

            md_code_block: Color::Rgb(180, 220, 140),
            md_code_fence: Color::Rgb(100, 100, 100),
            md_inline_code_fg: Color::Rgb(230, 190, 80),
            md_inline_code_bg: Color::Rgb(45, 45, 45),
            md_list_marker: Color::Rgb(100, 100, 100),
            md_blockquote: Color::Rgb(160, 160, 160),
            md_hrule: Color::Rgb(80, 80, 80),

            search_match: Color::Rgb(120, 90, 30),
            search_current: Color::Rgb(220, 180, 40),
        }
    }
}
