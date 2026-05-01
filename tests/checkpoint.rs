use std::fs;
use std::path::Path;
use std::process::Command;

#[test]
fn checkpoint_create_list_and_rollback_round_trip() {
    let tmp = tempfile::tempdir().expect("tempdir");
    run_git(tmp.path(), &["init"]);
    fs::write(tmp.path().join("tracked.txt"), "checkpointed").expect("write fixture");
    run_git(tmp.path(), &["add", "tracked.txt"]);

    let created = clankers::checkpoints::create_checkpoint(tmp.path(), Some("safe point".to_string()))
        .expect("create checkpoint");
    let record = created.record.expect("record");
    assert_eq!(created.details.action, "create");
    assert_eq!(created.details.status, "success");
    assert_eq!(created.details.checkpoint_id.as_deref(), Some(record.id.as_str()));

    fs::write(tmp.path().join("tracked.txt"), "mutated").expect("mutate fixture");

    let listed = clankers::checkpoints::list_checkpoints(tmp.path()).expect("list checkpoints");
    assert_eq!(listed.records.len(), 1);
    assert_eq!(listed.records[0].id, record.id);

    let restored =
        clankers::checkpoints::rollback_checkpoint(tmp.path(), &record.id, true).expect("rollback checkpoint");
    assert_eq!(restored.details.action, "rollback");
    assert_eq!(fs::read_to_string(tmp.path().join("tracked.txt")).expect("read restored"), "checkpointed");
}

#[test]
fn checkpoint_rollback_requires_confirmation() {
    let tmp = tempfile::tempdir().expect("tempdir");
    run_git(tmp.path(), &["init"]);
    fs::write(tmp.path().join("tracked.txt"), "checkpointed").expect("write fixture");
    run_git(tmp.path(), &["add", "tracked.txt"]);
    let record = clankers::checkpoints::create_checkpoint(tmp.path(), None)
        .expect("create checkpoint")
        .record
        .expect("record");

    let error = clankers::checkpoints::rollback_checkpoint(tmp.path(), &record.id, false)
        .expect_err("rollback without confirmation should fail");
    assert!(error.to_string().contains("requires explicit confirmation"));
}

fn run_git(cwd: &Path, args: &[&str]) {
    let output = Command::new("git").arg("-C").arg(cwd).args(args).output().expect("run git");
    assert!(output.status.success(), "git failed: {}", String::from_utf8_lossy(&output.stderr));
}
