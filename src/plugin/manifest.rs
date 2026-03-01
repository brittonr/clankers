//! Plugin manifest (plugin.json)

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

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PluginKind {
    #[default]
    Extism,
    Zellij,
}

impl PluginManifest {
    pub fn load(path: &Path) -> Option<Self> {
        let content = std::fs::read_to_string(path).ok()?;
        serde_json::from_str(&content).ok()
    }
}
