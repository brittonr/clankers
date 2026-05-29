const README: &str = include_str!("../README.md");
const RELEASE_READINESS: &str = include_str!("../docs/src/reference/release-readiness.md");
const SCRIPT: &str = include_str!("../scripts/check-streaming-tokens-recording.rs");
const TEST_HARNESS: &str = include_str!("../scripts/test-harness.sh");

const COMMAND: &str = "./scripts/test-harness.sh dogfood streaming-tokens";
const SCRIPT_PATH: &str = "./scripts/check-streaming-tokens-recording.rs";
const RECEIPT_SCHEMA: &str = "clankers.streaming_tokens_recording.receipt.v1";

#[test]
fn streaming_tokens_recording_rail_is_discoverable() {
    assert!(TEST_HARNESS.contains("streaming-tokens"), "harness should expose streaming-tokens selector");
    assert!(
        TEST_HARNESS.contains(SCRIPT_PATH),
        "harness selector should dispatch to the maintained streaming recording rail"
    );
    assert!(README.contains(COMMAND), "README should document the focused streaming-token rail");
    assert!(
        RELEASE_READINESS.contains(COMMAND),
        "release-readiness docs should document the focused streaming-token rail"
    );
}

#[test]
fn streaming_tokens_recording_rail_keeps_incremental_receipt_contract() {
    for required in [
        RECEIPT_SCHEMA,
        "observed_incremental_text",
        "observed_incremental_thinking",
        "first_thinking_visible_before_second",
        "second_thinking_visible_before_text",
        "first_text_visible_before_second",
        "second_text_visible_before_third",
        "third_text_visible_before_final",
        "final_response_not_streaming",
        "mid_stream_input_sent_before_response_returned",
        "followup_request_started_ms",
        "thinking-1",
        "token-1",
        "final",
        "screen-{label}.txt",
        "CLANKERS_THINK_ALPHA",
        "CLANKERS_STREAM_ALPHA",
        "CLANKERS_INTERRUPT_STREAM_START",
        "CLANKERS_INTERRUPT_FOLLOWUP_ACK",
        "streaming…",
    ] {
        assert!(SCRIPT.contains(required), "streaming rail script should contain {required:?}");
    }
}

#[test]
fn streaming_tokens_recording_docs_name_required_pass_criteria() {
    for required in [
        "result: pass",
        "observed_incremental_text: true",
        "observed_incremental_thinking: true",
        "provider_requests >= 3",
        "mid_stream_input_sent_before_response_returned: true",
        "screen frames where earlier deltas are visible before later deltas",
        "target/dogfood/streaming-tokens-*/receipt.json",
    ] {
        assert!(
            RELEASE_READINESS.contains(required),
            "release-readiness docs should name pass criterion {required:?}"
        );
    }
}
