#!/usr/bin/env -S nix develop -c cargo -q -Zscript
---cargo
[package]
edition = "2024"
---

use std::fs;

fn main() {
    let convert = fs::read_to_string("crates/clankers-controller/src/convert.rs")
        .expect("read controller event conversion");
    require(&convert, "semantic_event_to_daemon_event", "controller daemon projection owner");
    assert!(
        !convert.contains("clanker_tui_types") && !convert.contains("daemon_event_to_tui_event"),
        "controller conversion must not own display/TUI DTO projection"
    );

    let attach_projection = fs::read_to_string("src/modes/attach/event_projection.rs")
        .expect("read attach display projection");
    require(
        &attach_projection,
        "daemon_event_to_tui_projects_streaming_and_replay_events",
        "focused daemon event translation fixture",
    );
    require(&attach_projection, "daemon_event_to_tui_event", "daemon to TUI translator");
    require(&attach_projection, "agent_message_to_tui_events", "history replay translator");
    require(&attach_projection, "token=[REDACTED]", "redacted app-edge fixture");
    require(&attach_projection, "BranchSummary", "negative replay metadata fixture");

    let attach_events = fs::read_to_string("src/modes/attach/events.rs").expect("read attach event handling");
    require(&attach_events, "daemon_event_to_tui_event(event)", "attach uses shared translator");
    require(&attach_events, "HistoryBlock", "history replay app-edge path");
    require(&attach_events, "HistoryEnd", "history replay close marker");
    require(&attach_events, "SystemMessage", "app-edge system message handling");

    let docs = fs::read_to_string("docs/src/reference/daemon.md").expect("read daemon docs");
    require(&docs, "daemon-event-translation-kit", "documented daemon event translation kit");
    require(&docs, "streaming/replay", "documented streaming/replay boundary");
    require(&docs, "app-edge", "documented app-edge boundary");

    let spec = fs::read_to_string("cairn/specs/daemon-event-translation/spec.md")
        .expect("read promoted Cairn");
    require(&spec, "daemon-event-translation-kit", "promoted Cairn requirement");
    require(&spec, "streaming-replay", "Cairn streaming/replay scenario");
    require(&spec, "app-edge", "Cairn app-edge scenario");
}

fn require(haystack: &str, needle: &str, label: &str) {
    assert!(haystack.contains(needle), "missing {label}: {needle}");
}
