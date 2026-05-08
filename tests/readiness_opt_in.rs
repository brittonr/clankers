mod readiness_common;

use std::process::Command;
use std::time::Duration;

use readiness_common::ReadinessCommand;
use readiness_common::repo_root;
use readiness_common::skip_unless_env;

const VM_CHECKS: &[&str] = &[
    "vm-smoke",
    "vm-remote-daemon",
    "vm-session-recovery",
    "vm-plugin-runtime",
    "vm-module-daemon",
    "vm-module-router",
    "vm-module-integration",
];

#[test]
fn readiness_live_local_model_aspen2_qwen36_nextest_opt_in() {
    if skip_unless_env("CLANKERS_RUN_LIVE_READINESS") {
        return;
    }
    let mut command = ReadinessCommand::new("cargo");
    command
        .current_dir(repo_root())
        .args([
            "nextest",
            "run",
            "-p",
            "clankers",
            "--test",
            "aspen2_qwen36_integration",
            "--no-fail-fast",
        ])
        .timeout(Duration::from_secs(900));
    command.run().assert_success();
}

#[test]
fn readiness_vm_required_nixos_checks_nextest_opt_in() {
    if skip_unless_env("CLANKERS_RUN_VM_READINESS") {
        return;
    }
    let system = current_system().expect("nix should report current system for VM readiness");
    for check in selected_vm_checks() {
        let attr = format!(".#checks.{system}.{check}");
        println!("running VM readiness check {attr}");
        let mut command = ReadinessCommand::new("nix");
        let output = command
            .current_dir(repo_root())
            .args(["build", &attr, "--no-link", "-L"])
            .timeout(vm_check_timeout())
            .run();
        output.assert_success();
    }
}

#[test]
fn readiness_flake_ci_nextest_opt_in() {
    if skip_unless_env("CLANKERS_RUN_FLAKE_READINESS") {
        return;
    }
    let mut command = ReadinessCommand::new("nix");
    command
        .current_dir(repo_root())
        .args(["flake", "check"])
        .timeout(Duration::from_secs(1_800))
        .run()
        .assert_success();
}

fn selected_vm_checks() -> Vec<&'static str> {
    match std::env::var("CLANKERS_VM_READINESS_SELECTOR").unwrap_or_else(|_| "all".to_owned()).as_str() {
        "all" => VM_CHECKS.to_vec(),
        "core" => vec!["vm-smoke", "vm-remote-daemon", "vm-session-recovery"],
        "module" => vec!["vm-module-daemon", "vm-module-router", "vm-module-integration"],
        "smoke" => vec!["vm-smoke"],
        check if VM_CHECKS.contains(&check) => {
            vec![VM_CHECKS.iter().copied().find(|candidate| *candidate == check).unwrap()]
        }
        other => panic!("unknown CLANKERS_VM_READINESS_SELECTOR {other}"),
    }
}

fn vm_check_timeout() -> Duration {
    let selector = std::env::var("CLANKERS_VM_READINESS_SELECTOR").unwrap_or_else(|_| "all".to_owned());
    if selector == "all" {
        Duration::from_secs(3_600)
    } else {
        Duration::from_secs(1_800)
    }
}

fn current_system() -> Option<String> {
    let output = Command::new("nix")
        .args(["eval", "--raw", "--impure", "--expr", "builtins.currentSystem"])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    Some(String::from_utf8_lossy(&output.stdout).trim().to_owned())
}
