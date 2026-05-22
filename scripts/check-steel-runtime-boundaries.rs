#!/usr/bin/env -S nix develop -c cargo -q -Zscript
---cargo
[package]
edition = "2024"

[dependencies]
serde_json = "1"
---

use std::fs;
use std::path::Path;

fn main() {
    if let Err(error) = run() {
        eprintln!("steel runtime boundary check failed: {error}");
        std::process::exit(1);
    }
    println!(
        "steel runtime boundary check passed for r[steel-lisp-runtime.wrapper-owned-evaluation.no-shell-interpreter-leak]"
    );
}

fn run() -> Result<(), String> {
    let runtime = read("crates/clankers-runtime/src/steel_runtime.rs")?;
    require(&runtime, "STEEL_RUNTIME_RECEIPT_SCHEMA", "runtime receipt schema")?;
    require(&runtime, "evaluate_steel_request", "wrapper evaluation function")?;
    require(&runtime, "AmbientAuthorityDenied", "ambient authority denial")?;
    require(&runtime, "no OS/process sandbox claim", "no sandbox overclaim wording")?;

    let cli = read("src/commands/steel.rs")?;
    require(&cli, "evaluate_steel_request", "CLI uses runtime wrapper")?;
    require_absent(&cli, "steel::steel_vm", "CLI must not import Steel interpreter internals")?;
    require_absent(&cli, "Engine::new", "CLI must not construct interpreter internals")?;

    for path in shell_paths()? {
        let text = read(&path)?;
        if path != "src/commands/steel.rs" {
            require_absent(&text, "steel::steel_vm", &format!("{path} direct Steel import"))?;
            require_absent(&text, "steel_lang::", &format!("{path} direct Steel import"))?;
            require_absent(&text, "steel_core::", &format!("{path} direct Steel import"))?;
        }
    }
    Ok(())
}

fn shell_paths() -> Result<Vec<String>, String> {
    let mut paths = Vec::new();
    collect_rs(Path::new("src"), &mut paths)?;
    Ok(paths)
}

fn collect_rs(dir: &Path, paths: &mut Vec<String>) -> Result<(), String> {
    for entry in fs::read_dir(dir).map_err(|error| format!("read {}: {error}", dir.display()))? {
        let entry = entry.map_err(|error| format!("read entry: {error}"))?;
        let path = entry.path();
        if path.is_dir() {
            collect_rs(&path, paths)?;
        } else if path.extension().and_then(|ext| ext.to_str()) == Some("rs") {
            paths.push(path.to_string_lossy().replace('\\', "/"));
        }
    }
    paths.sort();
    Ok(())
}

fn read(path: impl AsRef<Path>) -> Result<String, String> {
    fs::read_to_string(path.as_ref()).map_err(|error| format!("read {}: {error}", path.as_ref().display()))
}

fn require(text: &str, needle: &str, label: &str) -> Result<(), String> {
    if text.contains(needle) {
        Ok(())
    } else {
        Err(format!("missing {label}: `{needle}`"))
    }
}

fn require_absent(text: &str, needle: &str, label: &str) -> Result<(), String> {
    if text.contains(needle) {
        Err(format!("unexpected {label}: `{needle}`"))
    } else {
        Ok(())
    }
}
