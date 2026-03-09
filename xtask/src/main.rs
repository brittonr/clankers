use std::env;
use std::fs;
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;
use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = env::args().skip(1).collect();

    match args.first().map(String::as_str) {
        Some("build-plugins") => build_plugins(args.get(1).map(String::as_str)),
        Some(cmd) => {
            eprintln!("unknown command: {cmd}");
            eprintln!();
            print_usage();
            ExitCode::FAILURE
        }
        None => {
            print_usage();
            ExitCode::FAILURE
        }
    }
}

fn print_usage() {
    eprintln!("usage: cargo xtask <command>");
    eprintln!();
    eprintln!("commands:");
    eprintln!("  build-plugins [filter]   Build all WASM plugins (or matching filter)");
}

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("xtask must live one level below repo root")
        .to_path_buf()
}

fn discover_plugins(root: &Path) -> Vec<PathBuf> {
    let dirs = ["plugins", "examples/plugins"];
    let mut plugins = Vec::new();
    for dir in dirs {
        let base = root.join(dir);
        if let Ok(entries) = fs::read_dir(&base) {
            for entry in entries.flatten() {
                let manifest = entry.path().join("Cargo.toml");
                if manifest.exists() {
                    plugins.push(manifest);
                }
            }
        }
    }
    plugins.sort();
    plugins
}

fn crate_name(manifest: &Path) -> Option<String> {
    let contents = fs::read_to_string(manifest).ok()?;
    for line in contents.lines() {
        let line = line.trim();
        if let Some(rest) = line.strip_prefix("name") {
            let rest = rest.trim().strip_prefix('=')?.trim();
            return Some(rest.trim_matches('"').to_string());
        }
    }
    None
}

fn build_plugins(filter: Option<&str>) -> ExitCode {
    let root = repo_root();
    let plugins = discover_plugins(&root);
    let mut built = 0u32;

    for manifest in &plugins {
        let name = match crate_name(manifest) {
            Some(n) => n,
            None => {
                eprintln!("warning: couldn't parse crate name from {}", manifest.display());
                continue;
            }
        };

        if let Some(f) = filter
            && !name.contains(f)
        {
            continue;
        }

        let dir = manifest.parent().unwrap();
        let wasm_stem = name.replace('-', "_");

        println!("building {name} → wasm32-unknown-unknown (release)…");

        let status = Command::new("cargo")
            .args([
                "build",
                "--manifest-path",
                &manifest.to_string_lossy(),
                "--target",
                "wasm32-unknown-unknown",
                "--release",
                "-Zbuild-std=std,panic_abort",
            ])
            .status();

        match status {
            Ok(s) if s.success() => {}
            Ok(s) => {
                eprintln!("cargo build failed for {name} (exit {})", s.code().unwrap_or(-1));
                return ExitCode::FAILURE;
            }
            Err(e) => {
                eprintln!("failed to run cargo: {e}");
                return ExitCode::FAILURE;
            }
        }

        let src = dir.join("target/wasm32-unknown-unknown/release").join(format!("{wasm_stem}.wasm"));
        let dst = dir.join(format!("{wasm_stem}.wasm"));

        if let Err(e) = fs::copy(&src, &dst) {
            eprintln!("failed to copy {}: {e}", src.display());
            return ExitCode::FAILURE;
        }

        let size = fs::metadata(&dst).map(|m| m.len()).unwrap_or(0);
        let rel = dst.strip_prefix(&root).unwrap_or(&dst);
        println!("  ✓ {:.0}K → {}", size as f64 / 1024.0, rel.display());
        built += 1;
    }

    if built == 0 {
        eprintln!("no plugins matched{}", filter.map_or(String::new(), |f| format!(" filter '{f}'")));
        return ExitCode::FAILURE;
    }

    println!("built {built} plugin(s).");
    ExitCode::SUCCESS
}
