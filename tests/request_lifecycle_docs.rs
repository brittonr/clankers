use std::path::Path;

const README: &str = include_str!("../README.md");
const SUMMARY: &str = include_str!("../docs/src/SUMMARY.md");
const DAEMON_DOC: &str = include_str!("../docs/src/reference/daemon.md");
const GENERATED_ARCHITECTURE: &str = include_str!("../docs/src/generated/architecture.md");
const REQUEST_LIFECYCLE: &str = include_str!("../docs/src/reference/request-lifecycle.md");
const CONTROLLER_EVENT_PROCESSING: &str = include_str!("../crates/clankers-controller/src/event_processing.rs");
const CONTROLLER_COMMAND: &str = include_str!("../crates/clankers-controller/src/command.rs");
const CONTROLLER_PERSISTENCE: &str = include_str!("../crates/clankers-controller/src/persistence.rs");
const AGENT_EXECUTION: &str = include_str!("../crates/clankers-agent/src/turn/execution.rs");
const STANDALONE_EVENT_LOOP: &str = include_str!("../src/modes/event_loop_runner/mod.rs");
const DAEMON_AGENT_PROCESS: &str = include_str!("../src/modes/daemon/agent_process.rs");
const DAEMON_SOCKET_BRIDGE: &str = include_str!("../src/modes/daemon/socket_bridge.rs");

#[test]
fn request_lifecycle_doc_is_discoverable_from_top_level_docs() {
    assert!(
        README.contains("docs/src/reference/request-lifecycle.md"),
        "README architecture section should point contributors to the request lifecycle doc"
    );
    assert!(
        SUMMARY.contains("[Request Lifecycle](./reference/request-lifecycle.md)"),
        "docs/src/SUMMARY.md must link the request lifecycle golden-path doc"
    );
    assert!(
        DAEMON_DOC.contains("[Request Lifecycle](./request-lifecycle.md)"),
        "daemon reference should link to the request lifecycle golden-path doc"
    );
    assert!(
        GENERATED_ARCHITECTURE.contains("[Request Lifecycle](../reference/request-lifecycle.md)"),
        "generated architecture map should link to the request lifecycle golden-path doc"
    );
    assert!(
        Path::new("docs/src/reference/request-lifecycle.md").exists(),
        "request lifecycle doc should exist at the path linked from SUMMARY.md"
    );
}

#[test]
fn request_lifecycle_doc_names_the_core_boundary_types() {
    for term in [
        "SessionCommand::Prompt",
        "SessionController",
        "AgentEvent",
        "DaemonEvent",
        "EngineModelRequest",
        "CompletionRequest",
        "SessionManager",
        "_session_id",
    ] {
        assert!(REQUEST_LIFECYCLE.contains(term), "request lifecycle doc should mention boundary term `{term}`");
    }
}

#[test]
fn request_lifecycle_doc_tracks_existing_source_anchors() {
    let anchors = [
        (CONTROLLER_COMMAND, "pub async fn handle_command"),
        (CONTROLLER_EVENT_PROCESSING, "pub fn feed_event"),
        (CONTROLLER_EVENT_PROCESSING, "pub fn drain_events"),
        (CONTROLLER_EVENT_PROCESSING, "fn process_agent_event"),
        (CONTROLLER_PERSISTENCE, "pub(crate) fn persist_event"),
        (AGENT_EXECUTION, "pub(super) fn completion_request_from_engine_request"),
        (AGENT_EXECUTION, "pub(super) async fn stream_model_request"),
        (STANDALONE_EVENT_LOOP, "fn process_agent_event"),
        (DAEMON_AGENT_PROCESS, "async fn run_agent_actor"),
        (DAEMON_SOCKET_BRIDGE, "pub fn drain_and_broadcast"),
    ];

    for (source, anchor) in anchors {
        assert!(
            source.contains(anchor),
            "request lifecycle doc refers to source anchor `{anchor}`, but it was not found"
        );
    }
}

#[test]
fn request_lifecycle_doc_keeps_the_ownership_checklist() {
    for phrase in [
        "Which type is the source of truth",
        "Is the state transition owned by",
        "Does standalone mode and daemon attach mode observe the same user-visible result",
        "Does resume/replay reconstruct the same context",
        "deterministic regression test",
    ] {
        assert!(REQUEST_LIFECYCLE.contains(phrase), "request lifecycle checklist should keep phrase `{phrase}`");
    }
}
