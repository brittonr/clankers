#!/usr/bin/env -S nix develop -c cargo -q -Zscript
---cargo
[package]
edition = "2024"
---

use std::fs;

fn main() {
    let convert = fs::read_to_string("crates/clankers-controller/src/convert.rs")
        .expect("read controller event conversion");
    require(
        &convert,
        "daemon_event_translation_kit_preserves_streaming_replay_and_app_edge_events",
        "focused daemon event translation fixture",
    );
    require(&convert, "daemon_event_to_tui_event", "daemon to TUI translator");
    require(&convert, "agent_message_to_tui_events", "history replay translator");
    require(&convert, "token=[REDACTED]", "redacted app-edge fixture");
    require(&convert, "BranchSummary", "negative replay metadata fixture");

    let attach_events = fs::read_to_string("src/modes/attach/events.rs").expect("read attach event handling");
    require(&attach_events, "daemon_event_to_tui_event(event)", "attach uses shared translator");
    require(&attach_events, "HistoryBlock", "history replay app-edge path");
    require(&attach_events, "HistoryEnd", "history replay close marker");
    require(&attach_events, "SystemMessage", "app-edge system message handling");

    let docs = fs::read_to_string("docs/src/reference/daemon.md").expect("read daemon docs");
    require(&docs, "daemon-event-translation-kit", "documented daemon event translation kit");
    require(&docs, "streaming/replay", "documented streaming/replay boundary");
    require(&docs, "app-edge", "documented app-edge boundary");

    let spec = fs::read_to_string("openspec/specs/daemon-event-translation/spec.md")
        .expect("read promoted OpenSpec");
    require(&spec, "daemon-event-translation-kit", "promoted OpenSpec requirement");
    require(&spec, "streaming-replay", "OpenSpec streaming/replay scenario");
    require(&spec, "app-edge", "OpenSpec app-edge scenario");
}

fn require(haystack: &str, needle: &str, label: &str) {
    assert!(haystack.contains(needle), "missing {label}: {needle}");
}
