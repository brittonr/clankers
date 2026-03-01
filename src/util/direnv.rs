//! One-time direnv environment loader.
//!
//! When clankers is launched from a shell with direnv hooks (the normal case),
//! the process already inherits the `.envrc` environment — nothing to do.
//!
//! When clankers is launched without direnv hooks (daemon, RPC, cron, desktop
//! launcher), we run `direnv export json` once at startup and inject the
//! resulting variables into the process environment so that all child
//! commands (bash tool, git, cargo, etc.) see them.

use std::collections::HashMap;
use std::path::Path;

use tracing::debug;
use tracing::info;
use tracing::warn;

/// Load the direnv environment into the current process if:
///   1. An `.envrc` exists at or above `cwd`, AND
///   2. The environment hasn't already been loaded (no `DIRENV_DIR` set).
///
/// This is designed to be called once, early in `main()`, before any
/// threads are spawned (so `set_var` is safe).
pub fn load_direnv_if_needed(cwd: &Path) {
    // If DIRENV_DIR is already set, the parent shell's direnv hook already
    // loaded the environment — skip to avoid double-loading.
    if std::env::var("DIRENV_DIR").is_ok() {
        debug!("direnv: environment already loaded (DIRENV_DIR set), skipping");
        return;
    }

    // Walk up from cwd looking for .envrc
    if !has_envrc(cwd) {
        debug!("direnv: no .envrc found at or above {}", cwd.display());
        return;
    }

    // Check that direnv is available
    let direnv_ok = std::process::Command::new("direnv")
        .arg("--version")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .is_ok_and(|s| s.success());

    if !direnv_ok {
        debug!("direnv: binary not found in PATH, skipping");
        return;
    }

    // Run `direnv export json` to get the environment diff
    let output = match std::process::Command::new("direnv")
        .args(["export", "json"])
        .current_dir(cwd)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .output()
    {
        Ok(o) => o,
        Err(e) => {
            warn!("direnv: failed to run `direnv export json`: {}", e);
            return;
        }
    };

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        // Common case: .envrc is blocked — not an error, just info
        if stderr.contains("is blocked") {
            info!("direnv: .envrc is blocked. Run `direnv allow` in {} to enable.", cwd.display());
        } else {
            warn!("direnv: `direnv export json` failed: {}", stderr.trim());
        }
        return;
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let json_str = stdout.trim();

    // direnv prints empty string when nothing changed
    if json_str.is_empty() {
        debug!("direnv: no environment changes");
        return;
    }

    let env_map: HashMap<String, serde_json::Value> = match serde_json::from_str(json_str) {
        Ok(m) => m,
        Err(e) => {
            warn!("direnv: failed to parse export json: {}", e);
            return;
        }
    };

    let mut count = 0;
    for (key, value) in &env_map {
        match value {
            serde_json::Value::String(val) => {
                // SAFETY: called early in main() before threads are spawned.
                unsafe { std::env::set_var(key, val) };
                count += 1;
            }
            serde_json::Value::Null => {
                // direnv uses null to indicate a variable should be unset
                unsafe { std::env::remove_var(key) };
                count += 1;
            }
            _ => {
                debug!("direnv: skipping non-string value for {}", key);
            }
        }
    }

    info!("direnv: loaded {} environment variables from .envrc", count);
}

/// Walk up from `start` looking for a `.envrc` file.
fn has_envrc(start: &Path) -> bool {
    let mut dir = start;
    loop {
        if dir.join(".envrc").exists() {
            return true;
        }
        match dir.parent() {
            Some(parent) => dir = parent,
            None => return false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn has_envrc_returns_false_for_root() {
        // The filesystem root almost certainly doesn't have a .envrc
        assert!(!has_envrc(Path::new("/")));
    }

    #[test]
    fn has_envrc_finds_file_in_current_dir() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join(".envrc"), "use flake").unwrap();
        assert!(has_envrc(tmp.path()));
    }

    #[test]
    fn has_envrc_finds_file_in_parent() {
        let tmp = tempfile::tempdir().unwrap();
        let child = tmp.path().join("subdir");
        std::fs::create_dir(&child).unwrap();
        std::fs::write(tmp.path().join(".envrc"), "use flake").unwrap();
        assert!(has_envrc(&child));
    }

    #[test]
    fn load_skips_when_direnv_dir_set() {
        // If DIRENV_DIR is already set, load_direnv_if_needed should be a no-op.
        // We can't easily test this without side effects, but we can verify
        // the function doesn't panic.
        let tmp = tempfile::tempdir().unwrap();
        load_direnv_if_needed(tmp.path());
    }
}
