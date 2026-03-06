# Spec: Extensible Model Roles

## Overview

Replace the `ModelRole` enum (6 fixed variants) with a string-keyed map.
Users can define custom roles in settings. The builtin roles are seeded as
defaults.

## Current Pain

```rust
pub enum ModelRole { Default, Smol, Slow, Plan, Commit, Review }
```

Users who want roles like `debug`, `test`, `security`, `translate` can't add
them. The `/role` help text hardcodes the list. `infer_role_for_task()` has
hardcoded keyword lists.

## New Types

```rust
// src/config/model_roles.rs

/// A role definition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelRoleDef {
    /// Role name (lowercase).
    pub name: String,
    /// Human-readable description.
    pub description: String,
    /// Assigned model (None = use default).
    #[serde(default)]
    pub model: Option<String>,
    /// Keywords for auto-inference from task text.
    #[serde(default)]
    pub keywords: Vec<String>,
}

/// Role registry.
pub struct ModelRoles {
    roles: IndexMap<String, ModelRoleDef>,
}

impl ModelRoles {
    /// Create with the 6 builtin roles as defaults.
    pub fn with_defaults() -> Self { ... }

    /// Merge user-defined roles (adds new, overrides existing).
    pub fn merge_user_roles(&mut self, user_roles: Vec<ModelRoleDef>) {
        for role in user_roles {
            self.roles.insert(role.name.clone(), role);
        }
    }

    pub fn get(&self, name: &str) -> Option<&ModelRoleDef> {
        self.roles.get(name)
    }

    pub fn all(&self) -> impl Iterator<Item = &ModelRoleDef> {
        self.roles.values()
    }

    /// Infer a role from task text by matching keywords.
    pub fn infer(&self, task: &str) -> &str {
        let lower = task.to_lowercase();
        for role in self.roles.values() {
            if role.keywords.iter().any(|kw| lower.contains(kw)) {
                return &role.name;
            }
        }
        "default"
    }
}
```

## User Config

```toml
# settings.toml
[[model_roles]]
name = "debug"
description = "Debugging and tracing"
model = "claude-sonnet-4-5-20250514"
keywords = ["debug", "trace", "backtrace", "panic", "stacktrace"]

[[model_roles]]
name = "test"
description = "Writing and running tests"
model = "claude-sonnet-4-5-20250514"
keywords = ["test", "spec", "assert", "coverage"]

# Override a builtin
[[model_roles]]
name = "review"
description = "Code review"
model = "claude-opus-4-6-20250610"
keywords = ["review", "audit", "inspect"]
```

## `/role` Command Update

The help text becomes dynamic:

```rust
impl SlashHandler for RoleHandler {
    fn handle(&self, args: &str, ctx: &mut SlashContext) -> SlashResult {
        if args.is_empty() {
            // List all roles dynamically
            let lines: Vec<String> = ctx.roles.all()
                .map(|r| format!("  {} — {}", r.name, r.description))
                .collect();
            return SlashResult::Message(lines.join("\n"));
        }
        // ...
    }
}
```

## Migration

1. Replace `ModelRole` enum with `ModelRoles` struct.
2. `ModelRoles::with_defaults()` seeds the 6 builtins.
3. At init, merge user-defined roles from settings.
4. Update all `match role { ... }` sites to use `roles.get(name)`.
5. Delete `ModelRole::parse()`, `ModelRole::all()`, `ModelRole::description()`.
