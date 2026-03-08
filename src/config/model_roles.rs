//! Model roles — route different tasks to different models
//!
//! Roles map names to model IDs. Six builtins are seeded by default;
//! users can override them or add new ones in settings.toml:
//!
//! ```toml
//! [[model_roles]]
//! name = "debug"
//! description = "Debugging and tracing"
//! model = "claude-sonnet-4-5-20250514"
//! keywords = ["debug", "trace", "backtrace", "panic"]
//! ```

use std::fmt;

use indexmap::IndexMap;
use serde::Deserialize;
use serde::Serialize;

// ── Role definition ─────────────────────────────────────────────────────────

/// A single role definition: name, description, optional model, and keywords
/// for auto-inference.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelRoleDef {
    /// Role name (lowercase, e.g. "smol", "debug").
    pub name: String,
    /// Human-readable description shown in `/role` output.
    pub description: String,
    /// Assigned model ID. `None` means inherit from the "default" role,
    /// then fall back to the active model.
    #[serde(default)]
    pub model: Option<String>,
    /// Keywords used by `infer_role()` to auto-select this role from task text.
    #[serde(default)]
    pub keywords: Vec<String>,
}

impl fmt::Display for ModelRoleDef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.name)
    }
}

// ── Role registry ───────────────────────────────────────────────────────────

/// Ordered map of role definitions. Builtin roles are seeded first;
/// user-defined roles can override or extend them.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelRoles {
    #[serde(flatten)]
    roles: IndexMap<String, ModelRoleDef>,
}

impl Default for ModelRoles {
    fn default() -> Self {
        Self::with_defaults()
    }
}

impl ModelRoles {
    /// Seed the six builtin roles.
    pub fn with_defaults() -> Self {
        let mut roles = IndexMap::new();
        let builtins = [
            ("default", "General-purpose tasks", None, vec![]),
            ("smol", "Simple/fast tasks (file ops, grep, etc.)", None,
             vec!["grep", "find", "list", "read", "ls"]),
            ("slow", "Complex reasoning and analysis", None,
             vec!["complex", "refactor", "think", "analyze"]),
            ("plan", "Architecture and planning", None,
             vec!["plan", "architect", "design"]),
            ("commit", "Commit message generation", None,
             vec!["commit", "changelog", "git"]),
            ("review", "Code review and analysis", None,
             vec!["review", "audit", "security"]),
        ];
        for (name, desc, model, kws) in builtins {
            roles.insert(name.to_string(), ModelRoleDef {
                name: name.to_string(),
                description: desc.to_string(),
                model,
                keywords: kws.into_iter().map(String::from).collect(),
            });
        }
        Self { roles }
    }

    /// Merge user-defined roles. New names are added; existing names are
    /// overridden (letting users change builtins).
    pub fn merge(&mut self, user_roles: Vec<ModelRoleDef>) {
        for role in user_roles {
            self.roles.insert(role.name.clone(), role);
        }
    }

    /// Look up a role by name. Accepts aliases for builtin roles
    /// (e.g. "fast" → "smol", "large" → "slow").
    pub fn get(&self, name: &str) -> Option<&ModelRoleDef> {
        let binding = name.to_lowercase();
        let canonical = match binding.as_str() {
            "small" | "fast" => "smol",
            "large" | "thinking" => "slow",
            "planning" | "architect" => "plan",
            "git" => "commit",
            "code-review" => "review",
            other => other,
        };
        self.roles.get(canonical)
    }

    /// Iterate over all roles in insertion order.
    pub fn all(&self) -> impl Iterator<Item = &ModelRoleDef> {
        self.roles.values()
    }

    /// Resolve the model for a role. Falls back to the "default" role's model,
    /// then to the provided fallback.
    pub fn resolve(&self, name: &str, fallback: &str) -> String {
        if let Some(role) = self.get(name)
            && let Some(ref m) = role.model
        {
            return m.clone();
        }
        // Fall back to "default" role's model
        if let Some(def) = self.roles.get("default")
            && let Some(ref m) = def.model
        {
            return m.clone();
        }
        fallback.to_string()
    }

    /// Set a role's model. If the role doesn't exist, creates it.
    pub fn set_model(&mut self, name: &str, model: String) {
        if let Some(role) = self.roles.get_mut(name) {
            role.model = Some(model);
        } else {
            self.roles.insert(name.to_string(), ModelRoleDef {
                name: name.to_string(),
                description: format!("Custom role: {}", name),
                model: Some(model),
                keywords: vec![],
            });
        }
    }

    /// Clear all role model overrides.
    pub fn reset(&mut self) {
        for role in self.roles.values_mut() {
            role.model = None;
        }
    }

    /// Human-readable summary of all role assignments.
    pub fn summary(&self, fallback: &str) -> String {
        let mut lines = Vec::new();
        for role in self.roles.values() {
            let model = self.resolve(&role.name, fallback);
            let is_explicit = role.model.is_some();
            let marker = if is_explicit { "" } else { " (inherited)" };
            lines.push(format!("  {:>8} → {}{}", role.name, model, marker));
        }
        lines.join("\n")
    }

    /// Infer a role from task text by matching keywords.
    pub fn infer(&self, task: &str) -> &str {
        let lower = task.to_lowercase();
        for role in self.roles.values() {
            if !role.keywords.is_empty() && role.keywords.iter().any(|kw| lower.contains(kw.as_str())) {
                return &role.name;
            }
        }
        "default"
    }

    /// All role names (for tab completion, help text, etc.).
    pub fn names(&self) -> Vec<&str> {
        self.roles.keys().map(|s| s.as_str()).collect()
    }

    /// Check if any roles have explicit model overrides.
    pub fn is_configured(&self) -> bool {
        self.roles.values().any(|r| r.model.is_some())
    }
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_defaults_have_six_roles() {
        let roles = ModelRoles::with_defaults();
        assert_eq!(roles.roles.len(), 6);
        assert!(roles.get("default").is_some());
        assert!(roles.get("smol").is_some());
        assert!(roles.get("slow").is_some());
        assert!(roles.get("plan").is_some());
        assert!(roles.get("commit").is_some());
        assert!(roles.get("review").is_some());
    }

    #[test]
    fn test_aliases() {
        let roles = ModelRoles::with_defaults();
        assert_eq!(roles.get("fast").expect("fast alias should exist").name, "smol");
        assert_eq!(roles.get("small").expect("small alias should exist").name, "smol");
        assert_eq!(roles.get("large").expect("large alias should exist").name, "slow");
        assert_eq!(roles.get("thinking").expect("thinking alias should exist").name, "slow");
        assert_eq!(roles.get("architect").expect("architect alias should exist").name, "plan");
        assert_eq!(roles.get("git").expect("git alias should exist").name, "commit");
        assert_eq!(roles.get("code-review").expect("code-review alias should exist").name, "review");
    }

    #[test]
    fn test_resolve_fallback_chain() {
        let roles = ModelRoles::with_defaults();
        // No models set → falls back to provided fallback
        assert_eq!(roles.resolve("smol", "claude-sonnet"), "claude-sonnet");
    }

    #[test]
    fn test_resolve_explicit() {
        let mut roles = ModelRoles::with_defaults();
        roles.set_model("smol", "claude-haiku".to_string());
        assert_eq!(roles.resolve("smol", "claude-sonnet"), "claude-haiku");
    }

    #[test]
    fn test_resolve_falls_back_to_default_role() {
        let mut roles = ModelRoles::with_defaults();
        roles.set_model("default", "claude-sonnet".to_string());
        // smol has no model → falls back to default role's model
        assert_eq!(roles.resolve("smol", "fallback"), "claude-sonnet");
    }

    #[test]
    fn test_merge_user_roles() {
        let mut roles = ModelRoles::with_defaults();
        roles.merge(vec![
            ModelRoleDef {
                name: "debug".to_string(),
                description: "Debugging".to_string(),
                model: Some("claude-sonnet".to_string()),
                keywords: vec!["debug".to_string(), "trace".to_string()],
            },
            // Override builtin
            ModelRoleDef {
                name: "review".to_string(),
                description: "Custom review".to_string(),
                model: Some("claude-opus".to_string()),
                keywords: vec!["review".to_string()],
            },
        ]);
        assert_eq!(roles.roles.len(), 7); // 6 builtins + 1 new
        assert_eq!(roles.get("debug").expect("debug role should exist").model.as_deref(), Some("claude-sonnet"));
        assert_eq!(roles.get("review").expect("review role should exist").description, "Custom review");
    }

    #[test]
    fn test_infer_role() {
        let roles = ModelRoles::with_defaults();
        assert_eq!(roles.infer("commit these changes"), "commit");
        assert_eq!(roles.infer("review the code"), "review");
        assert_eq!(roles.infer("plan the architecture"), "plan");
        assert_eq!(roles.infer("grep for errors"), "smol");
        assert_eq!(roles.infer("hello world"), "default");
    }

    #[test]
    fn test_infer_user_role() {
        let mut roles = ModelRoles::with_defaults();
        roles.merge(vec![ModelRoleDef {
            name: "debug".to_string(),
            description: "Debugging".to_string(),
            model: None,
            keywords: vec!["debug".to_string(), "backtrace".to_string()],
        }]);
        assert_eq!(roles.infer("debug this crash"), "debug");
        assert_eq!(roles.infer("show backtrace"), "debug");
    }

    #[test]
    fn test_summary() {
        let mut roles = ModelRoles::with_defaults();
        roles.set_model("default", "sonnet".to_string());
        roles.set_model("smol", "haiku".to_string());
        let s = roles.summary("fallback");
        assert!(s.contains("sonnet"));
        assert!(s.contains("haiku"));
    }

    #[test]
    fn test_reset() {
        let mut roles = ModelRoles::with_defaults();
        roles.set_model("smol", "haiku".to_string());
        assert!(roles.is_configured());
        roles.reset();
        assert!(!roles.is_configured());
    }

    #[test]
    fn test_names() {
        let roles = ModelRoles::with_defaults();
        let names = roles.names();
        assert_eq!(names, vec!["default", "smol", "slow", "plan", "commit", "review"]);
    }
}
