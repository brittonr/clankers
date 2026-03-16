//! Security policy: path access control for sensitive files.
//!
//! Maintains a global deny-list of paths (SSH keys, cloud credentials, etc.)
//! that tools should never read or write.

use std::path::Path;
use std::path::PathBuf;
use std::sync::OnceLock;

use tracing::info;

/// Paths resolved relative to `$HOME` that no tool should ever touch.
static SENSITIVE_PATHS: &[&str] = &[
    // Cryptographic keys
    ".ssh",
    ".gnupg",
    // Cloud credentials
    ".aws/credentials",
    ".aws/config",
    ".config/gcloud",
    ".azure",
    // Orchestration credentials
    ".kube/config",
    ".docker/config.json",
    // Language-ecosystem tokens
    ".npmrc",
    ".pypirc",
    ".cargo/credentials",
    ".cargo/credentials.toml",
    ".gem/credentials",
    // Infrastructure credentials
    ".terraform.d/credentials.tfrc.json",
    ".vault-token",
    // Password managers / auth CLIs
    ".config/op",
    ".config/gh",
    ".netrc",
    // Our own auth store
    ".config/clanker-router/auth.json",
    ".clankers/agent/auth.json",
    ".pi/agent/auth.json",
    // Shell history (may contain pasted secrets)
    ".bash_history",
    ".zsh_history",
    ".local/share/fish/fish_history",
];

static SENSITIVE_SYSTEM_PATHS: &[&str] = &["/etc/shadow", "/etc/sudoers", "/etc/ssh", "/root"];

/// Resolved deny-list, built once at startup.
#[derive(Debug, Clone)]
pub struct PathPolicy {
    denied: Vec<PathBuf>,
}

impl Default for PathPolicy {
    fn default() -> Self {
        Self::new()
    }
}

impl PathPolicy {
    /// Build the policy from well-known sensitive paths.
    pub fn new() -> Self {
        let mut denied = Vec::new();

        if let Some(home) = dirs::home_dir() {
            for rel in SENSITIVE_PATHS {
                let p = home.join(rel);
                denied.push(p.canonicalize().unwrap_or(p));
            }
        }

        for p in SENSITIVE_SYSTEM_PATHS {
            let p = PathBuf::from(p);
            denied.push(p.canonicalize().unwrap_or(p));
        }

        Self { denied }
    }

    /// Check whether a path is blocked.
    ///
    /// Returns `Some(reason)` if denied, `None` if allowed.
    pub fn check(&self, raw_path: &str) -> Option<String> {
        let path = Path::new(raw_path);

        // Resolve to absolute
        let absolute = if path.is_absolute() {
            path.to_path_buf()
        } else {
            std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")).join(path)
        };

        // Canonicalize (follow symlinks). Try the file itself, then its parent
        // (for files that don't exist yet).
        let canonical = if absolute.exists() {
            absolute.canonicalize().unwrap_or_else(|_| absolute.clone())
        } else if let Some(parent) = absolute.parent()
            && parent.exists()
        {
            let pc = parent.canonicalize().unwrap_or_else(|_| parent.to_path_buf());
            pc.join(absolute.file_name().unwrap_or_default())
        } else {
            absolute
        };

        for denied in &self.denied {
            if canonical.starts_with(denied) {
                return Some(format!("blocked: {} is inside sensitive path {}", raw_path, denied.display()));
            }
        }

        None
    }
}

// ─── Global instance ───────────────────────────────────────────────────────

static POLICY: OnceLock<PathPolicy> = OnceLock::new();

/// Initialize the global path policy. Call once at startup.
pub fn init_policy() {
    let _ = POLICY.set(PathPolicy::new());
    info!(
        "sandbox: path policy initialized ({} denied paths)",
        POLICY.get().expect("Policy was just set").denied.len()
    );
}

/// Check a path against the global policy.
/// Returns `None` (allowed) if the policy hasn't been initialized.
pub fn check_path(raw_path: &str) -> Option<String> {
    POLICY.get().and_then(|p| p.check(raw_path))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blocks_ssh_keys() {
        let policy = PathPolicy::new();
        if let Some(home) = dirs::home_dir() {
            let ssh = home.join(".ssh/id_rsa");
            assert!(policy.check(ssh.to_str().unwrap()).is_some());
        }
    }

    #[test]
    fn blocks_aws_credentials() {
        let policy = PathPolicy::new();
        if let Some(home) = dirs::home_dir() {
            let p = home.join(".aws/credentials");
            assert!(policy.check(p.to_str().unwrap()).is_some());
        }
    }

    #[test]
    fn blocks_gnupg() {
        let policy = PathPolicy::new();
        if let Some(home) = dirs::home_dir() {
            let p = home.join(".gnupg/private-keys-v1.d/key.key");
            assert!(policy.check(p.to_str().unwrap()).is_some());
        }
    }

    #[test]
    fn blocks_clankers_ucan() {
        let policy = PathPolicy::new();
        if let Some(home) = dirs::home_dir() {
            let p = home.join(".config/clanker-router/auth.json");
            assert!(policy.check(p.to_str().unwrap()).is_some());
        }
    }

    #[test]
    fn blocks_etc_shadow() {
        let policy = PathPolicy::new();
        assert!(policy.check("/etc/shadow").is_some());
    }

    #[test]
    fn blocks_shell_history() {
        let policy = PathPolicy::new();
        if let Some(home) = dirs::home_dir() {
            assert!(policy.check(home.join(".bash_history").to_str().unwrap()).is_some());
            assert!(policy.check(home.join(".zsh_history").to_str().unwrap()).is_some());
        }
    }

    #[test]
    fn allows_normal_project_files() {
        let policy = PathPolicy::new();
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("src/main.rs");
        std::fs::create_dir_all(dir.path().join("src")).unwrap();
        std::fs::write(&file, "fn main() {}").unwrap();
        assert!(policy.check(file.to_str().unwrap()).is_none());
    }

    #[test]
    fn allows_tmp() {
        let policy = PathPolicy::new();
        assert!(policy.check("/tmp/test-file").is_none());
    }

    #[test]
    fn default_matches_new() {
        let from_new = PathPolicy::new();
        let from_default = PathPolicy::default();
        assert_eq!(from_new.denied.len(), from_default.denied.len());
    }

    #[test]
    fn blocks_relative_path_to_sensitive() {
        let policy = PathPolicy::new();
        if let Some(home) = dirs::home_dir() {
            let dir = tempfile::tempdir().unwrap();
            let saved = std::env::current_dir().unwrap();
            std::env::set_current_dir(dir.path()).unwrap();
            let abs = home.join(".ssh/id_rsa");
            assert!(policy.check(abs.to_str().unwrap()).is_some());
            std::env::set_current_dir(saved).unwrap();
        }
    }

    #[test]
    fn blocks_symlink_to_sensitive() {
        let policy = PathPolicy::new();
        if let Some(home) = dirs::home_dir() {
            let dir = tempfile::tempdir().unwrap();
            let link = dir.path().join("sneaky-link");
            let target = home.join(".ssh");
            if std::os::unix::fs::symlink(&target, &link).is_ok() {
                let via_link = link.join("id_rsa");
                assert!(policy.check(via_link.to_str().unwrap()).is_some());
            }
        }
    }

    #[test]
    fn blocks_nonexistent_file_in_sensitive_dir() {
        let policy = PathPolicy::new();
        if let Some(home) = dirs::home_dir() {
            let p = home.join(".ssh/nonexistent-key-12345");
            assert!(policy.check(p.to_str().unwrap()).is_some());
        }
    }

    #[test]
    fn allows_nonexistent_file_in_safe_dir() {
        let policy = PathPolicy::new();
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("does-not-exist.rs");
        assert!(policy.check(p.to_str().unwrap()).is_none());
    }

    #[test]
    fn block_message_contains_path_info() {
        let policy = PathPolicy::new();
        if let Some(home) = dirs::home_dir() {
            let p = home.join(".ssh/id_rsa");
            let msg = policy.check(p.to_str().unwrap()).unwrap();
            assert!(msg.contains("blocked"));
            assert!(msg.contains("sensitive path"));
        }
    }

    #[test]
    fn policy_has_reasonable_deny_count() {
        let policy = PathPolicy::new();
        assert!(
            policy.denied.len() >= SENSITIVE_SYSTEM_PATHS.len(),
            "expected at least {} denied paths, got {}",
            SENSITIVE_SYSTEM_PATHS.len(),
            policy.denied.len()
        );
    }

    #[test]
    fn blocks_all_system_paths() {
        let policy = PathPolicy::new();
        for path in SENSITIVE_SYSTEM_PATHS {
            assert!(policy.check(path).is_some(), "system path {} should be blocked", path);
        }
    }

    #[test]
    fn blocks_all_home_sensitive_paths() {
        let policy = PathPolicy::new();
        if let Some(home) = dirs::home_dir() {
            for rel in SENSITIVE_PATHS {
                let full = home.join(rel);
                assert!(
                    policy.check(full.to_str().unwrap()).is_some(),
                    "home path {} should be blocked",
                    full.display()
                );
            }
        }
    }
}
