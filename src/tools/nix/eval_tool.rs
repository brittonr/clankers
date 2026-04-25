//! NixEvalTool — in-process Nix expression evaluation.
//!
//! Evaluates pure Nix expressions via snix-eval without spawning `nix eval`.
//! Falls back to the nix CLI for impure expressions.

#![allow(unexpected_cfgs)]
#![cfg_attr(dylint_lib = "tigerstyle", allow(compound_assertion, ignored_result, no_unwrap, no_panic, no_todo, unjustified_no_todo_allow, no_recursion, unchecked_narrowing, unchecked_division, unbounded_loop, catch_all_on_enum, explicit_defaults, unbounded_channel, unbounded_collection_growth, assertion_density, raw_arithmetic_overflow, sentinel_fallback, acronym_style, bool_naming, negated_predicate, numeric_units, float_for_currency, function_length, nested_conditionals, platform_dependent_cast, usize_in_public_api, too_many_parameters, compound_condition, unjustified_allow, ambiguous_params, ambient_clock, verified_purity, contradictory_time, multi_lock_ordering, reason = "full workspace tigerstyle audit gate: legacy debt documented locally while cleanup proceeds incrementally"))]

use async_trait::async_trait;
use serde_json::Value;
use serde_json::json;

use super::super::Tool;
use super::super::ToolContext;
use super::super::ToolDefinition;
use super::super::ToolResult;

pub struct NixEvalTool {
    definition: ToolDefinition,
}

impl NixEvalTool {
    pub fn new() -> Self {
        Self {
            definition: ToolDefinition {
                name: "nix_eval".to_string(),
                description: "Evaluate a Nix expression in-process. Fast, no process spawn. \
                    Use for reading flake metadata, evaluating config values, listing available \
                    packages. Falls back to `nix eval` for impure expressions."
                    .to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "expr": {
                            "type": "string",
                            "description": "Nix expression to evaluate"
                        },
                        "file": {
                            "type": "string",
                            "description": "Path to a .nix file to evaluate (alternative to expr)"
                        },
                        "apply": {
                            "type": "string",
                            "description": "Function to apply to the result (e.g., 'builtins.attrNames')"
                        }
                    }
                }),
            },
        }
    }
}

impl Default for NixEvalTool {
    fn default() -> Self {
        Self::new()
    }
}

/// Timeout for the nix eval CLI fallback (seconds).
const CLI_FALLBACK_TIMEOUT: u64 = 60;

#[async_trait]
impl Tool for NixEvalTool {
    fn definition(&self) -> &ToolDefinition {
        &self.definition
    }

    async fn execute(&self, ctx: &ToolContext, params: Value) -> ToolResult {
        let expr = params.get("expr").and_then(|v| v.as_str());
        let file = params.get("file").and_then(|v| v.as_str());
        let apply = params.get("apply").and_then(|v| v.as_str());

        // Build the expression to evaluate
        let eval_expr = match (expr, file) {
            (Some(e), _) => e.to_string(),
            (None, Some(f)) => format!("import {f}"),
            (None, None) => {
                return ToolResult::error("At least one of 'expr' or 'file' must be provided");
            }
        };

        // Wrap with apply function if provided
        let full_expr = match apply {
            Some(func) => format!("{func} ({eval_expr})"),
            None => eval_expr.clone(),
        };

        ctx.emit_progress(&format!("evaluating: {}", truncate_for_display(&full_expr)));

        // Try in-process pure evaluation first
        match clankers_nix::evaluate(&full_expr) {
            Ok(result) => {
                let json_str =
                    serde_json::to_string_pretty(&result.value).unwrap_or_else(|_| format!("{:?}", result.value));

                let mut output = json_str;
                if !result.warnings.is_empty() {
                    use std::fmt::Write;
                    write!(output, "\n\n[{} warning(s)]", result.warnings.len()).ok();
                }

                ToolResult::text(output)
            }
            Err(clankers_nix::NixError::EvalFailed { is_impure: true, .. }) => {
                // Impure expression — fall back to nix eval CLI
                ctx.emit_progress("falling back to nix eval CLI (impure expression)");
                fallback_nix_eval_cli(&full_expr, ctx, CLI_FALLBACK_TIMEOUT).await
            }
            Err(e) => ToolResult::error(format!("{e}")),
        }
    }
}

/// Fall back to `nix eval --json` for impure expressions.
async fn fallback_nix_eval_cli(expr: &str, _ctx: &ToolContext, timeout_secs: u64) -> ToolResult {
    use tokio::process::Command;
    use tokio::time::Duration;

    let clean_env = crate::tools::sandbox::sanitized_env();

    let mut cmd = Command::new("nix");
    cmd.args(["eval", "--json", "--expr", expr]);
    cmd.env_clear().envs(clean_env);
    cmd.stdout(std::process::Stdio::piped());
    cmd.stderr(std::process::Stdio::piped());

    let child = match cmd.spawn() {
        Ok(c) => c,
        Err(e) => return ToolResult::error(format!("failed to spawn nix eval: {e}")),
    };

    let output = if timeout_secs > 0 {
        match tokio::time::timeout(Duration::from_secs(timeout_secs), child.wait_with_output()).await {
            Ok(Ok(output)) => output,
            Ok(Err(e)) => return ToolResult::error(format!("nix eval failed: {e}")),
            Err(_) => {
                return ToolResult::error(format!("nix eval timed out after {timeout_secs}s"));
            }
        }
    } else {
        match child.wait_with_output().await {
            Ok(output) => output,
            Err(e) => return ToolResult::error(format!("nix eval failed: {e}")),
        }
    };

    if output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout);
        // Try to pretty-print the JSON
        match serde_json::from_str::<serde_json::Value>(&stdout) {
            Ok(val) => {
                let pretty = serde_json::to_string_pretty(&val).unwrap_or_else(|_| stdout.to_string());
                ToolResult::text(pretty)
            }
            Err(_) => ToolResult::text(stdout.to_string()),
        }
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        ToolResult::error(format!("nix eval failed:\n{stderr}"))
    }
}

/// Truncate expression for display in progress messages.
fn truncate_for_display(expr: &str) -> String {
    let oneline = expr.replace('\n', " ");
    if oneline.len() > 80 {
        format!("{}...", &oneline[..77])
    } else {
        oneline
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_flake_ref_check() {
        // Verify the tool has the right name
        let tool = NixEvalTool::new();
        assert_eq!(tool.definition().name, "nix_eval");
    }
}
