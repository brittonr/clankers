//! Plugin sandbox (permission model)
//!
//! Permissions declared in `plugin.json` control what a plugin can do:
//!
//! - `fs:read` — read files from the host filesystem (via host functions)
//! - `fs:write` — write files on the host filesystem (via host functions)
//! - `net` — make HTTP requests (enforced via Extism `allowed_hosts`)
//! - `exec` — execute shell commands (via host functions)
//! - `ui` — send UI actions (set_widget, set_status, notify)
//!
//! The `"all"` wildcard grants every permission.

use serde::Deserialize;
use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Permission {
    #[serde(rename = "fs:read")]
    FsRead,
    #[serde(rename = "fs:write")]
    FsWrite,
    #[serde(rename = "net")]
    Net,
    #[serde(rename = "exec")]
    Exec,
    #[serde(rename = "ui")]
    Ui,
}

impl Permission {
    /// String representation matching `plugin.json` format.
    // r[impl plugin.perm.no-cross-grant]
    pub fn as_str(&self) -> &'static str {
        match self {
            Permission::FsRead => "fs:read",
            Permission::FsWrite => "fs:write",
            Permission::Net => "net",
            Permission::Exec => "exec",
            Permission::Ui => "ui",
        }
    }
}

/// Check if a set of permissions includes a specific permission.
// r[impl plugin.perm.all-grants-every]
// r[impl plugin.perm.explicit-match]
// r[impl plugin.perm.deny-without-grant]
pub fn has_permission(granted: &[String], required: Permission) -> bool {
    let required_str = required.as_str();
    granted.iter().any(|p| p == required_str || p == "all")
}

/// Validate that a plugin's tool call is allowed given its permissions.
/// Returns `Ok(())` if allowed, `Err(reason)` if denied.
pub fn check_tool_permission(granted: &[String], tool_name: &str) -> Result<(), String> {
    // All plugins can have their tool functions called — the permission
    // model gates what the tool *does* (fs, net, exec), not whether
    // the tool can be invoked at all. Tool invocation is always allowed
    // if the plugin is Active.
    let _ = (granted, tool_name);
    Ok(())
}

/// Validate that a plugin's event handler response is allowed.
/// Strips UI actions from plugins without the `ui` permission.
// r[impl plugin.filter.strips-without-ui]
// r[impl plugin.filter.passes-with-ui]
// r[impl plugin.filter.empty-passthrough]
pub fn filter_ui_actions<T>(granted: &[String], actions: Vec<T>) -> Vec<T> {
    if has_permission(granted, Permission::Ui) || actions.is_empty() {
        actions
    } else {
        tracing::warn!("Plugin tried to send UI actions without 'ui' permission — stripped");
        Vec::new()
    }
}
