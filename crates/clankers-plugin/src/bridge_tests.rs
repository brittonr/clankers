//! Tests for plugin event parse/dispatch consistency.

use super::bridge::*;

// ── parse + matches_event_kind agreement ────────────────────

// r[verify plugin.event.parse-matches-agree]
#[test]
fn parse_and_matches_agree_for_all_dispatchable() {
    let dispatchable = [
        "tool_call",
        "tool_result",
        "tool_execution_start",
        "agent_start",
        "agent_end",
        "turn_start",
        "turn_end",
        "message_update",
        "user_input",
        "user_cancel",
        "session_start",
        "session_end",
        "model_change",
        "usage_update",
        "session_branch",
        "session_compaction",
    ];

    for kind in dispatchable {
        let event = PluginEvent::parse(kind)
            .unwrap_or_else(|| panic!("parse({kind}) should succeed"));
        assert!(
            event.matches_event_kind(kind),
            "parse({kind}) returned {event:?} but matches_event_kind({kind}) is false"
        );
    }
}

#[test]
fn plugin_init_parses_but_does_not_match() {
    // PluginInit is only parsed (for init dispatch), not matched in event loop
    let event = PluginEvent::parse("plugin_init").unwrap();
    assert_eq!(event, PluginEvent::PluginInit);
    assert!(!event.matches_event_kind("plugin_init"));
}

// ── parse completeness ──────────────────────────────────────

// r[verify plugin.event.parse-complete]
#[test]
fn every_variant_is_reachable_via_parse() {
    let all_kinds = [
        ("plugin_init", PluginEvent::PluginInit),
        ("tool_call", PluginEvent::ToolCall),
        ("tool_result", PluginEvent::ToolResult),
        ("tool_execution_start", PluginEvent::ToolExecutionStart),
        ("agent_start", PluginEvent::AgentStart),
        ("agent_end", PluginEvent::AgentEnd),
        ("turn_start", PluginEvent::TurnStart),
        ("turn_end", PluginEvent::TurnEnd),
        ("message_update", PluginEvent::MessageUpdate),
        ("user_input", PluginEvent::UserInput),
        ("user_cancel", PluginEvent::UserCancel),
        ("session_start", PluginEvent::SessionStart),
        ("session_end", PluginEvent::SessionEnd),
        ("model_change", PluginEvent::ModelChange),
        ("usage_update", PluginEvent::UsageUpdate),
        ("session_branch", PluginEvent::SessionBranch),
        ("session_compaction", PluginEvent::SessionCompaction),
    ];

    for (kind_str, expected_variant) in all_kinds {
        let parsed = PluginEvent::parse(kind_str)
            .unwrap_or_else(|| panic!("parse({kind_str}) returned None"));
        assert_eq!(
            parsed, expected_variant,
            "parse({kind_str}) returned {parsed:?}, expected {expected_variant:?}"
        );
    }
}

// ── unknown rejection ───────────────────────────────────────

// r[verify plugin.event.unknown-rejects]
#[test]
fn unknown_event_strings_rejected() {
    assert!(PluginEvent::parse("").is_none());
    assert!(PluginEvent::parse("nonexistent").is_none());
    assert!(PluginEvent::parse("TOOL_CALL").is_none()); // case sensitive
    assert!(PluginEvent::parse("tool-call").is_none()); // dash vs underscore
    assert!(PluginEvent::parse("plugin_init ").is_none()); // trailing space
}
