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
            ],
            expected_commands: &[
                "cargo fmt --check",
                "cargo check --tests",
                "cargo nextest run --workspace --no-fail-fast",
                "cargo clippy --workspace --all-targets -- -D warnings",
                "./scripts/verify.sh",
                "./xtask/tigerstyle.sh",
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
    ];

    for case in cases {
        let receipt = run_harness_dry_run(&case);
        assert_receipt_contract(&case, &receipt);
    }
}

fn run_harness_dry_run(case: &HarnessCase) -> tempfile::TempDir {
    let receipt_dir = tempfile::tempdir().expect("receipt tempdir should be creatable");
    let output = Command::new("bash")
        .current_dir(repo_root())
        .env("CLANKERS_TEST_DRY_RUN", "1")
        .env("CLANKERS_TEST_RESULT_DIR", receipt_dir.path())
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
    receipt_dir
}

fn assert_receipt_contract(case: &HarnessCase, receipt_dir: &tempfile::TempDir) {
    let result_path = receipt_dir.path().join("results.json");
    let summary_path = receipt_dir.path().join("summary.md");
    let junit_path = receipt_dir.path().join("junit.xml");
    assert!(result_path.is_file(), "{} should write results.json", case.name);
    assert!(summary_path.is_file(), "{} should write summary.md", case.name);
    assert!(junit_path.is_file(), "{} should write junit.xml", case.name);

    let results = std::fs::read_to_string(&result_path).expect("results.json should be readable");
    let summary = std::fs::read_to_string(&summary_path).expect("summary.md should be readable");
    let junit = std::fs::read_to_string(&junit_path).expect("junit.xml should be readable");
    let json: Value = serde_json::from_str(&results).expect("results.json should be valid JSON");

    let steps = json["steps"].as_array().expect("steps should be a JSON array");
    assert_eq!(steps.len(), case.expected_steps.len(), "{} step count", case.name);
    assert_eq!(json["passed"], 0, "{} dry-run should not pass real steps", case.name);
    assert_eq!(json["failed"], 0, "{} dry-run should not fail", case.name);
    assert_eq!(json["skipped"], case.expected_steps.len(), "{} skipped count", case.name);
    assert_eq!(json["mode"], case.args[0], "{} mode field", case.name);

    assert!(summary.contains("# clankers test harness summary"));
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
