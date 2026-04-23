//! Settings loading (global + project JSON)

use std::path::Path;

use clankers_agent_defs::definition::AgentScope;
use clankers_tui_types::MenuPlacement;
use serde::Deserialize;
use serde::Serialize;
use serde_json;

use crate::keybindings::KeymapConfig;
use crate::model_roles::ModelRoles;

/// Full settings, merged from global + project
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    /// Default model to use
    #[serde(default = "default_model")]
    pub model: String,

    /// Default max tokens for output
    #[serde(default = "default_max_tokens")]
    pub max_tokens: usize,

    /// Agent scope for discovery
    #[serde(default)]
    pub agent_scope: AgentScope,

    /// Whether to confirm before running project agents
    #[serde(default = "default_true")]
    pub confirm_project_agents: bool,

    /// Whether to create git worktrees for sessions (opt-in — writes go to
    /// a hidden worktree directory which surprises users expecting in-place edits)
    #[serde(default)]
    pub use_worktrees: bool,

    /// Custom system prompt prefix
    #[serde(default)]
    pub system_prompt_prefix: Option<String>,

    /// Custom system prompt suffix
    #[serde(default)]
    pub system_prompt_suffix: Option<String>,

    /// Theme name
    #[serde(default)]
    pub theme: Option<String>,

    /// Max output lines before truncation
    #[serde(default = "default_max_lines")]
    pub max_output_lines: usize,

    /// Max output bytes before truncation
    #[serde(default = "default_max_bytes")]
    pub max_output_bytes: usize,

    /// Bash command timeout in seconds (0 = no timeout)
    #[serde(default)]
    pub bash_timeout: u64,

    /// Auto-launch inside Zellij when available
    #[serde(default)]
    pub zellij: Option<bool>,

    /// Keymap configuration (preset + overrides)
    #[serde(default)]
    pub keymap: KeymapConfig,

    /// Model roles — route different tasks to different models
    #[serde(default, rename = "modelRoles")]
    pub model_roles: ModelRoles,

    /// Whether plan mode is enabled by default
    #[serde(default)]
    pub plan_mode: bool,

    /// Leader menu customization (add/override/hide items).
    #[serde(default)]
    pub leader_menu: LeaderMenuConfig,

    /// Memory capacity limits (cross-session learning loop)
    #[serde(default)]
    pub memory: MemoryLimits,

    /// Context compression settings (LLM-based summarization)
    #[serde(default)]
    pub compression: CompressionSettings,

    /// Routing policy configuration (auto model selection by complexity)
    #[serde(default)]
    pub routing: Option<clankers_model_selection::config::RoutingPolicyConfig>,

    /// Cost tracking configuration (budget limits and warnings)
    #[serde(default)]
    pub cost_tracking: Option<clankers_model_selection::cost_tracker::CostTrackerConfig>,

    /// Max number of subagent panes to auto-create in the BSP tiling.
    /// When the limit is reached, new subagents only appear in the overview panel.
    /// Set to 0 to disable auto-pane creation entirely.
    #[serde(default = "default_max_subagent_panes")]
    pub max_subagent_panes: usize,

    /// Tools to disable (by name). Merged from global + project settings.
    /// Tools in this list are not registered with the agent.
    #[serde(default)]
    pub disabled_tools: Vec<String>,

    /// Hook system configuration.
    #[serde(default)]
    pub hooks: clankers_hooks::HooksConfig,

    /// Command to run automatically after the agent finishes a turn.
    /// When set, enables auto-test mode (e.g. "cargo nextest run", "npm test").
    /// Use `/autotest` to toggle on/off during a session.
    #[serde(default)]
    pub auto_test_command: Option<String>,

    /// Disable prompt caching (send requests without cache_control breakpoints).
    /// When false (default), tool result compaction is also skipped because
    /// prompt caching provides larger cost savings than compaction.
    #[serde(default)]
    pub no_cache: bool,

    /// Cache TTL for prompt caching. Default is "5m" (ephemeral).
    /// Set to "1h" for 1-hour cache at 2× base input cost (useful for
    /// long-running agentic tasks where turns exceed the 5-minute window).
    #[serde(default)]
    pub cache_ttl: Option<String>,

    /// When true, scan nix/bash tool output for /nix/store/ paths and append
    /// a compact annotation listing referenced packages. Default: false.
    #[serde(default)]
    pub annotate_store_refs: bool,

    /// When true, the default interactive mode auto-starts a background daemon
    /// and attaches the TUI to a daemon session instead of running an in-process
    /// agent. Override with `--daemon` / `--no-daemon` CLI flags.
    #[serde(default = "default_true")]
    pub use_daemon: bool,

    /// Whether the TUI should dump recent conversation blocks to terminal
    /// scrollback after leaving the alternate screen. `None` keeps the default
    /// enabled behavior while `Some(false)` disables the dump explicitly.
    #[serde(default, alias = "scrollback_on_exit")]
    pub scrollback_on_exit: Option<bool>,

    /// Default capability restrictions for all sessions (including local).
    ///
    /// When set, every agent session gets a capability gate that enforces
    /// these restrictions at tool execution time — the LLM cannot bypass
    /// them. Capabilities are specified as UCAN capability objects.
    ///
    /// Example (settings.json):
    /// ```json
    /// "defaultCapabilities": [
    ///   { "ToolUse": { "tool_pattern": "read,write,edit,bash,rg" } },
    ///   { "ShellExecute": { "command_pattern": "*", "working_dir": "/home/user/project" } },
    ///   { "FileAccess": { "prefix": "/home/user/project", "read_only": false } }
    /// ]
    /// ```
    ///
    /// When absent (default), local sessions have full access. Remote sessions
    /// are still constrained by their UCAN token capabilities.
    #[serde(default)]
    pub default_capabilities: Option<Vec<clankers_ucan::Capability>>,
}

// ---------------------------------------------------------------------------
// Leader menu user config
// ---------------------------------------------------------------------------

/// User-configurable leader menu items and hide rules.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct LeaderMenuConfig {
    /// Items to add or override in the leader menu.
    #[serde(default)]
    pub items: Vec<LeaderMenuItemConfig>,
    /// Items to hide from the leader menu.
    #[serde(default)]
    pub hide: Vec<LeaderMenuHideConfig>,
}

/// A user-defined leader menu item.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LeaderMenuItemConfig {
    /// Key to press.
    pub key: char,
    /// Display label.
    pub label: String,
    /// Slash command to execute (e.g. "/shell git status").
    pub command: String,
    /// Submenu name. If omitted, goes to root.
    #[serde(default)]
    pub submenu: Option<String>,
}

/// Hides a specific leader menu entry by key + placement.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LeaderMenuHideConfig {
    /// Key to hide.
    pub key: char,
    /// Submenu name. If omitted, hides from root.
    #[serde(default)]
    pub submenu: Option<String>,
}

impl clankers_tui_types::MenuContributor for LeaderMenuConfig {
    fn menu_items(&self) -> Vec<clankers_tui_types::MenuContribution> {
        use clankers_tui_types::LeaderAction;
        use clankers_tui_types::MenuContribution;
        use clankers_tui_types::PRIORITY_USER;

        self.items
            .iter()
            .map(|item| MenuContribution {
                key: item.key,
                label: item.label.clone(),
                action: LeaderAction::Command(item.command.clone()),
                placement: match &item.submenu {
                    Some(name) => MenuPlacement::Submenu(name.clone()),
                    None => MenuPlacement::Root,
                },
                priority: PRIORITY_USER,
                source: "config".into(),
            })
            .collect()
    }
}

impl LeaderMenuConfig {
    /// Convert hide rules to a set of (key, placement) pairs for the builder.
    pub fn hidden_set(&self) -> std::collections::HashSet<(char, MenuPlacement)> {
        self.hide
            .iter()
            .map(|h| {
                let placement = match &h.submenu {
                    Some(name) => MenuPlacement::Submenu(name.clone()),
                    None => MenuPlacement::Root,
                };
                (h.key, placement)
            })
            .collect()
    }
}

// ---------------------------------------------------------------------------
// Memory limits
// ---------------------------------------------------------------------------

/// Capacity limits for cross-session memory.
///
/// The agent's memory tool checks these before saving new entries.
/// Character counts refer to the sum of `entry.text.len()` within each scope.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MemoryLimits {
    /// Max chars for global-scope memories (default: 2200 ≈ 800 tokens).
    #[serde(default = "default_global_char_limit")]
    pub global_char_limit: usize,
    /// Max chars for per-project memories (default: 1375 ≈ 500 tokens).
    #[serde(default = "default_project_char_limit")]
    pub project_char_limit: usize,
}

fn default_global_char_limit() -> usize {
    2200
}
fn default_project_char_limit() -> usize {
    1375
}

impl Default for MemoryLimits {
    fn default() -> Self {
        Self {
            global_char_limit: default_global_char_limit(),
            project_char_limit: default_project_char_limit(),
        }
    }
}

// ---------------------------------------------------------------------------
// Compression settings
// ---------------------------------------------------------------------------

/// Configuration for LLM-based context compression.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompressionSettings {
    /// Model to use for summarization. When absent, uses the cheapest
    /// available model from the active provider.
    #[serde(default)]
    pub model: Option<String>,
    /// Number of recent messages to keep intact during compression.
    #[serde(default = "default_keep_recent")]
    pub keep_recent: usize,
    /// Minimum message count before compression is allowed.
    #[serde(default = "default_min_messages")]
    pub min_messages: usize,
}

fn default_keep_recent() -> usize {
    4
}
fn default_min_messages() -> usize {
    5
}

impl Default for CompressionSettings {
    fn default() -> Self {
        Self {
            model: None,
            keep_recent: default_keep_recent(),
            min_messages: default_min_messages(),
        }
    }
}

fn default_model() -> String {
    "claude-sonnet-4-5".to_string()
}
fn default_max_tokens() -> usize {
    16384
}
fn default_true() -> bool {
    true
}
fn default_max_lines() -> usize {
    2000
}
fn default_max_bytes() -> usize {
    50 * 1024
}
fn default_max_subagent_panes() -> usize {
    4
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            model: default_model(),
            max_tokens: default_max_tokens(),
            agent_scope: AgentScope::default(),
            confirm_project_agents: true,
            use_worktrees: false,
            system_prompt_prefix: None,
            system_prompt_suffix: None,
            theme: None,
            max_output_lines: default_max_lines(),
            max_output_bytes: default_max_bytes(),
            bash_timeout: 0,
            zellij: None,
            keymap: KeymapConfig::default(),
            model_roles: ModelRoles::default(),
            plan_mode: false,
            leader_menu: LeaderMenuConfig::default(),
            memory: MemoryLimits::default(),
            compression: CompressionSettings::default(),
            routing: None,
            cost_tracking: None,
            max_subagent_panes: default_max_subagent_panes(),
            disabled_tools: Vec::new(),
            hooks: clankers_hooks::HooksConfig::default(),
            auto_test_command: None,
            no_cache: false,
            cache_ttl: None,
            annotate_store_refs: false,
            use_daemon: true,
            scrollback_on_exit: None,
            default_capabilities: None,
        }
    }
}

impl Settings {
    /// Load settings by merging pi fallback, global, and project files.
    /// Priority (highest wins): project > global (~/.clankers) > pi fallback (~/.pi)
    pub fn load(global_path: &Path, project_path: &Path) -> Self {
        Self::load_with_pi_fallback(None, global_path, project_path)
    }

    /// Load settings with an optional ~/.pi/agent/settings.json fallback.
    /// Priority (highest wins): project > global (~/.clankers) > pi fallback (~/.pi)
    pub fn load_with_pi_fallback(pi_settings_path: Option<&Path>, global_path: &Path, project_path: &Path) -> Self {
        let pi = pi_settings_path.and_then(Self::load_file).map(Self::normalize_pi_settings);
        let global = Self::load_file(global_path);
        let project = Self::load_file(project_path);
        Self::merge_layers(pi, global, project)
    }

    /// Load settings with Nickel support. Checks `.ncl` paths first, falls
    /// back to `.json` at each layer.
    ///
    /// Priority (highest wins): project > global > pi fallback.
    /// At each layer: `.ncl` preferred over `.json` when both exist.
    pub fn load_with_nickel(
        pi_settings_path: Option<&Path>,
        global_json: &Path,
        global_ncl: &Path,
        project_json: &Path,
        project_ncl: &Path,
    ) -> Self {
        let pi = pi_settings_path.and_then(Self::load_file).map(Self::normalize_pi_settings);
        let global = Self::load_layer(Some(global_ncl), global_json);
        let project = Self::load_layer(Some(project_ncl), project_json);
        Self::merge_layers(pi, global, project)
    }

    /// Load a single config layer. Checks `.ncl` first (if the nickel feature
    /// is enabled), then falls back to `.json`.
    fn load_layer(ncl_path: Option<&Path>, json_path: &Path) -> Option<serde_json::Value> {
        #[cfg(feature = "nickel")]
        if let Some(ncl) = ncl_path
            && ncl.exists()
        {
            match crate::nickel::eval_ncl_file(ncl) {
                Ok(value) => return Some(value),
                Err(e) => {
                    eprintln!("warning: failed to evaluate {}: {e}", ncl.display());
                    // Fall through to JSON
                }
            }
        }
        #[cfg(not(feature = "nickel"))]
        let _ = ncl_path;

        Self::load_file(json_path)
    }

    /// Map pi-specific setting names to clankers equivalents.
    /// e.g. pi uses "defaultModel" while clankers uses "model".
    fn normalize_pi_settings(mut value: serde_json::Value) -> serde_json::Value {
        if let Some(obj) = value.as_object_mut() {
            // Map defaultModel -> model
            if let Some(model) = obj.remove("defaultModel") {
                obj.entry("model").or_insert(model);
            }
        }
        value
    }

    fn load_file(path: &Path) -> Option<serde_json::Value> {
        let content = std::fs::read_to_string(path).ok()?;
        serde_json::from_str(&content).ok()
    }

    /// Merge up to three layers of settings: pi fallback < global < project
    fn merge_layers(
        pi: Option<serde_json::Value>,
        global: Option<serde_json::Value>,
        project: Option<serde_json::Value>,
    ) -> Self {
        let mut base = pi.unwrap_or_else(|| serde_json::json!({}));

        // Merge global on top of pi fallback
        if let Some(g) = global {
            Self::merge_into(&mut base, &g);
        }

        // Merge project on top
        if let Some(p) = project {
            Self::merge_into(&mut base, &p);
        }

        serde_json::from_value(base).unwrap_or_default()
    }

    /// Recursively merge source object fields into target object.
    ///
    /// When both target and source have an object at the same key, the merge
    /// recurses into the nested object so that individual fields are preserved.
    /// Non-object values (strings, numbers, arrays, bools, nulls) are replaced
    /// wholesale — arrays are NOT concatenated.
    fn merge_into(target: &mut serde_json::Value, source: &serde_json::Value) {
        if let (Some(target_obj), Some(source_obj)) = (target.as_object_mut(), source.as_object()) {
            for (key, value) in source_obj {
                match (target_obj.get_mut(key), value) {
                    // Both sides are objects → recurse
                    (Some(existing), new_val) if existing.is_object() && new_val.is_object() => {
                        Self::merge_into(existing, new_val);
                    }
                    // Otherwise replace (or insert new key)
                    _ => {
                        target_obj.insert(key.clone(), value.clone());
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn disabled_tools_from_json() {
        let json = r#"{"disabledTools": ["bash", "commit"]}"#;
        let settings: Settings = serde_json::from_str(json).unwrap();
        assert_eq!(settings.disabled_tools, vec!["bash".to_string(), "commit".to_string()]);
    }

    #[test]
    fn disabled_tools_default_empty() {
        let json = r#"{}"#;
        let settings: Settings = serde_json::from_str(json).unwrap();
        assert!(settings.disabled_tools.is_empty());
    }

    #[test]
    fn disabled_tools_project_overrides_global() {
        let global = serde_json::json!({"disabledTools": ["bash"]});
        let project = serde_json::json!({"disabledTools": ["commit", "review"]});
        let settings = Settings::merge_layers(None, Some(global), Some(project));
        // Project replaces global (field-level merge, not array merge)
        assert_eq!(settings.disabled_tools, vec!["commit".to_string(), "review".to_string()]);
    }

    #[test]
    fn auto_test_command_from_json() {
        let json = r#"{"autoTestCommand": "cargo nextest run"}"#;
        let settings: Settings = serde_json::from_str(json).unwrap();
        assert_eq!(settings.auto_test_command, Some("cargo nextest run".to_string()));
    }

    #[test]
    fn auto_test_command_default_none() {
        let json = r#"{}"#;
        let settings: Settings = serde_json::from_str(json).unwrap();
        assert!(settings.auto_test_command.is_none());
    }

    #[test]
    fn auto_test_command_project_overrides_global() {
        let global = serde_json::json!({"autoTestCommand": "cargo test"});
        let project = serde_json::json!({"autoTestCommand": "cargo nextest run"});
        let settings = Settings::merge_layers(None, Some(global), Some(project));
        assert_eq!(settings.auto_test_command, Some("cargo nextest run".to_string()));
    }

    #[test]
    fn use_daemon_default_true() {
        let json = r#"{}"#;
        let settings: Settings = serde_json::from_str(json).unwrap();
        assert!(settings.use_daemon);
    }

    #[test]
    fn use_daemon_explicit_false() {
        let json = r#"{"useDaemon": false}"#;
        let settings: Settings = serde_json::from_str(json).unwrap();
        assert!(!settings.use_daemon);
    }

    #[test]
    fn use_daemon_project_overrides_global() {
        let global = serde_json::json!({"useDaemon": true});
        let project = serde_json::json!({"useDaemon": false});
        let settings = Settings::merge_layers(None, Some(global), Some(project));
        assert!(!settings.use_daemon);
    }

    #[test]
    fn scrollback_on_exit_default_none() {
        let json = r#"{}"#;
        let settings: Settings = serde_json::from_str(json).unwrap();
        assert_eq!(settings.scrollback_on_exit, None);
    }

    #[test]
    fn scrollback_on_exit_snake_case_false() {
        let json = r#"{"scrollback_on_exit": false}"#;
        let settings: Settings = serde_json::from_str(json).unwrap();
        assert_eq!(settings.scrollback_on_exit, Some(false));
    }

    #[test]
    fn scrollback_on_exit_camel_case_true() {
        let json = r#"{"scrollbackOnExit": true}"#;
        let settings: Settings = serde_json::from_str(json).unwrap();
        assert_eq!(settings.scrollback_on_exit, Some(true));
    }

    #[test]
    fn scrollback_on_exit_project_overrides_global() {
        let global = serde_json::json!({"scrollbackOnExit": false});
        let project = serde_json::json!({"scrollbackOnExit": true});
        let settings = Settings::merge_layers(None, Some(global), Some(project));
        assert_eq!(settings.scrollback_on_exit, Some(true));
    }

    // ── Deep merge tests ───────────────────────────────────────────

    #[test]
    fn deep_merge_nested_object_partial_override() {
        let global = serde_json::json!({
            "hooks": {
                "enabled": true,
                "scriptTimeoutSecs": 10
            }
        });
        let project = serde_json::json!({
            "hooks": {
                "disabledHooks": ["pre-tool"]
            }
        });
        let settings = Settings::merge_layers(None, Some(global), Some(project));
        assert!(settings.hooks.enabled);
        assert_eq!(settings.hooks.script_timeout_secs, 10);
        assert_eq!(settings.hooks.disabled_hooks, vec!["pre-tool".to_string()]);
    }

    #[test]
    fn deep_merge_scalar_override_within_nested_object() {
        let global = serde_json::json!({
            "memory": {"globalCharLimit": 2200}
        });
        let project = serde_json::json!({
            "memory": {"globalCharLimit": 4400}
        });
        let settings = Settings::merge_layers(None, Some(global), Some(project));
        assert_eq!(settings.memory.global_char_limit, 4400);
    }

    #[test]
    fn deep_merge_array_fields_replaced_not_merged() {
        let global = serde_json::json!({"disabledTools": ["bash"]});
        let project = serde_json::json!({"disabledTools": ["commit"]});
        let settings = Settings::merge_layers(None, Some(global), Some(project));
        assert_eq!(settings.disabled_tools, vec!["commit".to_string()]);
    }

    #[test]
    fn deep_merge_three_layers() {
        let pi = serde_json::json!({
            "hooks": {"enabled": false, "scriptTimeoutSecs": 5},
            "memory": {"globalCharLimit": 1000}
        });
        let global = serde_json::json!({
            "hooks": {"enabled": true}
        });
        let project = serde_json::json!({
            "hooks": {"disabledHooks": ["pre-tool"]},
            "memory": {"projectCharLimit": 999}
        });
        let settings = Settings::merge_layers(Some(pi), Some(global), Some(project));
        // hooks.enabled: pi=false, global=true → true
        assert!(settings.hooks.enabled);
        // hooks.scriptTimeoutSecs: pi=5, not overridden → 5
        assert_eq!(settings.hooks.script_timeout_secs, 5);
        // hooks.disabledHooks: project sets it
        assert_eq!(settings.hooks.disabled_hooks, vec!["pre-tool".to_string()]);
        // memory.globalCharLimit: pi=1000, not overridden → 1000
        assert_eq!(settings.memory.global_char_limit, 1000);
        // memory.projectCharLimit: project=999
        assert_eq!(settings.memory.project_char_limit, 999);
    }
}
