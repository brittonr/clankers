mod readiness_common;

use std::fs;
use std::time::Duration;

use readiness_common::ReadinessSandbox;
use readiness_common::repo_root;
use serde_json::Value;

#[test]
fn readiness_e2e_version_help_config_and_auth_are_credential_free() {
    let sandbox = ReadinessSandbox::new();

    let version = sandbox.clankers().arg("version").run();
    version.assert_success();
    assert!(version.stdout.contains("clankers 0.1.0"), "stdout: {}", version.stdout);

    let help = sandbox.clankers().arg("--help").run();
    help.assert_success();
    assert!(help.stdout.contains("clankers") && help.stdout.contains("Commands"), "stdout: {}", help.stdout);

    let paths = sandbox.clankers().args(["config", "paths"]).run();
    paths.assert_success();
    assert!(paths.stdout.contains("Global config"), "stdout: {}", paths.stdout);

    let auth = sandbox.clankers().args(["auth", "status"]).run();
    auth.assert_success();
    let combined = auth.combined();
    assert!(
        combined.contains("No authentication")
            || combined.contains("Accounts:")
            || combined.contains("API key")
            || combined.contains("not authenticated"),
        "auth status should be structural and non-interactive: {combined}"
    );
}

#[test]
fn readiness_e2e_fake_provider_print_bash_read_find_and_json() {
    let sandbox = ReadinessSandbox::new();
    fs::write(sandbox.work.join("Cargo.toml"), "[package]\nname = \"clankers-readiness-fixture\"\n").unwrap();
    fs::create_dir_all(sandbox.work.join("src/nested")).unwrap();
    fs::write(sandbox.work.join("src/main.rs"), "fn main() {}\n").unwrap();
    fs::write(sandbox.work.join("src/nested/mod.rs"), "pub fn marker() {}\n").unwrap();

    let mut simple = sandbox.clankers();
    let simple = simple.env("CLANKERS_FAKE_PROVIDER", "1").args(["-p", "Reply with exactly one word: yes"]).run();
    simple.assert_success();
    assert!(simple.stdout.to_ascii_lowercase().contains("yes"), "stdout: {}", simple.stdout);

    let mut bash = sandbox.clankers();
    let bash = bash
        .current_dir(repo_root())
        .env("CLANKERS_FAKE_PROVIDER", "1")
        .args(["-p", "Use the bash tool to run: echo CLANKERS_TOOL_TEST_OK"])
        .timeout(Duration::from_secs(120))
        .run();
    bash.assert_success();
    assert!(bash.stdout.contains("CLANKERS_TOOL_TEST_OK"), "stdout: {}", bash.stdout);

    let mut read = sandbox.clankers();
    let read = read
        .current_dir(repo_root())
        .env("CLANKERS_FAKE_PROVIDER", "1")
        .args([
            "-p",
            "Use the read tool to read the file Cargo.toml and tell me the package name",
        ])
        .run();
    read.assert_success();
    assert!(read.stdout.contains("clankers"), "stdout: {}", read.stdout);

    let mut find = sandbox.clankers();
    let find = find
        .current_dir(repo_root())
        .env("CLANKERS_FAKE_PROVIDER", "1")
        .args(["-p", "Use the find tool to find files named 'mod.rs' under src/"])
        .run();
    find.assert_success();
    assert!(find.stdout.contains("mod.rs"), "stdout: {}", find.stdout);

    let mut json = sandbox.clankers();
    let json = json.env("CLANKERS_FAKE_PROVIDER", "1").args(["--mode", "json", "-p", "Say hello"]).run();
    json.assert_success();
    let parsed = json
        .stdout
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| serde_json::from_str::<Value>(line).expect("json mode should emit JSONL"))
        .collect::<Vec<_>>();
    assert!(!parsed.is_empty(), "json mode should emit at least one JSON line");
}

#[test]
fn readiness_e2e_fake_provider_write_edit_read_round_trip() {
    let sandbox = ReadinessSandbox::new();
    let file = std::env::temp_dir().join(format!("clankers-e2e-write-test-{}", std::process::id()));
    let _ = fs::remove_file(&file);
    let file_arg = file.to_string_lossy();

    let mut write = sandbox.clankers();
    let write = write
        .current_dir(repo_root())
        .env("CLANKERS_FAKE_PROVIDER", "1")
        .args([
            "-p",
            &format!("Use the write tool to create the file {file_arg} with content 'hello world'."),
        ])
        .timeout(Duration::from_secs(120))
        .run();
    write.assert_success();
    assert!(
        file.is_file(),
        "fake provider write should create fixture file at {}\nstdout:\n{}\nstderr:\n{}",
        file.display(),
        write.stdout,
        write.stderr
    );

    let mut edit = sandbox.clankers();
    let edit = edit
        .current_dir(repo_root())
        .env("CLANKERS_FAKE_PROVIDER", "1")
        .args([
            "-p",
            &format!("Use the edit tool to replace 'world' with 'clankers' in {file_arg}."),
        ])
        .timeout(Duration::from_secs(120))
        .run();
    edit.assert_success();

    let content = fs::read_to_string(&file).expect("fake provider write/edit should leave fixture readable");
    assert!(content.contains("hello clankers"), "file content: {content}");

    let mut read = sandbox.clankers();
    let read = read
        .current_dir(repo_root())
        .env("CLANKERS_FAKE_PROVIDER", "1")
        .args([
            "-p",
            &format!("Use the read tool to read {file_arg} and show me the final content."),
        ])
        .timeout(Duration::from_secs(120))
        .run();
    read.assert_success();
    assert!(read.stdout.contains("clankers"), "stdout: {}", read.stdout);
}
