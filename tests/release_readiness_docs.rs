const README: &str = include_str!("../README.md");
const SUMMARY: &str = include_str!("../docs/src/SUMMARY.md");
const RELEASE_READINESS: &str = include_str!("../docs/src/reference/release-readiness.md");
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
fn release_readiness_doc_names_required_harness_gates() {
    for required in [
        "./scripts/test-harness.sh full",
        "./scripts/test-harness.sh live aspen2-qwen36",
        "target/test-harness/summary.md",
        "target/test-harness/results.json",
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
}
