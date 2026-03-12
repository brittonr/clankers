use std::path::PathBuf;

use serde::Deserialize;
use serde::Serialize;

/// Hook system configuration (appears in settings.json as "hooks").
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HooksConfig {
    /// Master enable/disable for the hook system.
    #[serde(default = "default_true")]
    pub enabled: bool,

    /// Directory for user hook scripts.
    /// Default: `.clankers/hooks/` (relative to project root).
    #[serde(default)]
    pub hooks_dir: Option<PathBuf>,

    /// Hook points to disable (by kebab-case name, e.g. "pre-tool").
    #[serde(default)]
    pub disabled_hooks: Vec<String>,

    /// Timeout for script hooks in seconds.
    #[serde(default = "default_script_timeout")]
    pub script_timeout_secs: u64,

    /// Whether to manage .git/hooks/ shims.
    #[serde(default)]
    pub manage_git_hooks: bool,
}

fn default_true() -> bool {
    true
}
fn default_script_timeout() -> u64 {
    10
}

impl Default for HooksConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            hooks_dir: None,
            disabled_hooks: Vec::new(),
            script_timeout_secs: default_script_timeout(),
            manage_git_hooks: false,
        }
    }
}

impl HooksConfig {
    /// Resolve the hooks directory path. Returns the configured path or the
    /// default `.clankers/hooks/` relative to the given project root.
    pub fn resolve_hooks_dir(&self, project_root: &std::path::Path) -> PathBuf {
        self.hooks_dir.clone().unwrap_or_else(|| project_root.join(".clankers").join("hooks"))
    }

    /// Check if a hook point is disabled by name.
    pub fn is_hook_disabled(&self, hook_name: &str) -> bool {
        self.disabled_hooks.iter().any(|h| h == hook_name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config() {
        let cfg = HooksConfig::default();
        assert!(cfg.enabled);
        assert_eq!(cfg.script_timeout_secs, 10);
        assert!(!cfg.manage_git_hooks);
        assert!(cfg.hooks_dir.is_none());
        assert!(cfg.disabled_hooks.is_empty());
    }

    #[test]
    fn deserialize_from_json() {
        let json = r#"{"enabled": false, "scriptTimeoutSecs": 30, "disabledHooks": ["pre-tool"]}"#;
        let cfg: HooksConfig = serde_json::from_str(json).unwrap();
        assert!(!cfg.enabled);
        assert_eq!(cfg.script_timeout_secs, 30);
        assert_eq!(cfg.disabled_hooks, vec!["pre-tool"]);
    }

    #[test]
    fn resolve_hooks_dir_default() {
        let cfg = HooksConfig::default();
        let dir = cfg.resolve_hooks_dir(std::path::Path::new("/project"));
        assert_eq!(dir, PathBuf::from("/project/.clankers/hooks"));
    }

    #[test]
    fn resolve_hooks_dir_custom() {
        let cfg = HooksConfig {
            hooks_dir: Some(PathBuf::from("/custom/hooks")),
            ..Default::default()
        };
        let dir = cfg.resolve_hooks_dir(std::path::Path::new("/project"));
        assert_eq!(dir, PathBuf::from("/custom/hooks"));
    }

    #[test]
    fn is_hook_disabled() {
        let cfg = HooksConfig {
            disabled_hooks: vec!["pre-tool".into(), "on-error".into()],
            ..Default::default()
        };
        assert!(cfg.is_hook_disabled("pre-tool"));
        assert!(cfg.is_hook_disabled("on-error"));
        assert!(!cfg.is_hook_disabled("post-tool"));
    }
}
