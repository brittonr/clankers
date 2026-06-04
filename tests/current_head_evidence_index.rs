use std::path::Path;
use std::process::Command;

use serde_json::Value;
use serde_json::json;

#[test]
fn evidence_index_verifies_matching_clean_payload_receipt() {
    let receipt_dir = tempfile::tempdir().expect("receipt tempdir should exist");
    let out_dir = tempfile::tempdir().expect("index tempdir should exist");
    let head = git_stdout(["rev-parse", "HEAD"]);
    write_receipt(receipt_dir.path(), "quick", "matching", Some((&head, false)));

    let index = run_index(receipt_dir.path(), out_dir.path());
    let selected = &index["selected_receipts"]["quick"];
    assert_eq!(selected["payload_commit_verified"], true, "matching clean payload should verify: {selected:#}");
    assert!(
        selected["note"].as_str().expect("note should be a string").contains("matches indexed HEAD"),
        "note should explain positive verification: {selected:#}"
    );
}

#[test]
fn evidence_index_does_not_verify_legacy_dirty_or_mismatched_payload_receipts() {
    let cases = [
        ("legacy", None),
        ("dirty", Some(("HEAD", true))),
        ("mismatch", Some(("0000000000000000000000000000000000000000", false))),
    ];

    for (name, payload) in cases {
        let receipt_dir = tempfile::tempdir().expect("receipt tempdir should exist");
        let out_dir = tempfile::tempdir().expect("index tempdir should exist");
        let payload = payload.map(|(commit, dirty)| {
            if commit == "HEAD" {
                (git_stdout(["rev-parse", "HEAD"]), dirty)
            } else {
                (commit.to_string(), dirty)
            }
        });
        let payload_ref = payload.as_ref().map(|(commit, dirty)| (commit.as_str(), *dirty));
        write_receipt(receipt_dir.path(), "quick", name, payload_ref);

        let index = run_index(receipt_dir.path(), out_dir.path());
        let selected = &index["selected_receipts"]["quick"];
        assert_eq!(
            selected["payload_commit_verified"], false,
            "{name} receipt must not be current-HEAD verified: {selected:#}"
        );
    }
}

fn run_index(receipt_dir: &Path, out_dir: &Path) -> Value {
    let output = Command::new("./scripts/check-current-head-release-evidence.rs")
        .current_dir(repo_root())
        .args(["--allow-dirty", "--result-dir"])
        .arg(receipt_dir)
        .arg("--out-dir")
        .arg(out_dir)
        .output()
        .expect("evidence index helper should spawn");
    assert!(
        output.status.success(),
        "evidence index helper failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_str(&std::fs::read_to_string(out_dir.join("index.json")).expect("index.json should be readable"))
        .expect("index.json should be valid JSON")
}

fn write_receipt(root: &Path, mode: &str, run_id: &str, payload: Option<(&str, bool)>) {
    let run_dir = root.join("runs").join(run_id);
    let log_dir = run_dir.join("logs");
    std::fs::create_dir_all(&log_dir).expect("log dir should be created");
    let summary_path = run_dir.join("summary.md");
    let log_path = log_dir.join("step.log");
    std::fs::write(&summary_path, "# summary\n").expect("summary should be written");
    std::fs::write(&log_path, "ok\n").expect("log should be written");

    let mut receipt = json!({
        "mode": mode,
        "run_id": run_id,
        "run_dir": display_path(&run_dir),
        "started_at": "2026-05-23T00:00:00Z",
        "finished_at": format!("2026-05-23T00:00:0{}Z", run_id.len() % 10),
        "passed": 1,
        "failed": 0,
        "skipped": 0,
        "steps": [{
            "name": "fixture step",
            "status": "passed",
            "exit_code": 0,
            "command": "true",
            "log": display_path(&log_path),
        }],
    });
    if let Some((commit, tracked_dirty)) = payload {
        receipt["payload"] = json!({
            "commit": commit,
            "branch": "main",
            "describe": commit,
            "tracked_dirty": tracked_dirty,
            "upstream": null,
            "ahead_behind": null,
        });
    }
    std::fs::write(
        run_dir.join("results.json"),
        serde_json::to_string_pretty(&receipt).expect("receipt should encode") + "\n",
    )
    .expect("receipt should be written");
}

fn git_stdout<const N: usize>(args: [&str; N]) -> String {
    let output = Command::new("git").current_dir(repo_root()).args(args).output().expect("git should spawn");
    assert!(output.status.success(), "git command failed: {}", String::from_utf8_lossy(&output.stderr));
    String::from_utf8(output.stdout).expect("git stdout should be utf8").trim().to_string()
}

fn display_path(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

fn repo_root() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
}
