//! Thin wrapper around `rat-markdown` for theme integration.
//!
//! The parser/renderer lives in `rat_markdown`. This module re-exports
//! its public API and adds a `markdown_style_from_theme` constructor
//! that bridges `rat_markdown::MarkdownStyle` with the TUI's `Theme`.

#![allow(unexpected_cfgs)]
#![cfg_attr(
    dylint_lib = "tigerstyle",
    allow(
        compound_assertion,
        ignored_result,
        no_unwrap,
        no_panic,
        no_todo,
        unjustified_no_todo_allow,
        no_recursion,
        unchecked_narrowing,
        unchecked_division,
        unbounded_loop,
        catch_all_on_enum,
        explicit_defaults,
        unbounded_channel,
        unbounded_collection_growth,
        assertion_density,
        raw_arithmetic_overflow,
        sentinel_fallback,
        acronym_style,
        bool_naming,
        negated_predicate,
        numeric_units,
        float_for_currency,
        function_length,
        nested_conditionals,
        platform_dependent_cast,
        usize_in_public_api,
        too_many_parameters,
        compound_condition,
        unjustified_allow,
        ambiguous_params,
        ambient_clock,
        verified_purity,
        contradictory_time,
        multi_lock_ordering,
        reason = "full workspace tigerstyle audit gate: legacy debt documented locally while cleanup proceeds incrementally"
    )
)]

pub use rat_markdown::MarkdownStyle;
pub use rat_markdown::render_markdown;
use ratatui::style::Modifier;
use ratatui::style::Style;

use crate::theme::Theme;

/// Build a `MarkdownStyle` from a TUI `Theme` and base text style.
pub fn markdown_style_from_theme(theme: &Theme, base: Style) -> MarkdownStyle {
    MarkdownStyle {
        base,
        code_block: Style::default().fg(theme.md_code_block),
        code_fence: Style::default().fg(theme.md_code_fence),
        inline_code: Style::default().fg(theme.md_inline_code_fg).bg(theme.md_inline_code_bg),
        bold: base.add_modifier(Modifier::BOLD),
        italic: base.add_modifier(Modifier::ITALIC),
        bold_italic: base.add_modifier(Modifier::BOLD | Modifier::ITALIC),
        heading: base.add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
        subheading: base.add_modifier(Modifier::BOLD),
        list_marker: Style::default().fg(theme.md_list_marker),
        blockquote: Style::default().fg(theme.md_blockquote).add_modifier(Modifier::ITALIC),
        hrule: Style::default().fg(theme.md_hrule),
    }
}
