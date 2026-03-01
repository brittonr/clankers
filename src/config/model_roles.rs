//! Model roles — route different tasks to different models
//!
//! Supports role types:
//! - `default` — General-purpose model (used when no specific role matches)
//! - `smol` — Small/fast model for simple tasks (grep, file listing, etc.)
//! - `slow` — Large/expensive model for complex reasoning
//! - `plan` — Architecture/planning model (used in plan mode)
//! - `commit` — Model used for commit message generation
//! - `review` — Model used for code review
//!
//! Configuration lives in settings.json under `modelRoles`:
//! ```json
//! {
//!   "modelRoles": {
//!     "default": "claude-sonnet-4-5",
//!     "smol": "claude-haiku-3-5",
//!     "slow": "claude-opus-4-20250514",
//!     "plan": "claude-opus-4-20250514",
//!     "commit": "claude-haiku-3-5",
//!     "review": "claude-sonnet-4-5"
//!   }
//! }
//! ```

use std::collections::HashMap;
use std::fmt;

use serde::Deserialize;
use serde::Serialize;

/// A named role that maps to a specific model
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ModelRole {
    /// General-purpose, used when no specific role matches
    Default,
    /// Small/fast model for simple tasks
    Smol,
    /// Large/expensive model for complex reasoning
    Slow,
    /// Architecture and planning
    Plan,
    /// Commit message generation
    Commit,
    /// Code review
    Review,
}

impl fmt::Display for ModelRole {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ModelRole::Default => write!(f, "default"),
            ModelRole::Smol => write!(f, "smol"),
            ModelRole::Slow => write!(f, "slow"),
            ModelRole::Plan => write!(f, "plan"),
            ModelRole::Commit => write!(f, "commit"),
            ModelRole::Review => write!(f, "review"),
        }
    }
}

impl ModelRole {
    /// All known roles
    pub fn all() -> &'static [ModelRole] {
        &[
            ModelRole::Default,
            ModelRole::Smol,
            ModelRole::Slow,
            ModelRole::Plan,
            ModelRole::Commit,
            ModelRole::Review,
        ]
    }

    /// Parse from a role name string
    pub fn parse(s: &str) -> Option<ModelRole> {
        match s.to_lowercase().as_str() {
            "default" => Some(ModelRole::Default),
            "smol" | "small" | "fast" => Some(ModelRole::Smol),
            "slow" | "large" | "thinking" => Some(ModelRole::Slow),
            "plan" | "planning" | "architect" => Some(ModelRole::Plan),
            "commit" | "git" => Some(ModelRole::Commit),
            "review" | "code-review" => Some(ModelRole::Review),
            _ => None,
        }
    }

    /// Description of what this role is used for
    pub fn description(&self) -> &'static str {
        match self {
            ModelRole::Default => "General-purpose tasks",
            ModelRole::Smol => "Simple/fast tasks (file ops, grep, etc.)",
            ModelRole::Slow => "Complex reasoning and analysis",
            ModelRole::Plan => "Architecture and planning",
            ModelRole::Commit => "Commit message generation",
            ModelRole::Review => "Code review and analysis",
        }
    }
}

/// Configuration mapping roles to model IDs
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ModelRolesConfig {
    /// Role-to-model mapping
    #[serde(flatten)]
    pub roles: HashMap<ModelRole, String>,
}

impl ModelRolesConfig {
    /// Create a new empty config
    pub fn new() -> Self {
        Self { roles: HashMap::new() }
    }

    /// Create default role assignments based on a default model
    pub fn with_defaults(default_model: &str) -> Self {
        let mut roles = HashMap::new();
        roles.insert(ModelRole::Default, default_model.to_string());
        // Other roles fall back to default if not explicitly set
        Self { roles }
    }

    /// Resolve the model for a given role.
    /// Falls back to the default role, then to the provided fallback model.
    pub fn resolve(&self, role: ModelRole, fallback: &str) -> String {
        self.roles
            .get(&role)
            .or_else(|| self.roles.get(&ModelRole::Default))
            .cloned()
            .unwrap_or_else(|| fallback.to_string())
    }

    /// Set a role's model
    pub fn set(&mut self, role: ModelRole, model: String) {
        self.roles.insert(role, model);
    }

    /// Remove a role mapping (will fall back to default)
    pub fn unset(&mut self, role: ModelRole) {
        self.roles.remove(&role);
    }

    /// Get a human-readable summary of all role assignments
    pub fn summary(&self, fallback: &str) -> String {
        let mut lines = Vec::new();
        for role in ModelRole::all() {
            let model = self.resolve(*role, fallback);
            let is_explicit = self.roles.contains_key(role);
            let marker = if is_explicit { "" } else { " (inherited)" };
            lines.push(format!("  {:>8} → {}{}", role, model, marker));
        }
        lines.join("\n")
    }

    /// Check if any roles are explicitly configured
    pub fn is_configured(&self) -> bool {
        !self.roles.is_empty()
    }
}

/// Infer which role should be used for a given task/context.
/// This is a heuristic — the caller can always override.
pub fn infer_role_for_task(task_hint: &str) -> ModelRole {
    let lower = task_hint.to_lowercase();
    if lower.contains("commit") || lower.contains("changelog") || lower.contains("git") {
        ModelRole::Commit
    } else if lower.contains("review") || lower.contains("audit") || lower.contains("security") {
        ModelRole::Review
    } else if lower.contains("plan") || lower.contains("architect") || lower.contains("design") {
        ModelRole::Plan
    } else if lower.contains("grep")
        || lower.contains("find")
        || lower.contains("list")
        || lower.contains("read")
        || lower.contains("ls")
    {
        ModelRole::Smol
    } else if lower.contains("complex")
        || lower.contains("refactor")
        || lower.contains("think")
        || lower.contains("analyze")
    {
        ModelRole::Slow
    } else {
        ModelRole::Default
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resolve_fallback_chain() {
        let config = ModelRolesConfig::new();
        assert_eq!(config.resolve(ModelRole::Smol, "claude-sonnet"), "claude-sonnet");
    }

    #[test]
    fn test_resolve_explicit() {
        let mut config = ModelRolesConfig::new();
        config.set(ModelRole::Smol, "claude-haiku".to_string());
        assert_eq!(config.resolve(ModelRole::Smol, "claude-sonnet"), "claude-haiku");
    }

    #[test]
    fn test_resolve_falls_back_to_default_role() {
        let mut config = ModelRolesConfig::new();
        config.set(ModelRole::Default, "claude-sonnet".to_string());
        // Smol isn't set, should fall back to Default role
        assert_eq!(config.resolve(ModelRole::Smol, "fallback"), "claude-sonnet");
    }

    #[test]
    fn test_infer_role() {
        assert_eq!(infer_role_for_task("commit these changes"), ModelRole::Commit);
        assert_eq!(infer_role_for_task("review the code"), ModelRole::Review);
        assert_eq!(infer_role_for_task("plan the architecture"), ModelRole::Plan);
        assert_eq!(infer_role_for_task("grep for errors"), ModelRole::Smol);
        assert_eq!(infer_role_for_task("hello world"), ModelRole::Default);
    }

    #[test]
    fn test_summary() {
        let mut config = ModelRolesConfig::new();
        config.set(ModelRole::Default, "sonnet".to_string());
        config.set(ModelRole::Smol, "haiku".to_string());
        let s = config.summary("fallback");
        assert!(s.contains("sonnet"));
        assert!(s.contains("haiku"));
    }

    #[test]
    fn test_from_str() {
        assert_eq!(ModelRole::parse("smol"), Some(ModelRole::Smol));
        assert_eq!(ModelRole::parse("fast"), Some(ModelRole::Smol));
        assert_eq!(ModelRole::parse("unknown"), None);
    }
}
