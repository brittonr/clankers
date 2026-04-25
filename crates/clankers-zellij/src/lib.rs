//! Zellij integration and orchestration

pub mod streaming;

/// Error type for Zellij operations.
#[derive(Debug, Clone)]
pub struct ZellijError {
    pub message: String,
}

impl std::fmt::Display for ZellijError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "zellij: {}", self.message)
    }
}

impl std::error::Error for ZellijError {}

use std::path::Path;

/// Check if we're inside an active Zellij session.
///
/// Checks `ZELLIJ` first (set by Zellij in panes it spawns), then falls
/// back to probing `zellij action dump-layout` if `ZELLIJ_SESSION_NAME`
/// is set but `ZELLIJ` is not (e.g. processes started by tools/plugins
/// inside a Zellij pane). The probe result is cached.
pub fn is_inside_zellij() -> bool {
    use std::sync::atomic::AtomicU8;
    use std::sync::atomic::Ordering;
    // 0 = unchecked, 1 = yes, 2 = no
    static CACHED: AtomicU8 = AtomicU8::new(0);

    let cached = CACHED.load(Ordering::Relaxed);
    if cached != 0 {
        return cached == 1;
    }

    let is_available = if std::env::var("ZELLIJ").is_ok() {
        true
    } else if std::env::var("ZELLIJ_SESSION_NAME").is_ok() {
        // ZELLIJ_SESSION_NAME is set but ZELLIJ isn't — probe to see
        // if we can actually talk to the session
        std::process::Command::new("zellij")
            .args(["action", "dump-layout"])
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .is_ok_and(|s| s.success())
    } else {
        false
    };

    CACHED.store(if is_available { 1 } else { 2 }, Ordering::Relaxed);
    is_available
}

/// Get the current zellij session name
pub fn session_name() -> Option<String> {
    if !is_inside_zellij() {
        return None;
    }
    std::env::var("ZELLIJ_SESSION_NAME").ok()
}

/// Check if zellij is installed
pub fn is_zellij_available() -> bool {
    std::process::Command::new("zellij").arg("--version").output().is_ok_and(|o| o.status.success())
}

/// Resolve the absolute path to the clankers binary for Zellij layout commands.
///
/// Returns `(command, prefix_args)` where command is the full path to the
/// current executable. This works in both dev mode (target/debug/clankers) and
/// production (installed binary) since we always use the absolute path.
pub fn resolve_clankers_command() -> (String, Vec<String>) {
    let exe = std::env::current_exe().unwrap_or_else(|_| std::path::PathBuf::from("clankers"));
    (exe.to_string_lossy().to_string(), vec![])
}

/// Compute the session name for a given working directory.
pub fn session_name_for_cwd(cwd: &Path) -> String {
    format!("clankers-{}", cwd_hash(cwd))
}

/// Check whether a named zellij session already exists in the output of
/// `zellij list-sessions --no-formatting`.
pub fn session_exists_in_listing(listing: &str, session_name: &str) -> bool {
    listing.lines().any(|l| l.trim().starts_with(session_name))
}

/// Build the argument list for `zellij` to create a **new** session.
///
/// Returns e.g. `["-s", "clankers-abcd1234", "--layout", "/tmp/layout.kdl"]`.
pub fn build_new_session_args<'a>(session_name: &'a str, layout: Option<&'a str>) -> Vec<&'a str> {
    let mut args = vec!["-s", session_name];
    if let Some(layout_path) = layout {
        args.extend(["--layout", layout_path]);
    }
    args
}

/// Build the argument list for `zellij attach`.
pub fn build_attach_args(session_name: &str) -> Vec<&str> {
    vec!["attach", session_name]
}

/// Launch a new zellij session with a layout, or attach to existing.
/// Session name: clankers-<cwd-hash-prefix>
pub fn launch_or_attach(cwd: &Path, layout: Option<&str>) -> std::io::Result<()> {
    let session_name = session_name_for_cwd(cwd);

    // Check if session already exists
    let existing = std::process::Command::new("zellij").args(["list-sessions", "--no-formatting"]).output()?;
    let listing = String::from_utf8_lossy(&existing.stdout);

    if session_exists_in_listing(&listing, &session_name) {
        let args = build_attach_args(&session_name);
        std::process::Command::new("zellij").args(&args).status()?;
    } else {
        let args = build_new_session_args(&session_name, layout);
        std::process::Command::new("zellij").args(&args).status()?;
    }
    Ok(())
}

/// CLI arguments that should be forwarded when re-launching clankers inside Zellij.
///
/// Collects the relevant flags from the outer invocation so the inner clankers
/// instance behaves identically (same model, agent, thinking config, etc.).
pub struct ForwardableArgs {
    pub model: Option<String>,
    pub agent: Option<String>,
    pub thinking: bool,
    pub thinking_budget: Option<usize>,
    pub system_prompt: Option<String>,
    pub continue_session: bool,
    pub resume: Option<String>,
    pub no_session: bool,
}

impl ForwardableArgs {
    /// Convert to a flat `Vec<String>` of CLI flags.
    /// `--no-zellij` is always prepended to prevent re-launch loops.
    pub fn to_args(&self) -> Vec<String> {
        let mut args = vec!["--no-zellij".to_string()];
        if let Some(ref m) = self.model {
            args.extend(["--model".to_string(), m.clone()]);
        }
        if let Some(ref a) = self.agent {
            args.extend(["--agent".to_string(), a.clone()]);
        }
        if self.thinking {
            args.push("--thinking".to_string());
        }
        if let Some(budget) = self.thinking_budget {
            args.extend(["--thinking-budget".to_string(), budget.to_string()]);
        }
        if let Some(ref sp) = self.system_prompt {
            args.extend(["--system-prompt".to_string(), sp.clone()]);
        }
        if self.continue_session {
            args.push("--continue".to_string());
        }
        if let Some(ref id) = self.resume {
            args.extend(["--resume".to_string(), id.clone()]);
        }
        if self.no_session {
            args.push("--no-session".to_string());
        }
        args
    }
}

fn cwd_hash(cwd: &Path) -> String {
    use std::hash::Hash;
    use std::hash::Hasher;
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    cwd.hash(&mut hasher);
    format!("{:08x}", hasher.finish() & 0xFFFFFFFF)
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── cwd_hash ─────────────────────────────────────────────────────

    #[test]
    fn test_cwd_hash_deterministic() {
        let h1 = cwd_hash(Path::new("/home/user/project"));
        let h2 = cwd_hash(Path::new("/home/user/project"));
        assert_eq!(h1, h2);
        assert_eq!(h1.len(), 8);
    }

    #[test]
    fn test_cwd_hash_different_paths() {
        let h1 = cwd_hash(Path::new("/home/user/project-a"));
        let h2 = cwd_hash(Path::new("/home/user/project-b"));
        assert_ne!(h1, h2);
    }

    #[test]
    fn test_cwd_hash_is_hex() {
        let h = cwd_hash(Path::new("/tmp/test"));
        assert!(h.chars().all(|c| c.is_ascii_hexdigit()), "hash should be hex: {}", h);
    }

    // ── session_name_for_cwd ─────────────────────────────────────────

    #[test]
    fn test_session_name_for_cwd_prefix() {
        let name = session_name_for_cwd(Path::new("/home/user/project"));
        assert!(name.starts_with("clankers-"), "name: {}", name);
        assert_eq!(name.len(), 9 + 8); // "clankers-" + 8 hex chars
    }

    #[test]
    fn test_session_name_for_cwd_deterministic() {
        let n1 = session_name_for_cwd(Path::new("/foo/bar"));
        let n2 = session_name_for_cwd(Path::new("/foo/bar"));
        assert_eq!(n1, n2);
    }

    #[test]
    fn test_session_name_for_cwd_unique() {
        let n1 = session_name_for_cwd(Path::new("/project-a"));
        let n2 = session_name_for_cwd(Path::new("/project-b"));
        assert_ne!(n1, n2);
    }

    // ── session_exists_in_listing ────────────────────────────────────

    #[test]
    fn test_session_exists_in_listing_found() {
        let listing = "clankers-abcd1234\nother-session\n";
        assert!(session_exists_in_listing(listing, "clankers-abcd1234"));
    }

    #[test]
    fn test_session_exists_in_listing_not_found() {
        let listing = "clankers-abcd1234\nother-session\n";
        assert!(!session_exists_in_listing(listing, "clankers-deadbeef"));
    }

    #[test]
    fn test_session_exists_in_listing_empty() {
        assert!(!session_exists_in_listing("", "clankers-abcd1234"));
    }

    #[test]
    fn test_session_exists_in_listing_prefix_match() {
        // zellij sometimes adds metadata after the session name
        let listing = "clankers-abcd1234 (attached)\n";
        assert!(session_exists_in_listing(listing, "clankers-abcd1234"));
    }

    #[test]
    fn test_session_exists_in_listing_whitespace() {
        let listing = "  clankers-abcd1234  \n";
        assert!(session_exists_in_listing(listing, "clankers-abcd1234"));
    }

    #[test]
    fn test_session_exists_in_listing_prefix_semantics() {
        // starts_with semantics — a shorter name matches a longer session
        // This is safe because session_name_for_cwd always produces exactly
        // "clankers-" + 8 hex chars, so real names won't be prefixes of each other.
        let listing = "clankers-abcdef12\n";
        assert!(session_exists_in_listing(listing, "clankers-abcdef12"));
        // A completely different name should not match
        assert!(!session_exists_in_listing(listing, "clankers-99999999"));
    }

    // ── build_new_session_args ───────────────────────────────────────

    #[test]
    fn test_build_new_session_args_no_layout() {
        let args = build_new_session_args("clankers-abcd1234", None);
        assert_eq!(args, vec!["-s", "clankers-abcd1234"]);
    }

    #[test]
    fn test_build_new_session_args_with_layout() {
        let args = build_new_session_args("clankers-abcd1234", Some("/tmp/layout.kdl"));
        assert_eq!(args, vec!["-s", "clankers-abcd1234", "--layout", "/tmp/layout.kdl"]);
    }

    // ── build_attach_args ────────────────────────────────────────────

    #[test]
    fn test_build_attach_args() {
        let args = build_attach_args("clankers-abcd1234");
        assert_eq!(args, vec!["attach", "clankers-abcd1234"]);
    }

    // ── ForwardableArgs ──────────────────────────────────────────────

    #[test]
    fn test_forwardable_args_empty() {
        let fa = ForwardableArgs {
            model: None,
            agent: None,
            thinking: false,
            thinking_budget: None,
            system_prompt: None,
            continue_session: false,
            resume: None,
            no_session: false,
        };
        assert_eq!(fa.to_args(), vec!["--no-zellij"]);
    }

    #[test]
    fn test_forwardable_args_always_includes_no_zellij() {
        let fa = ForwardableArgs {
            model: Some("gpt-4".to_string()),
            agent: None,
            thinking: false,
            thinking_budget: None,
            system_prompt: None,
            continue_session: false,
            resume: None,
            no_session: false,
        };
        let args = fa.to_args();
        assert_eq!(args[0], "--no-zellij");
    }

    #[test]
    fn test_forwardable_args_model() {
        let fa = ForwardableArgs {
            model: Some("claude-sonnet-4-20250514".to_string()),
            agent: None,
            thinking: false,
            thinking_budget: None,
            system_prompt: None,
            continue_session: false,
            resume: None,
            no_session: false,
        };
        let args = fa.to_args();
        assert!(args.contains(&"--model".to_string()));
        assert!(args.contains(&"claude-sonnet-4-20250514".to_string()));
    }

    #[test]
    fn test_forwardable_args_agent() {
        let fa = ForwardableArgs {
            model: None,
            agent: Some("coder".to_string()),
            thinking: false,
            thinking_budget: None,
            system_prompt: None,
            continue_session: false,
            resume: None,
            no_session: false,
        };
        let args = fa.to_args();
        assert!(args.contains(&"--agent".to_string()));
        assert!(args.contains(&"coder".to_string()));
    }

    #[test]
    fn test_forwardable_args_thinking_with_budget() {
        let fa = ForwardableArgs {
            model: None,
            agent: None,
            thinking: true,
            thinking_budget: Some(20000),
            system_prompt: None,
            continue_session: false,
            resume: None,
            no_session: false,
        };
        let args = fa.to_args();
        assert!(args.contains(&"--thinking".to_string()));
        assert!(args.contains(&"--thinking-budget".to_string()));
        assert!(args.contains(&"20000".to_string()));
    }

    #[test]
    fn test_forwardable_args_system_prompt() {
        let fa = ForwardableArgs {
            model: None,
            agent: None,
            thinking: false,
            thinking_budget: None,
            system_prompt: Some("You are helpful.".to_string()),
            continue_session: false,
            resume: None,
            no_session: false,
        };
        let args = fa.to_args();
        assert!(args.contains(&"--system-prompt".to_string()));
        assert!(args.contains(&"You are helpful.".to_string()));
    }

    #[test]
    fn test_forwardable_args_continue() {
        let fa = ForwardableArgs {
            model: None,
            agent: None,
            thinking: false,
            thinking_budget: None,
            system_prompt: None,
            continue_session: true,
            resume: None,
            no_session: false,
        };
        let args = fa.to_args();
        assert!(args.contains(&"--continue".to_string()));
    }

    #[test]
    fn test_forwardable_args_resume() {
        let fa = ForwardableArgs {
            model: None,
            agent: None,
            thinking: false,
            thinking_budget: None,
            system_prompt: None,
            continue_session: false,
            resume: Some("abc-123".to_string()),
            no_session: false,
        };
        let args = fa.to_args();
        assert!(args.contains(&"--resume".to_string()));
        assert!(args.contains(&"abc-123".to_string()));
    }

    #[test]
    fn test_forwardable_args_no_session() {
        let fa = ForwardableArgs {
            model: None,
            agent: None,
            thinking: false,
            thinking_budget: None,
            system_prompt: None,
            continue_session: false,
            resume: None,
            no_session: true,
        };
        let args = fa.to_args();
        assert!(args.contains(&"--no-session".to_string()));
    }

    #[test]
    fn test_forwardable_args_all_flags() {
        let fa = ForwardableArgs {
            model: Some("opus".to_string()),
            agent: Some("reviewer".to_string()),
            thinking: true,
            thinking_budget: Some(50000),
            system_prompt: Some("Be concise.".to_string()),
            continue_session: true,
            resume: None,
            no_session: true,
        };
        let args = fa.to_args();
        // Must always start with --no-zellij
        assert_eq!(args[0], "--no-zellij");
        // Check all flags present
        assert!(args.contains(&"--model".to_string()));
        assert!(args.contains(&"opus".to_string()));
        assert!(args.contains(&"--agent".to_string()));
        assert!(args.contains(&"reviewer".to_string()));
        assert!(args.contains(&"--thinking".to_string()));
        assert!(args.contains(&"--thinking-budget".to_string()));
        assert!(args.contains(&"50000".to_string()));
        assert!(args.contains(&"--system-prompt".to_string()));
        assert!(args.contains(&"Be concise.".to_string()));
        assert!(args.contains(&"--continue".to_string()));
        assert!(args.contains(&"--no-session".to_string()));
        // --resume should NOT be present (it was None)
        assert!(!args.contains(&"--resume".to_string()));
    }

    #[test]
    fn test_forwardable_args_order_no_zellij_first() {
        // --no-zellij must be the very first arg to prevent re-launch
        let fa = ForwardableArgs {
            model: Some("test".to_string()),
            agent: Some("test".to_string()),
            thinking: true,
            thinking_budget: Some(1000),
            system_prompt: Some("test".to_string()),
            continue_session: true,
            resume: Some("id".to_string()),
            no_session: true,
        };
        let args = fa.to_args();
        assert_eq!(args[0], "--no-zellij");
    }

    // ── resolve_clankers_command ────────────────────────────────────────

    #[test]
    fn test_resolve_clankers_command_returns_absolute_path() {
        let (cmd, prefix) = resolve_clankers_command();
        assert!(!cmd.is_empty());
        assert!(prefix.is_empty(), "should have no prefix args");
        // Should be an absolute path (or fallback "clankers")
        assert!(cmd.starts_with('/') || cmd == "clankers", "expected absolute path, got: {}", cmd);
    }

    // ── environment detection ────────────────────────────────────────

    #[test]
    fn test_is_inside_zellij_consistent() {
        // Should return the same result on repeated calls (caching)
        let a = is_inside_zellij();
        let b = is_inside_zellij();
        assert_eq!(a, b);
    }

    #[test]
    fn test_session_name_none_when_not_inside() {
        if !is_inside_zellij() {
            assert!(session_name().is_none());
        }
    }
}
