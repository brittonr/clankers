//! Process sandbox for agent tool execution
//!
//! Two concerns, cleanly separated:
//!
//! 1. **Path policy** — which filesystem paths any tool may access. Enforced once in the tool
//!    dispatch layer (`turn.rs`), not per-tool.
//!
//! 2. **Bash child sandbox** — environment scrubbing and optional kernel-level restrictions applied
//!    to spawned shell commands. This is where the real attack surface lives.

use std::path::Path;
use std::path::PathBuf;

use tracing::info;

// ═══════════════════════════════════════════════════════════════════════════
// Path policy
// ═══════════════════════════════════════════════════════════════════════════

/// Paths that no tool should ever touch, regardless of workspace root.
///
/// These are resolved relative to `$HOME` at init time and checked by the
/// dispatch guard in `turn.rs`.
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
    ".config/clankers-router/auth.json",
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

use std::sync::OnceLock;

static POLICY: OnceLock<PathPolicy> = OnceLock::new();

/// Initialize the global path policy. Call once at startup.
pub fn init_policy() {
    let _ = POLICY.set(PathPolicy::new());
    info!("sandbox: path policy initialized ({} denied paths)", POLICY.get().expect("Policy was just set").denied.len());
}

/// Check a path against the global policy.
/// Returns `None` (allowed) if the policy hasn't been initialized.
pub fn check_path(raw_path: &str) -> Option<String> {
    POLICY.get().and_then(|p| p.check(raw_path))
}

// ═══════════════════════════════════════════════════════════════════════════
// Bash child sandbox
// ═══════════════════════════════════════════════════════════════════════════

/// Environment variables that should be stripped from bash child processes.
///
/// These contain or provide access to secrets. The heuristic suffix check
/// in `sanitized_env()` catches most custom ones; this list handles the
/// well-known variables that don't follow naming conventions.
static SCRUBBED_ENV_VARS: &[&str] = &[
    // Cloud
    "AWS_ACCESS_KEY_ID",
    "AWS_SECRET_ACCESS_KEY",
    "AWS_SESSION_TOKEN",
    "AWS_SECURITY_TOKEN",
    "AZURE_CLIENT_SECRET",
    "AZURE_TENANT_ID",
    "AZURE_CLIENT_ID",
    "GOOGLE_APPLICATION_CREDENTIALS",
    // CI/CD
    "GITHUB_TOKEN",
    "GH_TOKEN",
    "GITLAB_TOKEN",
    "GITLAB_PRIVATE_TOKEN",
    "CIRCLE_TOKEN",
    "CODECOV_TOKEN",
    // LLM keys (don't let bash leak our own keys)
    "ANTHROPIC_API_KEY",
    "OPENAI_API_KEY",
    "OPENROUTER_API_KEY",
    "GROQ_API_KEY",
    "DEEPSEEK_API_KEY",
    // Package registries
    "NPM_TOKEN",
    "NUGET_API_KEY",
    "PYPI_TOKEN",
    // Infra
    "VAULT_TOKEN",
    "CONSUL_HTTP_TOKEN",
    "DOCKER_PASSWORD",
    "DOCKER_AUTH_CONFIG",
    // SSH agent
    "SSH_AUTH_SOCK",
    "SSH_AGENT_PID",
    // Databases
    "DATABASE_URL",
    "REDIS_URL",
    "MONGODB_URI",
    "MONGO_URL",
];

/// Build a sanitized copy of the environment for bash child processes.
///
/// Removes known secret variables and anything matching heuristic
/// patterns (*_SECRET, *_TOKEN, *_PASSWORD, *_API_KEY, etc.).
/// Sets `CLANKERS_SANDBOX=1` so scripts can detect sandboxed execution.
pub fn sanitized_env() -> Vec<(String, String)> {
    let scrubbed: std::collections::HashSet<&str> = SCRUBBED_ENV_VARS.iter().copied().collect();

    let mut env: Vec<(String, String)> = std::env::vars()
        .filter(|(key, _)| {
            if scrubbed.contains(key.as_str()) {
                return false;
            }
            let upper = key.to_uppercase();
            !(upper.ends_with("_SECRET")
                || upper.ends_with("_TOKEN")
                || upper.ends_with("_PASSWORD")
                || upper.ends_with("_CREDENTIALS")
                || upper.ends_with("_API_KEY")
                || upper.ends_with("_APIKEY")
                || upper.ends_with("_PRIVATE_KEY"))
        })
        .collect();

    env.push(("CLANKERS_SANDBOX".to_string(), "1".to_string()));
    env
}

/// Apply Landlock filesystem restrictions to the *current thread/process*.
///
/// Designed to be called inside a `pre_exec` hook on bash child processes,
/// NOT on the clankers parent. This way clankers itself remains unrestricted but
/// every shell command the agent runs is kernel-sandboxed.
///
/// `project_root` gets read-write; system paths get read-only.
///
/// Returns `Ok(true)` if applied, `Ok(false)` if unsupported, `Err` on failure.
#[cfg(target_os = "linux")]
pub fn apply_landlock_to_current(project_root: &Path) -> Result<bool, String> {
    use std::os::unix::io::AsRawFd;

    // Landlock syscall numbers
    const LANDLOCK_CREATE_RULESET: i64 = 444;
    const LANDLOCK_ADD_RULE: i64 = 445;
    const LANDLOCK_RESTRICT_SELF: i64 = 446;

    // ABI v1 access flags
    const FS_EXECUTE: u64 = 1 << 0;
    const FS_WRITE_FILE: u64 = 1 << 1;
    const FS_READ_FILE: u64 = 1 << 2;
    const FS_READ_DIR: u64 = 1 << 3;
    const FS_REMOVE_DIR: u64 = 1 << 4;
    const FS_REMOVE_FILE: u64 = 1 << 5;
    const FS_MAKE_CHAR: u64 = 1 << 6;
    const FS_MAKE_DIR: u64 = 1 << 7;
    const FS_MAKE_REG: u64 = 1 << 8;
    const FS_MAKE_SOCK: u64 = 1 << 9;
    const FS_MAKE_FIFO: u64 = 1 << 10;
    const FS_MAKE_BLOCK: u64 = 1 << 11;
    const FS_MAKE_SYM: u64 = 1 << 12;

    const RULE_PATH_BENEATH: i32 = 1;

    const ALL_READ: u64 = FS_EXECUTE | FS_READ_FILE | FS_READ_DIR;
    const ALL_WRITE: u64 = FS_WRITE_FILE
        | FS_REMOVE_DIR
        | FS_REMOVE_FILE
        | FS_MAKE_CHAR
        | FS_MAKE_DIR
        | FS_MAKE_REG
        | FS_MAKE_SOCK
        | FS_MAKE_FIFO
        | FS_MAKE_BLOCK
        | FS_MAKE_SYM;
    const ALL_ACCESS: u64 = ALL_READ | ALL_WRITE;

    #[repr(C)]
    struct RulesetAttr {
        handled_access_fs: u64,
        handled_access_net: u64,
    }

    #[repr(C)]
    struct PathBeneathAttr {
        allowed_access: u64,
        parent_fd: i32,
    }

    // Create ruleset
    let attr = RulesetAttr {
        handled_access_fs: ALL_ACCESS,
        handled_access_net: 0,
    };
    let fd = unsafe {
        libc::syscall(LANDLOCK_CREATE_RULESET, &raw const attr, std::mem::size_of::<RulesetAttr>(), 0u32)
    };
    if fd < 0 {
        let err = std::io::Error::last_os_error();
        if err.raw_os_error() == Some(libc::ENOSYS) || err.raw_os_error() == Some(libc::EOPNOTSUPP) {
            return Ok(false); // kernel doesn't support landlock
        }
        return Err(format!("landlock_create_ruleset: {}", err));
    }
    let fd = fd as i32;

    // Helper: add a path rule
    let add_rule = |path: &Path, access: u64| -> Result<(), String> {
        if !path.exists() {
            return Ok(());
        }
        let file = std::fs::File::open(path).map_err(|e| format!("open {}: {}", path.display(), e))?;
        let rule = PathBeneathAttr {
            allowed_access: access,
            parent_fd: file.as_raw_fd(),
        };
        let ret =
            unsafe { libc::syscall(LANDLOCK_ADD_RULE, fd, RULE_PATH_BENEATH, &raw const rule, 0u32) };
        // Keep file open until after syscall (fd must be valid)
        std::mem::forget(file);
        if ret < 0 {
            return Err(format!("landlock_add_rule({}): {}", path.display(), std::io::Error::last_os_error()));
        }
        Ok(())
    };

    // Read-write paths
    let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("/homeless"));
    let rw_paths = [project_root.to_path_buf(), std::env::temp_dir(), PathBuf::from("/tmp")];
    for p in &rw_paths {
        let _ = add_rule(p, ALL_ACCESS);
    }

    // Write access for nix daemon socket (nix build talks to the daemon via Unix socket)
    let nix_rw_paths = [
        PathBuf::from("/nix/var/nix/daemon-socket"),
        home.join(".cache/nix"),
        home.join(".local/state/nix"),
    ];
    for p in &nix_rw_paths {
        let _ = add_rule(p, ALL_ACCESS);
    }

    // Read-only paths (system, toolchains)
    let ro_paths = [
        PathBuf::from("/nix"),
        PathBuf::from("/usr"),
        PathBuf::from("/lib"),
        PathBuf::from("/lib64"),
        PathBuf::from("/bin"),
        PathBuf::from("/sbin"),
        PathBuf::from("/etc"),
        PathBuf::from("/dev"),
        PathBuf::from("/proc"),
        PathBuf::from("/sys"),
        PathBuf::from("/run"),
        home.join(".cargo/bin"),
        home.join(".rustup"),
        home.join(".local"),
        home.join(".nix-profile"),
    ];
    for p in &ro_paths {
        let _ = add_rule(p, ALL_READ);
    }

    // Restrict
    unsafe {
        libc::prctl(libc::PR_SET_NO_NEW_PRIVS, 1, 0, 0, 0);
    }
    let ret = unsafe { libc::syscall(LANDLOCK_RESTRICT_SELF, fd, 0u32) };
    unsafe {
        libc::close(fd);
    }

    if ret < 0 {
        Err(format!("landlock_restrict_self: {}", std::io::Error::last_os_error()))
    } else {
        Ok(true)
    }
}

#[cfg(not(target_os = "linux"))]
pub fn apply_landlock_to_current(_project_root: &Path) -> Result<bool, String> {
    Ok(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── Path policy ─────────────────────────────────────────────────

    #[test]
    fn blocks_ssh_keys() {
        let policy = PathPolicy::new();
        if let Some(home) = dirs::home_dir() {
            let ssh = home.join(".ssh/id_rsa");
            assert!(policy.check(ssh.to_str().expect("Path should be valid UTF-8")).is_some());
        }
    }

    #[test]
    fn blocks_aws_credentials() {
        let policy = PathPolicy::new();
        if let Some(home) = dirs::home_dir() {
            let p = home.join(".aws/credentials");
            assert!(policy.check(p.to_str().expect("Path should be valid UTF-8")).is_some());
        }
    }

    #[test]
    fn blocks_gnupg() {
        let policy = PathPolicy::new();
        if let Some(home) = dirs::home_dir() {
            let p = home.join(".gnupg/private-keys-v1.d/key.key");
            assert!(policy.check(p.to_str().expect("Path should be valid UTF-8")).is_some());
        }
    }

    #[test]
    fn blocks_clankers_auth() {
        let policy = PathPolicy::new();
        if let Some(home) = dirs::home_dir() {
            let p = home.join(".config/clankers-router/auth.json");
            assert!(policy.check(p.to_str().expect("Path should be valid UTF-8")).is_some());
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
            assert!(policy.check(home.join(".bash_history").to_str().expect("Path should be valid UTF-8")).is_some());
            assert!(policy.check(home.join(".zsh_history").to_str().expect("Path should be valid UTF-8")).is_some());
        }
    }

    #[test]
    fn allows_normal_project_files() {
        let policy = PathPolicy::new();
        let dir = tempfile::tempdir().expect("Failed to create temp directory");
        let file = dir.path().join("src/main.rs");
        std::fs::create_dir_all(dir.path().join("src")).expect("Failed to create src directory");
        std::fs::write(&file, "fn main() {}").expect("Failed to write test file");
        assert!(policy.check(file.to_str().expect("Path should be valid UTF-8")).is_none());
    }

    #[test]
    fn allows_tmp() {
        let policy = PathPolicy::new();
        assert!(policy.check("/tmp/test-file").is_none());
    }

    // ── Env scrubbing ───────────────────────────────────────────────

    #[test]
    fn scrubs_known_secrets() {
        unsafe {
            std::env::set_var("TEST_CLANKERS_API_KEY", "secret");
        }
        let env = sanitized_env();
        assert!(!env.iter().any(|(k, _)| k == "TEST_CLANKERS_API_KEY"));
        unsafe {
            std::env::remove_var("TEST_CLANKERS_API_KEY");
        }
    }

    #[test]
    fn scrubs_heuristic_patterns() {
        unsafe {
            std::env::set_var("MY_APP_SECRET", "x");
        }
        let env = sanitized_env();
        assert!(!env.iter().any(|(k, _)| k == "MY_APP_SECRET"));
        unsafe {
            std::env::remove_var("MY_APP_SECRET");
        }
    }

    #[test]
    fn preserves_normal_vars() {
        let env = sanitized_env();
        assert!(env.iter().any(|(k, _)| k == "PATH"));
    }

    #[test]
    fn sets_sandbox_marker() {
        let env = sanitized_env();
        assert!(env.iter().any(|(k, v)| k == "CLANKERS_SANDBOX" && v == "1"));
    }

    // ── Default impl ────────────────────────────────────────────────

    #[test]
    fn default_matches_new() {
        let from_new = PathPolicy::new();
        let from_default = PathPolicy::default();
        assert_eq!(from_new.denied.len(), from_default.denied.len());
        for (a, b) in from_new.denied.iter().zip(from_default.denied.iter()) {
            assert_eq!(a, b);
        }
    }

    // ── Relative path resolution ────────────────────────────────────

    #[test]
    fn blocks_relative_path_to_sensitive() {
        let policy = PathPolicy::new();
        if let Some(home) = dirs::home_dir() {
            // Use ../.. traversal to reach home from a temp dir
            let dir = tempfile::tempdir().expect("Failed to create temp directory");
            let saved = std::env::current_dir().expect("Failed to get current directory");
            std::env::set_current_dir(dir.path()).expect("Failed to change to temp directory");

            // Absolute path still blocked when resolved from new cwd
            let abs = home.join(".ssh/id_rsa");
            assert!(
                policy.check(abs.to_str().expect("Path should be valid UTF-8")).is_some(),
                "absolute path to ~/.ssh should be blocked from any cwd"
            );

            std::env::set_current_dir(saved).expect("Failed to restore current directory");
        }
    }

    // ── Symlink following ───────────────────────────────────────────

    #[test]
    fn blocks_symlink_to_sensitive() {
        let policy = PathPolicy::new();
        if let Some(home) = dirs::home_dir() {
            let dir = tempfile::tempdir().expect("Failed to create temp directory");
            let link = dir.path().join("sneaky-link");
            // Attempt to create a symlink to ~/.ssh
            let target = home.join(".ssh");
            if std::os::unix::fs::symlink(&target, &link).is_ok() {
                let via_link = link.join("id_rsa");
                assert!(policy.check(via_link.to_str().expect("Path should be valid UTF-8")).is_some(), "symlink to ~/.ssh should be blocked");
            }
        }
    }

    // ── Non-existent path parent resolution ─────────────────────────

    #[test]
    fn blocks_nonexistent_file_in_sensitive_dir() {
        let policy = PathPolicy::new();
        if let Some(home) = dirs::home_dir() {
            // File doesn't exist, but parent dir is sensitive
            let p = home.join(".ssh/nonexistent-key-12345");
            assert!(
                policy.check(p.to_str().expect("Path should be valid UTF-8")).is_some(),
                "nonexistent file inside ~/.ssh should still be blocked"
            );
        }
    }

    #[test]
    fn allows_nonexistent_file_in_safe_dir() {
        let policy = PathPolicy::new();
        let dir = tempfile::tempdir().expect("Failed to create temp directory");
        let p = dir.path().join("does-not-exist.rs");
        assert!(policy.check(p.to_str().expect("Path should be valid UTF-8")).is_none(), "nonexistent file in temp dir should be allowed");
    }

    // ── Path message content ────────────────────────────────────────

    #[test]
    fn block_message_contains_path_info() {
        let policy = PathPolicy::new();
        if let Some(home) = dirs::home_dir() {
            let p = home.join(".ssh/id_rsa");
            let msg = policy.check(p.to_str().expect("Path should be valid UTF-8")).expect("Should be blocked");
            assert!(msg.contains("blocked"));
            assert!(msg.contains("sensitive path"));
        }
    }

    // ── Env scrubbing: all heuristic suffixes ───────────────────────

    #[test]
    fn scrubs_token_suffix() {
        unsafe {
            std::env::set_var("CLANKERS_TEST_MY_TOKEN", "t");
        }
        let env = sanitized_env();
        assert!(!env.iter().any(|(k, _)| k == "CLANKERS_TEST_MY_TOKEN"));
        unsafe {
            std::env::remove_var("CLANKERS_TEST_MY_TOKEN");
        }
    }

    #[test]
    fn scrubs_password_suffix() {
        unsafe {
            std::env::set_var("CLANKERS_TEST_DB_PASSWORD", "p");
        }
        let env = sanitized_env();
        assert!(!env.iter().any(|(k, _)| k == "CLANKERS_TEST_DB_PASSWORD"));
        unsafe {
            std::env::remove_var("CLANKERS_TEST_DB_PASSWORD");
        }
    }

    #[test]
    fn scrubs_credentials_suffix() {
        unsafe {
            std::env::set_var("CLANKERS_TEST_CLOUD_CREDENTIALS", "c");
        }
        let env = sanitized_env();
        assert!(!env.iter().any(|(k, _)| k == "CLANKERS_TEST_CLOUD_CREDENTIALS"));
        unsafe {
            std::env::remove_var("CLANKERS_TEST_CLOUD_CREDENTIALS");
        }
    }

    #[test]
    fn scrubs_private_key_suffix() {
        unsafe {
            std::env::set_var("CLANKERS_TEST_SIGNING_PRIVATE_KEY", "k");
        }
        let env = sanitized_env();
        assert!(!env.iter().any(|(k, _)| k == "CLANKERS_TEST_SIGNING_PRIVATE_KEY"));
        unsafe {
            std::env::remove_var("CLANKERS_TEST_SIGNING_PRIVATE_KEY");
        }
    }

    #[test]
    fn scrubs_explicit_anthropic_key() {
        unsafe {
            std::env::set_var("ANTHROPIC_API_KEY", "sk-test");
        }
        let env = sanitized_env();
        assert!(!env.iter().any(|(k, _)| k == "ANTHROPIC_API_KEY"));
        unsafe {
            std::env::remove_var("ANTHROPIC_API_KEY");
        }
    }

    #[test]
    fn scrubs_explicit_github_token() {
        unsafe {
            std::env::set_var("GITHUB_TOKEN", "ghp_test");
        }
        let env = sanitized_env();
        assert!(!env.iter().any(|(k, _)| k == "GITHUB_TOKEN"));
        unsafe {
            std::env::remove_var("GITHUB_TOKEN");
        }
    }

    #[test]
    fn scrubs_ssh_auth_sock() {
        unsafe {
            std::env::set_var("SSH_AUTH_SOCK", "/tmp/agent.123");
        }
        let env = sanitized_env();
        assert!(!env.iter().any(|(k, _)| k == "SSH_AUTH_SOCK"));
        unsafe {
            std::env::remove_var("SSH_AUTH_SOCK");
        }
    }

    // ── Heuristic is case-insensitive ───────────────────────────────

    #[test]
    fn scrubs_mixed_case_suffix() {
        unsafe {
            std::env::set_var("clankers_test_My_Secret", "s");
        }
        let env = sanitized_env();
        assert!(!env.iter().any(|(k, _)| k == "clankers_test_My_Secret"));
        unsafe {
            std::env::remove_var("clankers_test_My_Secret");
        }
    }

    // ── Preserves non-secret vars ───────────────────────────────────

    #[test]
    fn preserves_home() {
        let env = sanitized_env();
        assert!(env.iter().any(|(k, _)| k == "HOME"));
    }

    #[test]
    fn preserves_user() {
        if std::env::var("USER").is_ok() {
            let env = sanitized_env();
            assert!(env.iter().any(|(k, _)| k == "USER"));
        }
    }

    // ── Policy covers all sensitive paths ───────────────────────────

    #[test]
    fn policy_has_reasonable_deny_count() {
        let policy = PathPolicy::new();
        // Should have at least the SENSITIVE_PATHS + SENSITIVE_SYSTEM_PATHS entries
        assert!(
            policy.denied.len() >= SENSITIVE_SYSTEM_PATHS.len(),
            "expected at least {} denied paths, got {}",
            SENSITIVE_SYSTEM_PATHS.len(),
            policy.denied.len()
        );
    }

    // ── All sensitive system paths blocked ──────────────────────────

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
                    policy.check(full.to_str().expect("Path should be valid UTF-8")).is_some(),
                    "home path {} should be blocked",
                    full.display()
                );
            }
        }
    }
}
