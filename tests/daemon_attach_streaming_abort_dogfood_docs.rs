const README: &str = include_str!("../README.md");
const RELEASE_READINESS: &str = include_str!("../docs/src/reference/release-readiness.md");
const SCRIPT: &str = include_str!("../scripts/check-daemon-attach-streaming-abort-dogfood.rs");
const TEST_HARNESS: &str = include_str!("../scripts/test-harness.sh");

const COMMAND: &str = "./scripts/test-harness.sh dogfood daemon-attach-streaming-abort";
const SCRIPT_PATH: &str = "./scripts/check-daemon-attach-streaming-abort-dogfood.rs";
const RECEIPT_SCHEMA: &str = "clankers.daemon_attach_streaming_abort_dogfood.receipt.v1";

#[test]
fn daemon_attach_streaming_abort_rail_is_discoverable() {
    assert!(
        TEST_HARNESS.contains("daemon-attach-streaming-abort"),
        "harness should expose daemon-attach-streaming-abort selector"
    );
    assert!(
        TEST_HARNESS.contains(SCRIPT_PATH),
        "harness selector should dispatch to the maintained daemon attach streaming abort rail"
    );
    assert!(README.contains(COMMAND), "README should document the focused daemon attach abort rail");
    assert!(
        RELEASE_READINESS.contains(COMMAND),
        "release-readiness docs should document the focused daemon attach abort rail"
    );
}

#[test]
fn daemon_attach_streaming_abort_rail_keeps_receipt_contract() {
    for required in [
        RECEIPT_SCHEMA,
        "mid_stream_abort_processed_before_provider_returned",
        "followup_request_started_before_stream_completed",
        "busy_rejection_visible",
        "provider_requests_at_least_two",
        "DAEMON_ATTACH_ABORT_START",
        "DAEMON_ATTACH_ABORT_FOLLOWUP_ACK",
        "A prompt is already in progress",
        "screen-{label}.txt",
        "stream-active",
        "followup-ack",
        "stream_completed_ms",
        "followup_started_ms",
        "daemon_cleaned_up",
    ] {
        assert!(SCRIPT.contains(required), "daemon attach abort rail script should contain {required:?}");
    }
}

#[test]
fn daemon_attach_streaming_abort_docs_name_required_pass_criteria() {
    for required in [
        "target/dogfood/daemon-attach-streaming-abort-*/receipt.json",
        "result: pass",
        "mid_stream_abort_processed_before_provider_returned: true",
        "followup_request_started_before_stream_completed: true",
        "provider_requests >= 2",
        "busy_rejection_visible: false",
        "daemon_cleaned_up: true",
        "screen frames showing the active streaming state before the follow-up ack",
    ] {
        assert!(
            RELEASE_READINESS.contains(required),
            "release-readiness docs should name pass criterion {required:?}"
        );
    }
}
