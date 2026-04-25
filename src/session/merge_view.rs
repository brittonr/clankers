//! Convert session entries to TUI merge views.
//!
//! Lives in the main crate because it bridges `clankers-session` types
//! and `clanker-tui-types` — neither should depend on the other.

#![allow(unexpected_cfgs)]
#![cfg_attr(dylint_lib = "tigerstyle", allow(compound_assertion, ignored_result, no_unwrap, no_panic, no_todo, unjustified_no_todo_allow, no_recursion, unchecked_narrowing, unchecked_division, unbounded_loop, catch_all_on_enum, explicit_defaults, unbounded_channel, unbounded_collection_growth, assertion_density, raw_arithmetic_overflow, sentinel_fallback, acronym_style, bool_naming, negated_predicate, numeric_units, float_for_currency, function_length, nested_conditionals, platform_dependent_cast, usize_in_public_api, too_many_parameters, compound_condition, unjustified_allow, ambiguous_params, ambient_clock, verified_purity, contradictory_time, multi_lock_ordering, reason = "full workspace tigerstyle audit gate: legacy debt documented locally while cleanup proceeds incrementally"))]

use clanker_message::AgentMessage;
use clanker_message::Content;

use crate::session::entry::MessageEntry;

/// Convert a `MessageEntry` into a `MergeMessageView` for the TUI merge overlay.
pub fn to_merge_view(entry: &MessageEntry) -> clanker_tui_types::MergeMessageView {
    fn content_text(content: &[Content]) -> String {
        content
            .iter()
            .filter_map(|c| match c {
                Content::Text { text } => Some(text.as_str()),
                Content::Thinking { thinking, .. } => Some(thinking.as_str()),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join(" ")
    }

    fn truncate(text: &str, max: usize) -> String {
        let first_line = text.lines().next().unwrap_or(text).trim();
        if first_line.chars().count() > max {
            let preview: String = first_line.chars().take(max).collect();
            format!("{}…", preview)
        } else {
            first_line.to_string()
        }
    }

    let (preview, variant_label) = match &entry.message {
        AgentMessage::User(m) => (truncate(&content_text(&m.content), 70), "User"),
        AgentMessage::Assistant(m) => (truncate(&content_text(&m.content), 70), "Assistant"),
        AgentMessage::ToolResult(m) => {
            let text = content_text(&m.content);
            let p = if text.is_empty() {
                format!("[{}]", m.tool_name)
            } else {
                format!("[{}] {}", m.tool_name, text)
            };
            (truncate(&p, 70), "Tool")
        }
        AgentMessage::BashExecution(m) => (truncate(&format!("$ {}", m.command), 70), "Bash"),
        AgentMessage::Custom(m) => (truncate(&format!("[{}]", m.kind), 70), "Custom"),
        AgentMessage::BranchSummary(m) => (truncate(&m.summary, 70), "Branch"),
        AgentMessage::CompactionSummary(m) => (truncate(&m.summary, 70), "Compact"),
    };

    clanker_tui_types::MergeMessageView {
        id: entry.id.to_string(),
        preview,
        variant_label,
    }
}
