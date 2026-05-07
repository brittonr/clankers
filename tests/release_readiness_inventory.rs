use std::fs;
use std::path::PathBuf;

#[test]
fn readiness_inventory_lists_required_nextest_owned_rows() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let e2e = fs::read_to_string(root.join("tests/readiness_e2e.rs")).expect("read e2e readiness tests");
    let opt_in = fs::read_to_string(root.join("tests/readiness_opt_in.rs")).expect("read opt-in readiness tests");
    let wrapper = fs::read_to_string(root.join("tests/e2e/run-tests.sh")).expect("read e2e wrapper");
    let harness = fs::read_to_string(root.join("scripts/test-harness.sh")).expect("read test harness");
    let docs = fs::read_to_string(root.join("docs/src/reference/release-readiness.md")).expect("read readiness docs");

    for required in [
        "readiness_e2e_version_help_config_and_auth_are_credential_free",
        "readiness_e2e_fake_provider_print_bash_read_find_and_json",
        "readiness_e2e_fake_provider_write_edit_read_round_trip",
        "readiness_live_local_model_aspen2_qwen36_nextest_opt_in",
        "readiness_vm_required_nixos_checks_nextest_opt_in",
        "readiness_flake_ci_nextest_opt_in",
    ] {
        assert!(e2e.contains(required) || opt_in.contains(required), "missing nextest row {required}");
    }

    for check in [
        "vm-smoke",
        "vm-remote-daemon",
        "vm-session-recovery",
        "vm-plugin-runtime",
        "vm-module-daemon",
        "vm-module-router",
        "vm-module-integration",
    ] {
        assert!(opt_in.contains(check), "VM check {check} must be represented in Rust readiness tests");
    }

    for gate in [
        "CLANKERS_RUN_LIVE_READINESS",
        "CLANKERS_RUN_VM_READINESS",
        "CLANKERS_RUN_FLAKE_READINESS",
    ] {
        assert!(opt_in.contains(gate), "missing explicit opt-in gate {gate}");
        assert!(docs.contains(gate), "readiness docs must document {gate}");
    }

    assert!(wrapper.contains("cargo nextest run"), "e2e wrapper should delegate to nextest");
    assert!(!wrapper.contains("PASS=0"), "e2e wrapper must not own readiness assertions anymore");
    assert!(
        harness.contains("cargo nextest run -p clankers --test readiness_e2e"),
        "harness e2e mode should call nextest-owned E2E tests"
    );
    assert!(
        docs.contains("cargo nextest run -p clankers --test readiness_e2e"),
        "docs should point to canonical nextest E2E command"
    );
}
