use std::path::PathBuf;
use std::process::Command;
use std::process::Output;

use serde_json::Value;

fn clankers_bin() -> PathBuf {
    std::env::var_os("CARGO_BIN_EXE_clankers")
        .map(PathBuf::from)
        .expect("CARGO_BIN_EXE_clankers should be set for integration tests")
}

fn run_clankers(args: &[String]) -> Output {
    Command::new(clankers_bin()).args(args).env("NO_COLOR", "1").output().expect("run clankers")
}

fn run_clankers_json(args: &[String]) -> Value {
    let output = run_clankers(args);
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "clankers command failed\nargs: {args:?}\nstatus: {:?}\nstdout:\n{stdout}\nstderr:\n{stderr}",
        output.status.code()
    );
    serde_json::from_slice(&output.stdout).expect("stdout should be JSON")
}

fn run_clankers_failure(args: &[String]) -> Output {
    let output = run_clankers(args);
    assert!(
        !output.status.success(),
        "clankers command unexpectedly succeeded\nargs: {args:?}\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    output
}

fn string_field<'a>(value: &'a Value, path: &[&str]) -> &'a str {
    let mut current = value;
    for key in path {
        current = current.get(*key).unwrap_or_else(|| panic!("missing JSON key {key} in {value}"));
    }
    current.as_str().unwrap_or_else(|| panic!("JSON path {path:?} should be a string: {value}"))
}

#[test]
fn self_evolution_cli_runs_approve_preflight_and_live_apply_with_temp_files() {
    let tmp = tempfile::TempDir::new().expect("tempdir should exist");
    let target = tmp.path().join("target.txt");
    let candidate_source = tmp.path().join("candidate-source.txt");
    let candidate_output = tmp.path().join("candidates");

    std::fs::write(&target, "initial target artifact\n").expect("write target");
    std::fs::write(&candidate_source, "initial target artifact\nimproved candidate line\n").expect("write candidate");
    std::fs::create_dir(&candidate_output).expect("create candidate output root");

    let run = run_clankers_json(&[
        "self-evolution".into(),
        "run".into(),
        "--target".into(),
        target.display().to_string(),
        "--baseline-command".into(),
        format!("test -s {}", target.display()),
        "--candidate-output".into(),
        candidate_output.display().to_string(),
        "--candidate-file".into(),
        candidate_source.display().to_string(),
        "--session".into(),
        "cli-e2e-session".into(),
        "--dry-run".into(),
        "--json".into(),
    ]);

    assert_eq!(string_field(&run, &["status"]), "completed");
    assert_eq!(string_field(&run, &["recommendation", "promotion_status"]), "awaiting_human_approval");
    assert_eq!(run["recommendation"]["recommended"], true);
    assert_eq!(run["candidate"]["changed_from_baseline"], true);
    assert_eq!(std::fs::read_to_string(&target).expect("target after run"), "initial target artifact\n");

    let run_dir = PathBuf::from(string_field(&run, &["candidate", "output_dir"]));
    let receipt_path = run_dir.join("receipt.json");
    let approval_path = run_dir.join("approval.json");
    let application_path = run_dir.join("application.json");

    assert!(receipt_path.is_file(), "run receipt should be persisted");
    assert!(!approval_path.exists(), "approval should not exist before approval step");
    assert!(!application_path.exists(), "application should not exist before apply step");

    let approval = run_clankers_json(&[
        "self-evolution".into(),
        "approve".into(),
        "--receipt".into(),
        receipt_path.display().to_string(),
        "--session".into(),
        "cli-e2e-session".into(),
        "--confirmation-id".into(),
        "cli-e2e-confirmation".into(),
        "--approver".into(),
        "cli-e2e-human".into(),
        "--dry-run".into(),
        "--json".into(),
    ]);

    assert_eq!(string_field(&approval, &["approval", "promotion_status"]), "approval_recorded_not_applied");
    assert_eq!(approval["approval"]["approved"], true);
    assert_eq!(approval["approval"]["applied"], false);
    assert!(approval_path.is_file(), "approval receipt should be persisted");
    assert!(!application_path.exists(), "approval should not create application receipt");

    let verify_command = format!("grep -q 'improved candidate line' {}", target.display());
    let preflight = run_clankers_json(&[
        "self-evolution".into(),
        "apply".into(),
        "--receipt".into(),
        receipt_path.display().to_string(),
        "--approval".into(),
        approval_path.display().to_string(),
        "--mode".into(),
        "replace-file".into(),
        "--verify-command".into(),
        verify_command.clone(),
        "--dry-run".into(),
        "--json".into(),
    ]);

    assert_eq!(string_field(&preflight, &["status"]), "preflight_validated");
    assert_eq!(preflight["applied"], false);
    assert_eq!(string_field(&preflight, &["verification", "status"]), "recorded_not_executed_dry_run");
    assert_eq!(std::fs::read_to_string(&target).expect("target after preflight"), "initial target artifact\n");
    assert!(!application_path.exists(), "dry-run apply must not write application receipt");

    let live = run_clankers_json(&[
        "self-evolution".into(),
        "apply".into(),
        "--receipt".into(),
        receipt_path.display().to_string(),
        "--approval".into(),
        approval_path.display().to_string(),
        "--mode".into(),
        "replace-file".into(),
        "--verify-command".into(),
        verify_command,
        "--live-apply".into(),
        "--json".into(),
    ]);

    assert_eq!(string_field(&live, &["status"]), "applied");
    assert_eq!(live["applied"], true);
    assert_eq!(string_field(&live, &["verification", "status"]), "passed");
    assert_eq!(
        std::fs::read_to_string(&target).expect("target after live apply"),
        "initial target artifact\nimproved candidate line\n"
    );

    let planned_backup = PathBuf::from(string_field(&live, &["planned_backup_path"]));
    assert!(planned_backup.is_file(), "backup should exist at planned path");
    assert_eq!(std::fs::read_to_string(&planned_backup).expect("backup contents"), "initial target artifact\n");
    assert!(application_path.is_file(), "live apply should persist application receipt");

    let application: Value =
        serde_json::from_str(&std::fs::read_to_string(&application_path).expect("read application receipt"))
            .expect("application receipt should be JSON");
    assert_eq!(string_field(&application, &["status"]), "applied");
    assert_eq!(string_field(&application, &["rollback", "backup_path"]), planned_backup.display().to_string());
    assert_eq!(application["rollback"]["instructions"].as_array().expect("rollback instructions").len(), 2);

    let rollback_preflight = run_clankers_json(&[
        "self-evolution".into(),
        "rollback".into(),
        "--application".into(),
        application_path.display().to_string(),
        "--dry-run".into(),
        "--json".into(),
    ]);
    assert_eq!(string_field(&rollback_preflight, &["status"]), "rollback_preflight_validated");
    assert_eq!(rollback_preflight["restored"], false);
    assert_eq!(
        std::fs::read_to_string(&target).expect("target after rollback preflight"),
        "initial target artifact\nimproved candidate line\n"
    );
    let rollback_path = run_dir.join("rollback.json");
    assert!(!rollback_path.exists(), "dry-run rollback must not write rollback receipt");

    let rollback = run_clankers_json(&[
        "self-evolution".into(),
        "rollback".into(),
        "--application".into(),
        application_path.display().to_string(),
        "--yes".into(),
        "--json".into(),
    ]);
    assert_eq!(string_field(&rollback, &["status"]), "rolled_back");
    assert_eq!(rollback["restored"], true);
    assert_eq!(std::fs::read_to_string(&target).expect("target after rollback"), "initial target artifact\n");
    assert!(rollback_path.is_file(), "live rollback should persist rollback receipt");
}

#[test]
fn self_evolution_cli_rejects_stale_target_live_apply_before_mutation() {
    let tmp = tempfile::TempDir::new().expect("tempdir should exist");
    let target = tmp.path().join("target.txt");
    let candidate_source = tmp.path().join("candidate-source.txt");
    let candidate_output = tmp.path().join("candidates");

    std::fs::write(&target, "initial target artifact\n").expect("write target");
    std::fs::write(&candidate_source, "initial target artifact\nimproved candidate line\n").expect("write candidate");
    std::fs::create_dir(&candidate_output).expect("create candidate output root");

    let run = run_clankers_json(&[
        "self-evolution".into(),
        "run".into(),
        "--target".into(),
        target.display().to_string(),
        "--baseline-command".into(),
        format!("test -s {}", target.display()),
        "--candidate-output".into(),
        candidate_output.display().to_string(),
        "--candidate-file".into(),
        candidate_source.display().to_string(),
        "--session".into(),
        "cli-stale-session".into(),
        "--dry-run".into(),
        "--json".into(),
    ]);
    assert_eq!(run["recommendation"]["recommended"], true);

    let run_dir = PathBuf::from(string_field(&run, &["candidate", "output_dir"]));
    let receipt_path = run_dir.join("receipt.json");
    let approval_path = run_dir.join("approval.json");
    let application_path = run_dir.join("application.json");

    let approval = run_clankers_json(&[
        "self-evolution".into(),
        "approve".into(),
        "--receipt".into(),
        receipt_path.display().to_string(),
        "--session".into(),
        "cli-stale-session".into(),
        "--confirmation-id".into(),
        "cli-stale-confirmation".into(),
        "--approver".into(),
        "cli-stale-human".into(),
        "--dry-run".into(),
        "--json".into(),
    ]);
    assert_eq!(approval["approval"]["approved"], true);

    std::fs::write(&target, "operator changed target before apply\n").expect("mutate target after approval");

    let verify_command = format!("grep -q 'improved candidate line' {}", target.display());
    let failed_apply = run_clankers_failure(&[
        "self-evolution".into(),
        "apply".into(),
        "--receipt".into(),
        receipt_path.display().to_string(),
        "--approval".into(),
        approval_path.display().to_string(),
        "--mode".into(),
        "replace-file".into(),
        "--verify-command".into(),
        verify_command,
        "--live-apply".into(),
        "--json".into(),
    ]);

    let stderr = String::from_utf8_lossy(&failed_apply.stderr);
    assert!(
        stderr.contains("target artifact changed"),
        "stale-target error should be actionable, got stderr:\n{stderr}"
    );
    assert_eq!(
        std::fs::read_to_string(&target).expect("target after rejected live apply"),
        "operator changed target before apply\n"
    );
    assert!(!application_path.exists(), "rejected live apply must not persist application receipt");
    assert!(
        std::fs::read_dir(&run_dir)
            .expect("read run dir")
            .filter_map(std::result::Result::ok)
            .all(|entry| !entry.file_name().to_string_lossy().contains("backup")),
        "stale-target rejection should happen before backup creation"
    );
}
