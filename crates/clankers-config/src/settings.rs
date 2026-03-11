//! Settings loading (global + project JSON)

use std::path::Path;

use clankers_tui_types::MenuPlacement;
use serde::Deserialize;
use serde::Serialize;
use serde_json;

use clankers_agent_defs::definition::AgentScope;

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
                action: LeaderAction::SlashCommand(item.command.clone()),
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
            routing: None,
            cost_tracking: None,
            max_subagent_panes: default_max_subagent_panes(),
            disabled_tools: Vec::new(),
            hooks: clankers_hooks::HooksConfig::default(),
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

    /// Merge source object fields into target object
    fn merge_into(target: &mut serde_json::Value, source: &serde_json::Value) {
        if let (Some(target_obj), Some(source_obj)) = (target.as_object_mut(), source.as_object()) {
            for (key, value) in source_obj {
                target_obj.insert(key.clone(), value.clone());
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
}
