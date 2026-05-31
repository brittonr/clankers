#!/usr/bin/env -S nix develop -c cargo -q -Zscript
---cargo
[package]
edition = "2024"
---

use std::fs;
use std::path::Path;
use std::process::ExitCode;

const ERROR_EXIT: u8 = 1;
const CONFIG_MANIFEST: &str = "crates/clankers-config/Cargo.toml";
const CONFIG_SRC: &str = "crates/clankers-config/src";
const TUI_ADAPTER: &str = "src/tui_config.rs";

const FORBIDDEN_MANIFEST_DEPS: &[&str] = &["clankers-tui", "ratatui", "terminal-colorsaurus"];
const FORBIDDEN_CONFIG_TOKENS: &[&str] = &[
    "clankers_tui::",
    "ratatui::",
    "terminal_colorsaurus::",
    "Keymap::build",
    "Theme::dark",
    "Theme::light",
    "Color::Rgb",
    "into_keymap",
];
const REQUIRED_ADAPTER_TOKENS: &[&str] = &[
    "pub fn theme_from_def",
    "pub fn load_theme",
    "pub fn detect_theme",
    "pub fn keymap_from_config",
    "clankers_config::theme::load_theme_def",
    "Keymap::build",
];

fn main() -> ExitCode {
    match run() {
        Ok(()) => {
            println!("ok: config/tui boundary rail passed");
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("config/tui boundary rail failed: {error}");
            ExitCode::from(ERROR_EXIT)
        }
    }
}

fn run() -> Result<(), String> {
    check_manifest()?;
    check_config_sources()?;
    check_tui_adapter()
}

fn check_manifest() -> Result<(), String> {
    let manifest =
        fs::read_to_string(CONFIG_MANIFEST).map_err(|error| format!("failed to read {CONFIG_MANIFEST}: {error}"))?;
    for token in FORBIDDEN_MANIFEST_DEPS {
        if manifest.contains(token) {
            return Err(format!("{CONFIG_MANIFEST} still depends on forbidden display crate `{token}`"));
        }
    }
    Ok(())
}

fn check_config_sources() -> Result<(), String> {
    for path in rust_files(Path::new(CONFIG_SRC))? {
        let text = fs::read_to_string(&path).map_err(|error| format!("failed to read {}: {error}", path.display()))?;
        for token in FORBIDDEN_CONFIG_TOKENS {
            if text.contains(token) {
                return Err(format!(
                    "{} contains forbidden display/projection token `{token}`; move projection to {TUI_ADAPTER}",
                    path.display()
                ));
            }
        }
    }
    Ok(())
}

fn check_tui_adapter() -> Result<(), String> {
    let adapter = fs::read_to_string(TUI_ADAPTER).map_err(|error| format!("failed to read {TUI_ADAPTER}: {error}"))?;
    for token in REQUIRED_ADAPTER_TOKENS {
        if !adapter.contains(token) {
            return Err(format!("{TUI_ADAPTER} missing required projection marker `{token}`"));
        }
    }
    Ok(())
}

fn rust_files(root: &Path) -> Result<Vec<std::path::PathBuf>, String> {
    let mut files = Vec::new();
    collect_rust_files(root, &mut files)?;
    files.sort();
    Ok(files)
}

fn collect_rust_files(path: &Path, out: &mut Vec<std::path::PathBuf>) -> Result<(), String> {
    for entry in fs::read_dir(path).map_err(|error| format!("failed to read dir {}: {error}", path.display()))? {
        let entry = entry.map_err(|error| format!("failed to read dir entry under {}: {error}", path.display()))?;
        let path = entry.path();
        if path.is_dir() {
            collect_rust_files(&path, out)?;
        } else if path.extension().and_then(|ext| ext.to_str()) == Some("rs") {
            out.push(path);
        }
    }
    Ok(())
}
