//! Host functions exposed to WASM plugins
//!
//! Plugins interact with the host in two ways:
//!
//! 1. **Return-based protocol** — `on_event` / `handle_tool_call` return JSON with optional `"ui"`
//!    actions and `"message"` fields.
//!
//! 2. **Request-based protocol** — tool handlers can include a `"host_calls"` array in their
//!    response. Each entry is a host function invocation that the host executes and returns results
//!    for.
//!
//! ## UI Actions (return-based)
//!
//! ```json
//! {
//!   "handled": true,
//!   "message": "optional chat message",
//!   "display": true,
//!   "ui": [
//!     {"action": "set_widget", "widget": {"type": "Text", "content": "Hello", "bold": true}},
//!     {"action": "set_status", "text": "running", "color": "green"},
//!     {"action": "notify", "message": "Build complete!", "level": "info"}
//!   ]
//! }
//! ```
//!
//! ### Available UI Actions
//!
//! - `set_widget` — Set or replace the plugin's widget panel in the TUI
//! - `clear_widget` — Remove the plugin's widget panel
//! - `set_status` — Set the plugin's status bar segment
//! - `clear_status` — Remove the plugin's status bar segment
//! - `notify` — Show a toast notification (level: info/warning/error)
//!
//! ### Widget Types
//!
//! - `Text` — Styled text (`content`, `bold`, `color`)
//! - `Box` — Container with children (`children`, `direction`: vertical/horizontal)
//! - `List` — Selectable list (`items`, `selected`)
//! - `Input` — Text input display (`value`, `placeholder`)
//! - `Spacer` — Vertical space (`lines`)
//! - `Progress` — Progress bar (`label`, `value` 0.0–1.0, `color`)
//! - `Table` — Data table (`rows`, `headers`)
//!
//! ## Host Calls (request-based)
//!
//! Tool responses may include `"host_calls"` to request host actions:
//!
//! ```json
//! {
//!   "tool": "my_tool",
//!   "result": "...",
//!   "status": "ok",
//!   "host_calls": [
//!     {"fn": "log", "args": {"level": "info", "message": "processed 42 items"}},
//!     {"fn": "get_config", "args": {"key": "api_url"}}
//!   ]
//! }
//! ```

#![allow(unexpected_cfgs)]
#![cfg_attr(dylint_lib = "tigerstyle", allow(compound_assertion, ignored_result, no_unwrap, no_panic, no_todo, unjustified_no_todo_allow, no_recursion, unchecked_narrowing, unchecked_division, unbounded_loop, catch_all_on_enum, explicit_defaults, unbounded_channel, unbounded_collection_growth, assertion_density, raw_arithmetic_overflow, sentinel_fallback, acronym_style, bool_naming, negated_predicate, numeric_units, float_for_currency, function_length, nested_conditionals, platform_dependent_cast, usize_in_public_api, too_many_parameters, compound_condition, unjustified_allow, ambiguous_params, ambient_clock, verified_purity, contradictory_time, multi_lock_ordering, reason = "full workspace tigerstyle audit gate: legacy debt documented locally while cleanup proceeds incrementally"))]

use std::collections::HashMap;

use serde::Deserialize;
use serde::Serialize;

/// A host function call requested by a plugin.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HostCall {
    /// Function name (e.g. "log", "get_config", "read_file")
    #[serde(rename = "fn")]
    pub function: String,
    /// Arguments to the function
    #[serde(default)]
    pub args: serde_json::Value,
}

/// Result of executing a host call.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HostCallResult {
    /// Whether the call succeeded
    pub ok: bool,
    /// Result data (on success) or error message (on failure)
    pub data: serde_json::Value,
}

impl HostCallResult {
    pub fn success(data: serde_json::Value) -> Self {
        Self { ok: true, data }
    }

    pub fn error(msg: impl Into<String>) -> Self {
        Self {
            ok: false,
            data: serde_json::Value::String(msg.into()),
        }
    }
}

/// Host function registry.
///
/// Executes host-side calls requested by plugins. Each function checks
/// the plugin's permissions before proceeding.
pub struct HostFunctions {
    /// Plugin config values (injected at load time, readable via get_config)
    config: HashMap<String, String>,
}

impl HostFunctions {
    pub fn new() -> Self {
        Self { config: HashMap::new() }
    }

    /// Create with initial config values.
    pub fn with_config(config: HashMap<String, String>) -> Self {
        Self { config }
    }

    /// List of available host function names.
    pub fn available_functions() -> Vec<&'static str> {
        vec![
            // UI (return-based)
            "set_widget",
            "clear_widget",
            "set_status",
            "clear_status",
            "notify",
            // Request-based
            "log",
            "get_config",
            "get_env",
            "read_file",
            "write_file",
            "list_dir",
        ]
    }

    /// Execute a host call, checking permissions.
    // r[impl plugin.host.fs-read-gated]
    // r[impl plugin.host.fs-write-gated]
    // r[impl plugin.host.ungated-functions]
    // r[impl plugin.host.unknown-rejects]
    pub fn execute(&self, call: &HostCall, permissions: &[String]) -> HostCallResult {
        use crate::sandbox;
        use crate::sandbox::Permission;

        match call.function.as_str() {
            "log" => self.host_log(&call.args),
            "get_config" => self.host_get_config(&call.args),
            "get_env" => self.host_get_env(&call.args),
            "read_file" => {
                if !sandbox::has_permission(permissions, Permission::FsRead) {
                    return HostCallResult::error("Permission denied: requires 'fs:read'");
                }
                self.host_read_file(&call.args)
            }
            "write_file" => {
                if !sandbox::has_permission(permissions, Permission::FsWrite) {
                    return HostCallResult::error("Permission denied: requires 'fs:write'");
                }
                self.host_write_file(&call.args)
            }
            "list_dir" => {
                if !sandbox::has_permission(permissions, Permission::FsRead) {
                    return HostCallResult::error("Permission denied: requires 'fs:read'");
                }
                self.host_list_dir(&call.args)
            }
            other => HostCallResult::error(format!("Unknown host function: {other}")),
        }
    }

    /// Process all host_calls from a plugin response.
    pub fn process_host_calls(&self, response: &serde_json::Value, permissions: &[String]) -> Vec<HostCallResult> {
        let Some(calls_val) = response.get("host_calls") else {
            return Vec::new();
        };
        let Some(calls_arr) = calls_val.as_array() else {
            return Vec::new();
        };

        calls_arr
            .iter()
            .map(|v| match serde_json::from_value::<HostCall>(v.clone()) {
                Ok(call) => self.execute(&call, permissions),
                Err(e) => HostCallResult::error(format!("Malformed host_call: {e}")),
            })
            .collect()
    }

    // ── Host function implementations ────────────────────────────

    fn host_log(&self, args: &serde_json::Value) -> HostCallResult {
        let level = args.get("level").and_then(|v| v.as_str()).unwrap_or("info");
        let message = args.get("message").and_then(|v| v.as_str()).unwrap_or("");
        match level {
            "error" => tracing::error!(plugin = true, "{}", message),
            "warn" => tracing::warn!(plugin = true, "{}", message),
            "debug" => tracing::debug!(plugin = true, "{}", message),
            _ => tracing::info!(plugin = true, "{}", message),
        }
        HostCallResult::success(serde_json::Value::Null)
    }

    fn host_get_config(&self, args: &serde_json::Value) -> HostCallResult {
        let key = args.get("key").and_then(|v| v.as_str()).unwrap_or("");
        match self.config.get(key) {
            Some(val) => HostCallResult::success(serde_json::Value::String(val.clone())),
            None => HostCallResult::success(serde_json::Value::Null),
        }
    }

    fn host_get_env(&self, args: &serde_json::Value) -> HostCallResult {
        let key = args.get("key").and_then(|v| v.as_str()).unwrap_or("");
        if key.is_empty() {
            return HostCallResult::error("Missing 'key' argument");
        }
        match std::env::var(key) {
            Ok(val) => HostCallResult::success(serde_json::Value::String(val)),
            Err(_) => HostCallResult::success(serde_json::Value::Null),
        }
    }

    fn host_read_file(&self, args: &serde_json::Value) -> HostCallResult {
        let path = args.get("path").and_then(|v| v.as_str()).unwrap_or("");
        if path.is_empty() {
            return HostCallResult::error("Missing 'path' argument");
        }
        // Size guard: 1MB max for plugin reads
        const MAX_READ: u64 = 1_048_576;
        let meta = match std::fs::metadata(path) {
            Ok(m) => m,
            Err(e) => return HostCallResult::error(format!("Cannot read '{}': {}", path, e)),
        };
        if meta.len() > MAX_READ {
            return HostCallResult::error(format!("File too large: {} bytes (max {})", meta.len(), MAX_READ));
        }
        match std::fs::read_to_string(path) {
            Ok(contents) => HostCallResult::success(serde_json::Value::String(contents)),
            Err(e) => HostCallResult::error(format!("Cannot read '{}': {}", path, e)),
        }
    }

    fn host_write_file(&self, args: &serde_json::Value) -> HostCallResult {
        let path = args.get("path").and_then(|v| v.as_str()).unwrap_or("");
        let content = args.get("content").and_then(|v| v.as_str()).unwrap_or("");
        if path.is_empty() {
            return HostCallResult::error("Missing 'path' argument");
        }
        match std::fs::write(path, content) {
            Ok(()) => HostCallResult::success(serde_json::json!({"bytes_written": content.len()})),
            Err(e) => HostCallResult::error(format!("Cannot write '{}': {}", path, e)),
        }
    }

    fn host_list_dir(&self, args: &serde_json::Value) -> HostCallResult {
        let path = args.get("path").and_then(|v| v.as_str()).unwrap_or(".");
        match std::fs::read_dir(path) {
            Ok(entries) => {
                let names: Vec<serde_json::Value> = entries
                    .filter_map(|e| e.ok())
                    .map(|e| {
                        let name = e.file_name().to_string_lossy().to_string();
                        let is_dir = e.file_type().map(|t| t.is_dir()).unwrap_or(false);
                        serde_json::json!({"name": name, "is_dir": is_dir})
                    })
                    .collect();
                HostCallResult::success(serde_json::Value::Array(names))
            }
            Err(e) => HostCallResult::error(format!("Cannot list '{}': {}", path, e)),
        }
    }
}

impl Default for HostFunctions {
    fn default() -> Self {
        Self::new()
    }
}
