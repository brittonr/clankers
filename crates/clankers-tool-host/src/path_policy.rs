//! Security policy: path access control for sensitive files.
//!
//! Maintains a global deny-list of paths that tools should never read or write.

use std::path::PathBuf;

/// Paths resolved relative to `$HOME` that no tool should ever touch.
static SENSITIVE_PATHS: &[&str] = &[
    ".ssh",
    ".gnupg",
    ".aws/credentials",
    ".aws/config",
    ".config/gcloud",
    ".azure",
    ".kube/config",
    ".docker/config.json",
    ".npmrc",
    ".pypirc",
    ".cargo/credentials",
    ".cargo/credentials.toml",
    ".gem/credentials",
    ".terraform.d/credentials.tfrc.json",
    ".vault-token",
    ".config/op",
    ".config/gh",
    ".netrc",
    ".config/clanker-router/auth.json",
    ".clankers/agent/auth.json",
    ".pi/agent/auth.json",
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
    #[must_use]
    pub fn new() -> Self {
        let mut denied = Vec::with_capacity(SENSITIVE_PATHS.len().saturating_add(SENSITIVE_SYSTEM_PATHS.len()));

        if let Some(home) = dirs::home_dir() {
            for rel in SENSITIVE_PATHS {
                let path = home.join(rel);
                denied.push(path.canonicalize().unwrap_or(path));
            }
        }

        for path in SENSITIVE_SYSTEM_PATHS {
            let path = PathBuf::from(path);
            denied.push(path.canonicalize().unwrap_or(path));
        }

        Self { denied }
    }

    /// Check whether a path is blocked.
    ///
    /// Returns `Some(reason)` if denied, `None` if allowed.
    pub fn check(&self, raw_path: &str) -> Option<String> {
        let path = if let Some(relative_home_path) = raw_path.strip_prefix("~/") {
            dirs::home_dir()
                .map(|home| home.join(relative_home_path))
                .unwrap_or_else(|| PathBuf::from(raw_path))
        } else {
            PathBuf::from(raw_path)
        };

        let absolute = if path.is_absolute() {
            path
        } else {
            std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")).join(path)
        };

        let canonical = if absolute.exists() {
            absolute.canonicalize().unwrap_or_else(|_| absolute.clone())
        } else if let Some(parent) = absolute.parent()
            && parent.exists()
        {
            let canonical_parent = parent.canonicalize().unwrap_or_else(|_| parent.to_path_buf());
            canonical_parent.join(absolute.file_name().unwrap_or_default())
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

/// Initialize the path policy compatibility hook.
pub fn init_policy() {}

/// Check a path against the standard policy.
pub fn check_path(raw_path: &str) -> Option<String> {
    PathPolicy::new().check(raw_path)
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
    fn expands_home_relative_sensitive_paths() {
        let policy = PathPolicy::new();
        if dirs::home_dir().is_some() {
            assert!(policy.check("~/.ssh/id_rsa").is_some());
            assert!(policy.check("~/.aws/credentials").is_some());
        }
    }

    #[test]
    fn blocks_system_paths() {
        let policy = PathPolicy::new();
        for path in SENSITIVE_SYSTEM_PATHS {
            assert!(policy.check(path).is_some(), "system path {path} should be blocked");
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
    fn blocks_nonexistent_file_in_sensitive_dir() {
        let policy = PathPolicy::new();
        if let Some(home) = dirs::home_dir() {
            let path = home.join(".ssh/nonexistent-key-12345");
            assert!(policy.check(path.to_str().unwrap()).is_some());
        }
    }
}
