use std::path::Path;
use std::process::Command;

use serde_json::Value;

#[derive(Clone, Debug)]
struct HarnessCase {
    name: &'static str,
    args: &'static [&'static str],
    expected_steps: &'static [&'static str],
    expected_commands: &'static [&'static str],
}

#[test]
fn test_harness_dry_run_receipts_cover_representative_modes() {
    let cases = [
        HarnessCase {
            name: "quick",
            args: &["quick"],
            expected_steps: &["cargo check tests", "cargo nextest workspace"],
            expected_commands: &["cargo check --tests", "cargo nextest run --workspace --no-fail-fast"],
        },
        HarnessCase {
            name: "full",
            args: &["full"],
            expected_steps: &[
                "cargo fmt check",
                "cargo check tests",
                "cargo nextest workspace",
                "cargo clippy",
                "repo verify",
                "tigerstyle",
                "live readiness aspen2-qwen36",
                "dogfood bg-process-tui",
            ],
            expected_commands: &[
                "cargo fmt --check",
                "cargo check --tests",
                "cargo nextest run --workspace --no-fail-fast",
                "cargo clippy --workspace --all-targets -- -D warnings",
                "./scripts/verify.sh",
                "./xtask/tigerstyle.sh",
                "env CLANKERS_RUN_LIVE_READINESS=1 CLANKERS_LIVE_READINESS_SELECTOR=aspen2-qwen36 cargo nextest run -p clankers --test readiness_opt_in --no-fail-fast",
                "./scripts/check-bg-process-tui-dogfood.rs",
            ],
        },
        HarnessCase {
            name: "deterministic",
            args: &["deterministic"],
            expected_steps: &[
                "deterministic engine replay",
                "deterministic controller replay",
                "deterministic session resume replay",
            ],
            expected_commands: &[
                "cargo nextest run -p clankers-engine --test deterministic_turn_replay --no-fail-fast",
                "cargo nextest run -p clankers --test controller_deterministic_replay --no-fail-fast",
                "cargo nextest run -p clankers --test session_resume_deterministic_replay --no-fail-fast",
            ],
        },
        HarnessCase {
            name: "e2e-api",
            args: &["e2e", "api"],
            expected_steps: &["e2e api"],
            expected_commands: &["cargo nextest run -p clankers --test readiness_e2e --no-fail-fast"],
        },
        HarnessCase {
            name: "vm-smoke",
            args: &["vm", "smoke"],
            expected_steps: &["vm readiness smoke"],
            expected_commands: &[
                "env CLANKERS_RUN_VM_READINESS=1 CLANKERS_VM_READINESS_SELECTOR=smoke cargo nextest run -p clankers --test readiness_opt_in --no-fail-fast",
            ],
        },
        HarnessCase {
            name: "ci",
            args: &["ci"],
            expected_steps: &["flake readiness"],
            expected_commands: &[
                "env CLANKERS_RUN_FLAKE_READINESS=1 cargo nextest run -p clankers --test readiness_opt_in --no-fail-fast",
            ],
        },
        HarnessCase {
            name: "evidence-index",
            args: &["evidence-index"],
            expected_steps: &["current head release evidence index"],
            expected_commands: &["./scripts/check-current-head-release-evidence.rs --result-dir"],
        },
    ];

    for case in cases {
        let receipt = run_harness_dry_run(&case, &format!("contract-{}", case.name));
        assert_receipt_contract(&case, &receipt, &format!("contract-{}", case.name));
    }
}

#[test]
fn test_harness_list_mode_documents_profiles_selectors_env_and_receipts() {
    let receipt_dir = tempfile::tempdir().expect("receipt tempdir should be creatable");
    let output = Command::new("bash")
        .current_dir(repo_root())
        .env("CLANKERS_TEST_RESULT_DIR", receipt_dir.path())
        .env("CLANKERS_TEST_RUN_ID", "list-contract")
        .args(["scripts/test-harness.sh", "list"])
        .output()
        .expect("test harness list mode should spawn");

    assert!(
        output.status.success(),
        "list mode failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    for expected in [
        "# clankers test harness profiles",
        "## Modes",
        "`quick`",
        "`package <crate> [filter...]`",
        "`full`",
        "primary live aspen2 Qwen gate",
        "`deterministic`",
        "`e2e [fake|deterministic|fast|api|all|test-name]`",
        "`live [local-model|aspen2-qwen36|all]`",
        "`dogfood [bg-process-tui]`",
        "`vm [all|core|module|smoke|check-name]`",
        "`ci [extra nix args...]`",
        "`evidence-index`",
        "does not run missing readiness profiles",
        "`list`",
        "## Selectors",
        "E2E selectors",
        "Deterministic profile",
        "scripted provider/tool fixtures",
        "controller/agent replay tests",
        "persisted session-resume replay tests",
        "no live credentials",
        "Live selectors",
        "Dogfood selectors",
        "bg-process-tui",
        "VM selectors",
        "vm-smoke",
        "vm-module-daemon",
        "## Environment",
        "CLANKERS_TEST_DRY_RUN=1",
        "CLANKERS_TEST_RESULT_DIR=<dir>",
        "CLANKERS_TEST_RUN_ID=<id>",
        "CARGO_TARGET_DIR=<dir>",
        "CLANKERS_NO_DAEMON=1",
        "## Receipts",
        "<result-dir>/runs/<run_id>/summary.md",
        "<result-dir>/runs/<run_id>/results.json",
        "<result-dir>/runs/<run_id>/junit.xml",
        "<result-dir>/runs/<run_id>/logs/*.log",
        "payload.commit",
        "payload.tracked_dirty",
        "<result-dir>/summary.md",
    ] {
        assert!(stdout.contains(expected), "list output should contain {expected:?}\n{stdout}");
    }
}

#[test]
fn test_harness_stable_receipts_point_to_latest_completed_run() {
    let case = HarnessCase {
        name: "quick",
        args: &["quick"],
        expected_steps: &["cargo check tests", "cargo nextest workspace"],
        expected_commands: &["cargo check --tests", "cargo nextest run --workspace --no-fail-fast"],
    };
    let receipt_dir = tempfile::tempdir().expect("receipt tempdir should be creatable");

    run_harness_dry_run_in(&case, receipt_dir.path(), "first-run");
    let first_run_json_path = receipt_dir.path().join("runs/first-run/results.json");
    assert!(first_run_json_path.is_file(), "first run primary receipt should exist");

    run_harness_dry_run_in(&case, receipt_dir.path(), "second-run");

    let stable_json: Value = serde_json::from_str(
        &std::fs::read_to_string(receipt_dir.path().join("results.json"))
            .expect("stable results.json should be readable"),
    )
    .expect("stable results.json should be valid JSON");
    assert_eq!(stable_json["run_id"], "second-run");
    assert_eq!(stable_json["run_dir"], receipt_dir.path().join("runs/second-run").to_string_lossy().as_ref());
    assert!(first_run_json_path.is_file(), "stable copy must not remove older run receipt");
    assert!(receipt_dir.path().join("runs/second-run/logs/cargo_check_tests.log").is_file());
}

fn run_harness_dry_run(case: &HarnessCase, run_id: &str) -> tempfile::TempDir {
    let receipt_dir = tempfile::tempdir().expect("receipt tempdir should be creatable");
    run_harness_dry_run_in(case, receipt_dir.path(), run_id);
    receipt_dir
}

fn run_harness_dry_run_in(case: &HarnessCase, receipt_dir: &Path, run_id: &str) {
    let output = Command::new("bash")
        .current_dir(repo_root())
        .env("CLANKERS_TEST_DRY_RUN", "1")
        .env("CLANKERS_TEST_RESULT_DIR", receipt_dir)
        .env("CLANKERS_TEST_RUN_ID", run_id)
        .args(["scripts/test-harness.sh"])
        .args(case.args)
        .output()
        .expect("test harness should spawn");

    assert!(
        output.status.success(),
        "dry-run harness case {} failed\nstdout:\n{}\nstderr:\n{}",
        case.name,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

fn assert_receipt_contract(case: &HarnessCase, receipt_dir: &tempfile::TempDir, run_id: &str) {
    let result_path = receipt_dir.path().join("results.json");
    let summary_path = receipt_dir.path().join("summary.md");
    let junit_path = receipt_dir.path().join("junit.xml");
    let run_dir = receipt_dir.path().join("runs").join(run_id);
    let run_result_path = run_dir.join("results.json");
    let run_summary_path = run_dir.join("summary.md");
    let run_junit_path = run_dir.join("junit.xml");
    assert!(result_path.is_file(), "{} should write stable results.json", case.name);
    assert!(summary_path.is_file(), "{} should write stable summary.md", case.name);
    assert!(junit_path.is_file(), "{} should write stable junit.xml", case.name);
    assert!(run_result_path.is_file(), "{} should write run-scoped results.json", case.name);
    assert!(run_summary_path.is_file(), "{} should write run-scoped summary.md", case.name);
    assert!(run_junit_path.is_file(), "{} should write run-scoped junit.xml", case.name);

    let results = std::fs::read_to_string(&result_path).expect("results.json should be readable");
    let run_results = std::fs::read_to_string(&run_result_path).expect("run results.json should be readable");
    assert_eq!(results, run_results, "stable results should copy completed run results");
    let summary = std::fs::read_to_string(&summary_path).expect("summary.md should be readable");
    let junit = std::fs::read_to_string(&junit_path).expect("junit.xml should be readable");
    let json: Value = serde_json::from_str(&results).expect("results.json should be valid JSON");

    let steps = json["steps"].as_array().expect("steps should be a JSON array");
    assert_eq!(steps.len(), case.expected_steps.len(), "{} step count", case.name);
    assert_eq!(json["passed"], 0, "{} dry-run should not pass real steps", case.name);
    assert_eq!(json["failed"], 0, "{} dry-run should not fail", case.name);
    assert_eq!(json["skipped"], case.expected_steps.len(), "{} skipped count", case.name);
    assert_eq!(json["mode"], case.args[0], "{} mode field", case.name);
    assert_eq!(json["run_id"], run_id, "{} run_id field", case.name);
    assert_eq!(json["run_dir"], run_dir.to_string_lossy().as_ref(), "{} run_dir field", case.name);
    let payload = json["payload"].as_object().expect("receipt should include payload metadata");
    assert!(
        payload["commit"].as_str().is_some_and(|commit| commit.len() >= 7),
        "payload commit should be present"
    );
    assert!(payload["branch"].is_string(), "payload branch should be present");
    assert!(payload["describe"].is_string(), "payload describe should be present");
    assert!(payload["tracked_dirty"].is_boolean(), "payload tracked_dirty should be boolean");
    assert!(payload.contains_key("upstream"), "payload upstream key should be present");
    assert!(payload.contains_key("ahead_behind"), "payload ahead_behind key should be present");

    assert!(summary.contains("# clankers test harness summary"));
    assert!(summary.contains(&format!("- run_id: `{run_id}`")));
    assert!(summary.contains(&format!("- run_dir: `{}`", run_dir.to_string_lossy())));
    assert!(summary.contains("- failed: 0"));
    assert!(junit.contains("<testsuites>"));
    assert!(junit.contains("<testsuite"));

    for (index, step) in steps.iter().enumerate() {
        let expected_name = case.expected_steps[index];
        let expected_command = case.expected_commands[index];
        assert_eq!(step["name"], expected_name, "{} step name {index}", case.name);
        assert_eq!(step["status"], "skipped", "{} dry-run status {index}", case.name);
        assert_eq!(step["exit_code"], 0, "{} dry-run exit code {index}", case.name);
        let command = step["command"].as_str().expect("step command should be a string");
        assert!(
            command.contains(expected_command),
            "{} command {index} should contain {expected_command:?}, got {command:?}",
            case.name
        );
        let log = step["log"].as_str().expect("step log should be a string");
        assert!(
            Path::new(log).starts_with(run_dir.join("logs")),
            "{} step log should be under run-scoped log dir: {log}",
            case.name
        );
        assert!(Path::new(log).is_file(), "{} step log should exist: {log}", case.name);
        assert!(
            std::fs::read_to_string(log).expect("step log should be readable").contains("DRY RUN:"),
            "{} dry-run step log should record DRY RUN marker: {log}",
            case.name
        );
        assert!(summary.contains(expected_name), "summary should mention step {expected_name}");
        assert!(summary.contains(command), "summary should mention command {command}");
        assert!(junit.contains(&format!("name=\"{}\"", xml_escape(expected_name))));
        assert!(junit.contains("<skipped message=\"dry run\"/>"));
    }
}

fn repo_root() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
}

fn xml_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}
