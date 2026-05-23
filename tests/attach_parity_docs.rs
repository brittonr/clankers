const ATTACH: &str = include_str!("../src/modes/attach.rs");
const ATTACH_COMMANDS: &str = include_str!("../src/modes/attach/commands.rs");
const ATTACH_REMOTE: &str = include_str!("../src/modes/attach_remote.rs");
const SESSION_COMMAND_POLICY: &str = include_str!("../src/modes/session_command_policy.rs");
const REQUEST_LIFECYCLE: &str = include_str!("../docs/src/reference/request-lifecycle.md");

#[test]
fn local_and_remote_attach_thread_the_same_parity_tracker() {
    for anchor in [
        "pub(crate) struct AttachParityTracker",
        "fn should_suppress(&mut self, event: &DaemonEvent) -> bool",
        "fn is_thinking_ack_message(event: &DaemonEvent) -> bool",
        "session_command_policy::ack_matches(SessionAckPolicy::ThinkingLevel, event)",
        "session_command_policy::ack_matches(SessionAckPolicy::DisabledTools, event)",
        "expect_thinking_ack_message",
        "expect_disabled_tools_message",
    ] {
        assert!(ATTACH_COMMANDS.contains(anchor), "attach command module missing parity anchor `{anchor}`");
    }

    assert!(ATTACH.contains("pub(crate) use commands::AttachParityTracker;"));

    for anchor in [
        "pub(crate) fn ack_matches(policy: SessionAckPolicy, event: &DaemonEvent) -> bool",
        "text.starts_with(\"Thinking\")",
        "text.starts_with(\"Disabled tools updated:\")",
    ] {
        assert!(SESSION_COMMAND_POLICY.contains(anchor), "session command policy missing parity anchor `{anchor}`");
    }

    for anchor in [
        "use super::attach::AttachParityTracker;",
        "let mut parity_tracker = AttachParityTracker::default();",
        "drain_daemon_events(app, &mut client, &mut is_replaying_history, max_subagent_panes, &mut parity_tracker)",
        "handle_terminal_events(app, &mut client, terminal, &keymap, slash_registry, &mut parity_tracker)",
        "*parity_tracker = AttachParityTracker::default();",
    ] {
        assert!(ATTACH_REMOTE.contains(anchor), "attach_remote.rs missing parity anchor `{anchor}`");
    }
}

#[test]
fn thinking_slash_bridges_explicit_and_cycle_paths_before_suppressing_daemon_ack() {
    for anchor in [
        "AgentCommand::SetThinkingLevel(level)",
        "session_command_policy::set_thinking_level_effect(level)",
        "AgentCommand::CycleThinkingLevel",
        "session_command_policy::cycle_thinking_level_effect(app.thinking_level)",
        "apply_local_session_effect(app, effect.local);",
        "parity_tracker.expect_ack(effect.ack);",
    ] {
        assert!(ATTACH_COMMANDS.contains(anchor), "attach thinking parity anchor missing `{anchor}`");
    }

    for anchor in ["SessionCommand::SetThinkingLevel", "SessionCommand::CycleThinkingLevel"] {
        assert!(
            SESSION_COMMAND_POLICY.contains(anchor),
            "session command policy missing thinking command anchor `{anchor}`"
        );
    }
}

#[test]
fn disabled_tools_attach_bridge_applies_local_state_before_ack_suppression() {
    let local_apply = ATTACH
        .find("apply_standalone_disabled_tools(app, app.overlays.tool_toggle.disabled_set())")
        .expect("attach should apply standalone disabled-tools state before forwarding");
    let expect_suppression = ATTACH
        .find("parity_tracker.expect_disabled_tools_message();")
        .expect("attach should budget daemon disabled-tools ack suppression");
    let forward = ATTACH
        .find("client.send(SessionCommand::SetDisabledTools { tools: disabled });")
        .expect("attach should forward SetDisabledTools to daemon");

    assert!(
        local_apply < expect_suppression && expect_suppression < forward,
        "attach should apply disabled-tools state, then budget suppression, then forward daemon command"
    );
}

#[test]
fn request_lifecycle_doc_keeps_attach_parity_warning() {
    for phrase in [
        "Slash command and attach parity",
        "suppress only the matching daemon acknowledgement",
        "Keep suppression narrow",
        "Update local and remote attach code together",
    ] {
        assert!(REQUEST_LIFECYCLE.contains(phrase), "request lifecycle doc missing attach parity phrase `{phrase}`");
    }
}
