#!/usr/bin/env -S nix develop -c cargo -q -Zscript
---cargo
[package]
edition = "2024"
---

use std::fs;
use std::process::Command;

const MANIFEST: &str = "crates/clankers-adapters/Cargo.toml";
const GUIDE: &str = "docs/src/tutorials/embedded-agent-sdk.md";
const ERROR_EXIT: i32 = 1;
const FORBIDDEN: &[&str] = &[
    "clankers-agent",
    "clankers-controller",
    "clankers-provider",
    "clanker-router",
    "clankers-db",
    "clankers-session",
    "clankers-protocol",
    "clankers-tui",
    "clankers-prompts",
    "clankers-skills",
    "clankers-config",
    "ratatui",
    "crossterm",
    "iroh",
];

fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
        std::process::exit(ERROR_EXIT);
    }
}

fn run() -> Result<(), String> {
    let manifest = fs::read_to_string(MANIFEST).map_err(|error| format!("failed to read {MANIFEST}: {error}"))?;
    if manifest.lines().any(|line| line.trim() == "[features]") {
        return Err(format!("{MANIFEST} must not declare optional features without embedded guide coverage"));
    }
    let guide = fs::read_to_string(GUIDE).map_err(|error| format!("failed to read {GUIDE}: {error}"))?;
    for phrase in [
        "`clankers-adapters`: no optional features",
        "## Product embedding crate guidance",
        "Daemon, MCP, and ACP integrations remain supported as **application-edge** surfaces",
        "Declarative tool catalogs are parser-neutral DTOs",
    ] {
        if !guide.contains(phrase) {
            return Err(format!("{GUIDE} missing embedded adapters policy phrase: {phrase}"));
        }
    }
    let output = Command::new("cargo")
        .arg("tree")
        .arg("-p")
        .arg("clankers-adapters")
        .arg("--edges")
        .arg("normal")
        .arg("--prefix")
        .arg("none")
        .output()
        .map_err(|error| format!("failed to run cargo tree for clankers-adapters: {error}"))?;
    if !output.status.success() {
        return Err(format!(
            "cargo tree for clankers-adapters failed\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    let tree = String::from_utf8_lossy(&output.stdout);
    for forbidden in FORBIDDEN {
        if tree.lines().any(|line| line.split_whitespace().next() == Some(*forbidden)) {
            return Err(format!("clankers-adapters dependency graph includes forbidden crate `{forbidden}`"));
        }
    }
    println!("ok: clankers-adapters dependency graph excludes forbidden runtime crates");
    Ok(())
}
