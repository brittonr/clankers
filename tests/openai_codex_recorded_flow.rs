use std::path::PathBuf;
use std::process::Command;
use std::process::Output;
use std::process::Stdio;

fn clankers_bin() -> PathBuf {
    std::env::var_os("CARGO_BIN_EXE_clankers")
        .map(PathBuf::from)
        .expect("CARGO_BIN_EXE_clankers should be set for integration tests")
}

fn fixture_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/openai_codex").join(name)
}

fn run_clankers(home: &std::path::Path, args: &[&str], stdin: &str) -> Output {
    let mut child = Command::new(clankers_bin())
        .args(args)
        .env("HOME", home)
        .env("XDG_CONFIG_HOME", home.join(".config"))
        .env("NO_COLOR", "1")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn clankers");

    if !stdin.is_empty() {
        use std::io::Write;
        child.stdin.as_mut().expect("stdin should exist").write_all(stdin.as_bytes()).expect("write stdin");
    }

    child.wait_with_output().expect("wait for clankers")
}

fn stdout_text(output: &Output) -> String {
    String::from_utf8(output.stdout.clone()).expect("stdout should be utf8")
}

#[test]
fn recorded_cli_login_start_persists_pending_codex_login_fixture() {
    let home = tempfile::TempDir::new().expect("tempdir should exist");
    let output = run_clankers(home.path(), &["auth", "login", "--provider", "openai-codex", "--account", "work"], "\n");

    let stdout = stdout_text(&output);
    assert!(stdout.contains("Logging in to provider 'openai-codex' as account 'work'."), "{stdout}");
    assert!(stdout.contains("https://auth.openai.com/oauth/authorize"), "{stdout}");

    let pending_path = home.path().join(".clankers/agent/.login_verifiers/openai-codex/work.json");
    let pending = std::fs::read_to_string(&pending_path).expect("pending login file should exist");
    assert!(pending.contains("\"provider\":\"openai-codex\""), "{pending}");
    assert!(pending.contains("\"account\":\"work\""), "{pending}");
}

#[test]
fn recorded_cli_status_and_switch_flow_uses_fixture_accounts() {
    let home = tempfile::TempDir::new().expect("tempdir should exist");

    let import_work = run_clankers(
        home.path(),
        &[
            "auth",
            "import",
            "--input",
            fixture_path("work-export.json").to_str().expect("fixture path"),
        ],
        "",
    );
    assert!(import_work.status.success(), "{}", stdout_text(&import_work));

    let import_backup = run_clankers(
        home.path(),
        &[
            "auth",
            "import",
            "--input",
            fixture_path("backup-export.json").to_str().expect("fixture path"),
        ],
        "",
    );
    assert!(import_backup.status.success(), "{}", stdout_text(&import_backup));

    let status_before = run_clankers(home.path(), &["auth", "status", "--provider", "openai-codex"], "");
    assert!(status_before.status.success());
    let before = stdout_text(&status_before);
    assert!(before.contains("openai-codex:"), "{before}");
    assert!(before.contains("▸ work (work fixture)"), "{before}");
    assert!(before.contains("backup (backup fixture)"), "{before}");
    assert!(before.contains("authenticated, entitlement check failed"), "{before}");

    let switch = run_clankers(home.path(), &["auth", "switch", "--provider", "openai-codex", "backup"], "");
    assert!(switch.status.success());
    assert!(stdout_text(&switch).contains("Switched provider 'openai-codex' to account 'backup'."));

    let status_after = run_clankers(home.path(), &["auth", "status", "--provider", "openai-codex"], "");
    assert!(status_after.status.success());
    let after = stdout_text(&status_after);
    assert!(after.contains("▸ backup (backup fixture)"), "{after}");
    assert!(after.contains("work (work fixture)"), "{after}");
}
