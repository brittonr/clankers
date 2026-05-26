use std::fs;

const HARNESS: &str = "scripts/test-harness.sh";
const SCRIPT: &str = "scripts/check-daemon-attach-reconnect-dogfood.rs";
const RELEASE_READINESS: &str = "docs/src/reference/release-readiness.md";
const COMMAND: &str = "./scripts/test-harness.sh dogfood daemon-attach-reconnect";
const RECEIPT_SCHEMA: &str = "clankers.daemon_attach_reconnect_dogfood.receipt.v1";
const REQUIRED_RECEIPT_FIELDS: &[&str] = &[
    "replayed_history_visible",
    "session_not_forked",
    "post_reattach_ack_visible",
    "deterministic_provider",
    "provider_requests",
    "daemon_cleaned_up",
];

#[test]
fn daemon_attach_reconnect_dogfood_is_discoverable() {
    let harness = fs::read_to_string(HARNESS).expect("read test harness");
    let script = fs::read_to_string(SCRIPT).expect("read daemon attach reconnect dogfood script");
    let readiness = fs::read_to_string(RELEASE_READINESS).expect("read release readiness docs");

    assert!(harness.contains("daemon-attach-reconnect"));
    assert!(harness.contains("./scripts/check-daemon-attach-reconnect-dogfood.rs"));
    assert!(readiness.contains(COMMAND));
    assert!(readiness.contains(RECEIPT_SCHEMA));
    assert!(script.contains(RECEIPT_SCHEMA));

    for field in REQUIRED_RECEIPT_FIELDS {
        assert!(script.contains(field), "script receipt missing {field}");
        assert!(readiness.contains(field), "release readiness docs missing {field}");
    }
}

#[test]
fn daemon_attach_reconnect_dogfood_uses_local_deterministic_boundaries() {
    let script = fs::read_to_string(SCRIPT).expect("read daemon attach reconnect dogfood script");

    assert!(script.contains("XDG_RUNTIME_DIR"));
    assert!(script.contains("CLANKERS_AUTH_FILE"));
    assert!(script.contains("start_provider_stub"));
    assert!(script.contains("daemon attach reconnect dogfood replay sentinel"));
    assert!(script.contains("daemon\", \"stop"));
    assert!(!script.contains("aspen2"));
}

#[test]
fn daemon_attach_reconnect_doc_validator_rejects_missing_receipt_field() {
    let bad_doc = format!("{COMMAND}\n{RECEIPT_SCHEMA}\nreplayed_history_visible\n");
    let missing: Vec<&str> = REQUIRED_RECEIPT_FIELDS.iter().copied().filter(|field| !bad_doc.contains(field)).collect();

    assert!(missing.contains(&"session_not_forked"));
    assert!(missing.contains(&"daemon_cleaned_up"));
}
