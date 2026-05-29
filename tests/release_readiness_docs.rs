const README: &str = include_str!("../README.md");
const SUMMARY: &str = include_str!("../docs/src/SUMMARY.md");
const RELEASE_READINESS: &str = include_str!("../docs/src/reference/release-readiness.md");
const DOGFOOD_FULL_EVIDENCE: &str = include_str!("../docs/src/reference/internal-readiness-2026-05-26-dogfood-full.md");
const TEST_HARNESS: &str = include_str!("../scripts/test-harness.sh");
const READINESS_OPT_IN_TEST: &str = include_str!("../tests/readiness_opt_in.rs");

#[test]
fn release_readiness_doc_is_discoverable() {
    assert!(
        README.contains("docs/src/reference/release-readiness.md"),
        "README should link the release-readiness checklist"
    );
    assert!(
        SUMMARY.contains("[Release Readiness](./reference/release-readiness.md)"),
        "docs SUMMARY should link the release-readiness checklist"
    );
}

#[test]
fn dogfood_full_readiness_checkpoint_evidence_is_discoverable() {
    assert!(
        SUMMARY.contains(
            "[Internal Readiness Checkpoint 2026-05-26 Dogfood Full](./reference/internal-readiness-2026-05-26-dogfood-full.md)"
        ),
        "docs SUMMARY should link the dogfood-full readiness checkpoint evidence"
    );
    assert!(
        RELEASE_READINESS.contains("internal-readiness-2026-05-26-dogfood-full"),
        "release-readiness doc should point to the dogfood-full checkpoint evidence"
    );

    for required in [
        "internal-readiness-2026-05-26-dogfood-full",
        "ccec74b659dc588934378aed34638b333304695f",
        "20260526T021502Z-3107712",
        "target/test-harness/runs/20260526T021502Z-3107712/results.json",
        "target/dogfood/bg-process-tui-1779762368/receipt.json",
        "Steps passed: `8`",
        "Steps failed: `0`",
        "Result: `pass`",
        "Active processes observed: `1`",
        "`/layout toggle bg` visibility: `true`",
        "Bounded command visibility: `true`",
        "Sentinel process cleanup: `true`",
        "It does not claim unattended public production readiness",
    ] {
        assert!(DOGFOOD_FULL_EVIDENCE.contains(required), "dogfood-full evidence page should mention {required:?}");
    }
}

#[test]
fn release_readiness_doc_names_required_harness_gates() {
    for required in [
        "./scripts/test-harness.sh full",
        "./scripts/test-harness.sh live aspen2-qwen36",
        "./scripts/test-harness.sh dogfood bg-process-tui",
        "./scripts/test-harness.sh dogfood streaming-tokens",
        "./scripts/test-harness.sh dogfood daemon-attach-streaming-abort",
        "./scripts/test-harness.sh soak streaming 2",
        "./scripts/test-harness.sh soak daemon-attach 3",
        "background-process TUI dogfood",
        "observed_incremental_text: true",
        "observed_incremental_thinking: true",
        "provider_requests >= 3",
        "mid_stream_input_sent_before_response_returned: true",
        "mid_stream_abort_processed_before_provider_returned: true",
        "followup_request_started_before_stream_completed: true",
        "busy_rejection_visible: false",
        "soak as flake-hunting evidence",
        "CLANKERS_SOAK_ITERATIONS",
        "active_processes_observed > 0",
        "sentinel_processes_cleaned_up: true",
        "primary live testing model",
        "qwen on aspen2",
        "target/test-harness/summary.md",
        "target/test-harness/results.json",
        "./scripts/test-harness.sh evidence-index",
        "target/release-evidence/current-head/index.md",
        "does not run missing profiles",
        "payload.commit",
        "payload_commit_verified",
        "payload metadata",
        "Lemonade",
        "Qwen 3.6",
        "without launching OpenAI OAuth or browser login flows",
    ] {
        assert!(RELEASE_READINESS.contains(required), "release-readiness doc should mention {required:?}");
    }
}

#[test]
fn release_readiness_doc_tracks_live_qwen_harness_seam() {
    assert!(TEST_HARNESS.contains("aspen2-qwen36"), "test harness should keep the aspen2-qwen36 selector");
    assert!(
        TEST_HARNESS.contains("readiness_live_local_model_aspen2_qwen36_nextest_opt_in"),
        "test harness should run the nextest live readiness adapter"
    );
    assert!(
        READINESS_OPT_IN_TEST.contains("aspen2_qwen36_integration"),
        "live readiness adapter should run the aspen2_qwen36 integration test"
    );
    assert!(
        READINESS_OPT_IN_TEST.contains("CLANKERS_RUN_LIVE_READINESS"),
        "live readiness adapter should keep explicit opt-in gating"
    );
    assert!(
        TEST_HARNESS.contains("run_dogfood_selector bg-process-tui"),
        "full harness should run the maintained BG-process TUI dogfood rail"
    );
}
