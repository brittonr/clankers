//! Plugin manifest (plugin.json)

#![allow(unexpected_cfgs)]
#![cfg_attr(dylint_lib = "tigerstyle", allow(compound_assertion, ignored_result, no_unwrap, no_panic, no_todo, unjustified_no_todo_allow, no_recursion, unchecked_narrowing, unchecked_division, unbounded_loop, catch_all_on_enum, explicit_defaults, unbounded_channel, unbounded_collection_growth, assertion_density, raw_arithmetic_overflow, sentinel_fallback, acronym_style, bool_naming, negated_predicate, numeric_units, float_for_currency, function_length, nested_conditionals, platform_dependent_cast, usize_in_public_api, too_many_parameters, compound_condition, unjustified_allow, ambiguous_params, ambient_clock, verified_purity, contradictory_time, multi_lock_ordering, reason = "full workspace tigerstyle audit gate: legacy debt documented locally while cleanup proceeds incrementally"))]

use std::collections::HashMap;
use std::path::Path;

use serde::Deserialize;
use serde::Serialize;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginManifest {
    pub name: String,
    pub version: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub wasm: Option<String>,
    #[serde(default)]
    pub kind: PluginKind,
    #[serde(default)]
    pub stdio: Option<StdioManifest>,
    #[serde(default)]
    pub permissions: Vec<String>,
    #[serde(default)]
    pub tools: Vec<String>,
    #[serde(default)]
    pub commands: Vec<String>,
    #[serde(default)]
    pub events: Vec<String>,
    /// Detailed tool definitions with descriptions and JSON schemas.
    /// When present, these are used instead of calling `describe` on the plugin.
    #[serde(default)]
    pub tool_definitions: Vec<ToolManifest>,
    /// Optional host allowlist for HTTP access.
    /// Only takes effect when plugin also has "net" permission.
    #[serde(default)]
    pub allowed_hosts: Option<Vec<String>>,

    /// Maps plugin config keys to environment variable names.
    /// At load time, each env var is resolved and passed to the plugin via Extism config.
    #[serde(default)]
    pub config_env: HashMap<String, String>,

    /// Optional leader menu entries contributed by this plugin.
    #[serde(default)]
    pub leader_menu: Vec<PluginLeaderEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct StdioManifest {
    /// Executable to launch for the stdio plugin runtime.
    #[serde(default)]
    pub command: Option<String>,
    /// Arguments passed to the executable.
    #[serde(default)]
    pub args: Vec<String>,
    /// Optional working directory policy.
    #[serde(default)]
    pub working_dir: Option<PluginWorkingDirectory>,
    /// Environment variables forwarded to the child process.
    /// In v1, every allowlisted variable is required.
    #[serde(default)]
    pub env_allowlist: Vec<String>,
    /// Explicit sandbox mode.
    #[serde(default)]
    pub sandbox: Option<PluginSandboxMode>,
    /// Additional writable roots granted in restricted mode.
    #[serde(default)]
    pub writable_roots: Vec<String>,
    /// Whether restricted mode may allow outbound network access when the
    /// manifest permissions also allow it.
    #[serde(default)]
    pub allow_network: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum PluginWorkingDirectory {
    PluginDir,
    ProjectRoot,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum PluginSandboxMode {
    Inherit,
    Restricted,
}

/// A leader menu entry declared in a plugin's `plugin.json`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginLeaderEntry {
    /// Key to press in the leader menu.
    pub key: char,
    /// Display label.
    pub label: String,
    /// Slash command to execute (e.g. "/cal list").
    pub command: String,
    /// Submenu name. If omitted, goes to root.
    /// "plugins" is conventional for plugin top-level items.
    #[serde(default)]
    pub submenu: Option<String>,
}

/// A tool definition inside the plugin manifest, providing metadata
/// that clankers uses to register the tool with the LLM.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolManifest {
    pub name: String,
    #[serde(default)]
    pub description: String,
    /// The WASM function name to call. Defaults to `handle_tool_call`.
    #[serde(default = "default_handler")]
    pub handler: String,
    /// JSON Schema for the tool's input parameters.
    #[serde(default = "default_input_schema")]
    pub input_schema: serde_json::Value,
}

fn default_handler() -> String {
    "handle_tool_call".to_string()
}

fn default_input_schema() -> serde_json::Value {
    serde_json::json!({
        "type": "object",
        "properties": {},
        "additionalProperties": true
    })
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum PluginKind {
    #[default]
    Extism,
    Zellij,
    Stdio,
}

impl PluginKind {
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Extism => "extism",
            Self::Zellij => "zellij",
            Self::Stdio => "stdio",
        }
    }

    pub const fn uses_wasm_runtime(&self) -> bool {
        matches!(self, Self::Extism)
    }
}

impl std::fmt::Display for PluginKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ManifestValidationError {
    MissingStdioLaunchPolicy,
    MissingStdioCommand,
    EmptyStdioCommand,
    MissingStdioSandbox,
    UnexpectedStdioLaunchPolicy,
}

impl std::fmt::Display for ManifestValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MissingStdioLaunchPolicy => f.write_str("stdio plugins must declare a `stdio` launch policy"),
            Self::MissingStdioCommand => f.write_str("stdio plugins must declare `stdio.command`"),
            Self::EmptyStdioCommand => f.write_str("stdio plugin `stdio.command` cannot be blank"),
            Self::MissingStdioSandbox => f.write_str("stdio plugins must declare `stdio.sandbox`"),
            Self::UnexpectedStdioLaunchPolicy => {
                f.write_str("non-stdio plugins cannot declare a `stdio` launch policy")
            }
        }
    }
}

impl std::error::Error for ManifestValidationError {}

impl PluginManifest {
    pub fn load(path: &Path) -> Option<Self> {
        let content = std::fs::read_to_string(path).ok()?;
        serde_json::from_str(&content).ok()
    }

    pub fn validate(&self) -> Result<(), ManifestValidationError> {
        match self.kind {
            PluginKind::Stdio => {
                let stdio = self.stdio.as_ref().ok_or(ManifestValidationError::MissingStdioLaunchPolicy)?;
                match stdio.command.as_deref() {
                    Some(command) if !command.trim().is_empty() => {}
                    Some(_) => return Err(ManifestValidationError::EmptyStdioCommand),
                    None => return Err(ManifestValidationError::MissingStdioCommand),
                }
                if stdio.sandbox.is_none() {
                    return Err(ManifestValidationError::MissingStdioSandbox);
                }
            }
            PluginKind::Extism | PluginKind::Zellij => {
                if self.stdio.is_some() {
                    return Err(ManifestValidationError::UnexpectedStdioLaunchPolicy);
                }
            }
        }

        Ok(())
    }
}
