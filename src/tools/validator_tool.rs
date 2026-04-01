//! Validation tool — host-side adapter for the self-validate plugin.
//!
//! When the self-validate plugin's `handle_tool_call` returns a response
//! with a `meta.prompt` field, the `ValidatorTool` spawns a separate clankers
//! subprocess to execute that validation prompt. This bridges the gap
//! between the WASM sandbox (which can't spawn processes) and the host.

use std::sync::Arc;

use async_trait::async_trait;
use clankers_tui_types::SubagentEvent;
use serde_json::Value;

use crate::plugin::PluginManager;
use crate::tools::Tool;
use crate::tools::ToolContext;
use crate::tools::ToolDefinition;
use crate::tools::ToolResult;

type PanelTx = tokio::sync::mpsc::UnboundedSender<SubagentEvent>;

/// A tool that combines plugin-built prompts with subprocess execution
/// for self-validating development workflows.
pub struct ValidatorTool {
    definition: ToolDefinition,
    plugin_name: String,
    function_name: String,
    manager: Arc<std::sync::Mutex<PluginManager>>,
    panel_tx: Option<PanelTx>,
}

impl ValidatorTool {
    pub fn new(
        definition: ToolDefinition,
        plugin_name: String,
        function_name: String,
        manager: Arc<std::sync::Mutex<PluginManager>>,
    ) -> Self {
        Self {
            definition,
            plugin_name,
            function_name,
            manager,
            panel_tx: None,
        }
    }

    pub fn with_panel_tx(mut self, tx: PanelTx) -> Self {
        self.panel_tx = Some(tx);
        self
    }
}

#[async_trait]
impl Tool for ValidatorTool {
    fn definition(&self) -> &ToolDefinition {
        &self.definition
    }

    fn source(&self) -> &str {
        &self.plugin_name
    }

    async fn execute(&self, ctx: &ToolContext, params: Value) -> ToolResult {
        // Phase 1: Call the WASM plugin to build the validation prompt
        let envelope = serde_json::json!({
            "tool": self.definition.name,
            "args": params,
        });
        let input = serde_json::to_string(&envelope).unwrap_or_default();

        let plugin_response = {
            let manager = match self.manager.lock() {
                Ok(m) => m,
                Err(e) => return ToolResult::error(format!("Plugin manager lock error: {}", e)),
            };

            match manager.call_plugin(&self.plugin_name, &self.function_name, &input) {
                Ok(output) => output,
                Err(e) => return ToolResult::error(format!("Plugin error: {}", e)),
            }
        };

        // Phase 2: Parse the plugin's response to extract the validator prompt
        let parsed: Value = match serde_json::from_str(&plugin_response) {
            Ok(v) => v,
            Err(e) => return ToolResult::error(format!("Failed to parse plugin response: {}", e)),
        };

        let status = parsed.get("status").and_then(|s| s.as_str()).unwrap_or("ok");
        if status != "ok" {
            let result = parsed.get("result").and_then(|r| r.as_str()).unwrap_or("Unknown error");
            return ToolResult::error(result.to_string());
        }

        // Extract the meta.prompt for subprocess execution
        let prompt = parsed
            .get("meta")
            .and_then(|m| m.get("prompt"))
            .and_then(|p| p.as_str())
            .unwrap_or_else(|| parsed.get("result").and_then(|r| r.as_str()).unwrap_or(""));

        if prompt.is_empty() {
            return ToolResult::error("Validator produced an empty prompt");
        }

        let cwd = parsed.get("meta").and_then(|m| m.get("cwd")).and_then(|c| c.as_str());
        let agent = parsed.get("meta").and_then(|m| m.get("agent")).and_then(|a| a.as_str());

        // Phase 3: Spawn the validator subprocess
        let worker_name = format!("validator:{}", ctx.call_id);

        crate::tools::delegate::run_worker_subprocess(
            &worker_name,
            prompt,
            agent,
            cwd,
            self.panel_tx.as_ref(),
            ctx.signal.clone(),
            None,
        )
        .await
    }
}
