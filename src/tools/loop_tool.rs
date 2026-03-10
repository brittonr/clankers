//! Loop tool — create and run iterative workflows.
//!
//! Wraps `clankers_loop::LoopEngine` as an agent tool. The LLM can run
//! commands or prompts repeatedly with break conditions.
//!
//! Three modes:
//! - **run** — Execute a command N times or until a condition matches.
//! - **status** — Check the state of a running loop.
//! - **stop** — Stop a running loop.

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use clankers_loop::BreakCondition;
use clankers_loop::LoopDef;
use clankers_loop::LoopEngine;
use clankers_loop::LoopId;
use serde_json::Value;
use serde_json::json;
use tokio::process::Command;
use tracing::info;

use super::Tool;
use super::ToolContext;
use super::ToolDefinition;
use super::ToolResult;

pub struct LoopTool {
    definition: ToolDefinition,
    engine: Arc<LoopEngine>,
}

impl LoopTool {
    pub fn new(engine: Arc<LoopEngine>) -> Self {
        Self {
            definition: ToolDefinition {
                name: "loop".to_string(),
                description: concat!(
                    "Run a command or prompt iteratively with break conditions.\n\n",
                    "Actions:\n",
                    "  run    — Execute a command repeatedly\n",
                    "  status — Check state of a running/completed loop\n",
                    "  stop   — Stop a running loop\n",
                    "  list   — List all active loops\n\n",
                    "Break conditions:\n",
                    "  contains:<text>  — Stop when output contains text\n",
                    "  exit:0           — Stop when exit code is 0\n",
                    "  not_contains:<text> — Stop when text is absent\n\n",
                    "Examples:\n",
                    "  Run 'cargo test' until it passes:\n",
                    "    {action: 'run', command: 'cargo test', break_on: 'exit:0', max: 10}\n",
                    "  Poll a URL every 30s until it returns 'ready':\n",
                    "    {action: 'run', command: 'curl -s http://...', break_on: 'contains:ready', ",
                    "     interval: 30, max: 60}",
                ).to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "action": {
                            "type": "string",
                            "enum": ["run", "status", "stop", "list"],
                            "description": "Action to perform"
                        },
                        "name": {
                            "type": "string",
                            "description": "Loop name (for run)"
                        },
                        "command": {
                            "type": "string",
                            "description": "Shell command to execute each iteration"
                        },
                        "break_on": {
                            "type": "string",
                            "description": "Break condition: 'contains:TEXT', 'exit:CODE', 'not_contains:TEXT'"
                        },
                        "max": {
                            "type": "integer",
                            "description": "Max iterations (default: 10)"
                        },
                        "interval": {
                            "type": "integer",
                            "description": "Seconds between iterations for poll mode (default: 0 = immediate)"
                        },
                        "id": {
                            "type": "string",
                            "description": "Loop ID (for status/stop)"
                        }
                    },
                    "required": ["action"]
                }),
            },
            engine,
        }
    }

    async fn handle_run(&self, ctx: &ToolContext, params: &Value) -> ToolResult {
        let command = match params.get("command").and_then(|v| v.as_str()) {
            Some(c) => c.to_string(),
            None => return ToolResult::error("'run' requires 'command' parameter"),
        };
        let name = params
            .get("name")
            .and_then(|v| v.as_str())
            .unwrap_or("loop")
            .to_string();
        let max = params
            .get("max")
            .and_then(|v| v.as_u64())
            .unwrap_or(10) as u32;
        let interval_secs = params
            .get("interval")
            .and_then(|v| v.as_u64())
            .unwrap_or(0);

        let break_condition = match params.get("break_on").and_then(|v| v.as_str()) {
            Some(s) => parse_break_condition(s),
            None => BreakCondition::Never,
        };

        let action_payload = json!({"command": command});

        let def = if interval_secs > 0 {
            LoopDef::poll(&name, interval_secs, break_condition, None, action_payload)
                .with_max_iterations(max)
        } else if matches!(break_condition, BreakCondition::Never) {
            LoopDef::fixed(&name, max, action_payload)
        } else {
            LoopDef::until(&name, break_condition, action_payload)
                .with_max_iterations(max)
        };

        let loop_id = self.engine.register(def);
        self.engine.start(&loop_id);

        info!("loop started: {} ({}) — max {} iterations", name, loop_id, max);
        ctx.emit_progress(&format!("Starting loop '{name}' (max {max} iterations)"));

        // Run the loop inline (blocking the tool call until done or cancelled).
        let mut iteration = 0u32;
        let mut last_output = String::new();

        loop {
            if ctx.signal.is_cancelled() {
                self.engine.stop(&loop_id);
                return ToolResult::text(format!(
                    "Loop '{name}' cancelled after {iteration} iteration(s).\nLast output:\n{last_output}"
                ));
            }

            // Wait interval if polling
            if interval_secs > 0 && iteration > 0 {
                tokio::select! {
                    () = tokio::time::sleep(Duration::from_secs(interval_secs)) => {}
                    () = ctx.signal.cancelled() => {
                        self.engine.stop(&loop_id);
                        return ToolResult::text(format!(
                            "Loop '{name}' cancelled during wait after {iteration} iteration(s)."
                        ));
                    }
                }
            }

            ctx.emit_progress(&format!("[{name}] iteration {iteration}"));

            // Execute the command
            let output = match run_shell_command(&command).await {
                Ok(o) => o,
                Err(e) => {
                    self.engine.fail(&loop_id);
                    return ToolResult::error(format!(
                        "Loop '{name}' failed on iteration {iteration}: {e}"
                    ));
                }
            };

            last_output = output.stdout.clone();
            let exit_code = output.exit_code;

            // Report to engine
            let should_continue =
                self.engine
                    .record_iteration(&loop_id, output.stdout.clone(), exit_code);

            ctx.emit_progress(&format!(
                "[{name}] iteration {iteration}: exit={} ({} bytes output)",
                exit_code.unwrap_or(-1),
                output.stdout.len(),
            ));

            iteration += 1;

            if !should_continue {
                break;
            }
        }

        // Build final result
        let state = self.engine.get(&loop_id);
        let status = state
            .as_ref()
            .map(|s| format!("{:?}", s.status))
            .unwrap_or_else(|| "unknown".into());
        let elapsed = state.as_ref().map(|s| s.elapsed_secs()).unwrap_or(0);

        ToolResult::text(format!(
            "Loop '{name}' {status} after {iteration} iteration(s) ({elapsed}s).\nLast output:\n{last_output}"
        ))
    }

    fn handle_status(&self, params: &Value) -> ToolResult {
        let id_str = match params.get("id").and_then(|v| v.as_str()) {
            Some(s) => s,
            None => return ToolResult::error("'status' requires 'id' parameter"),
        };
        let id = LoopId(id_str.into());

        match self.engine.get(&id) {
            Some(state) => {
                let summary = state.summary();
                let last_output = state
                    .results
                    .last()
                    .map(|r| r.output.as_str())
                    .unwrap_or("(no output yet)");
                ToolResult::text(format!("{summary}\nLast output:\n{last_output}"))
            }
            None => ToolResult::error(format!("Loop {id} not found.")),
        }
    }

    fn handle_stop(&self, params: &Value) -> ToolResult {
        let id_str = match params.get("id").and_then(|v| v.as_str()) {
            Some(s) => s,
            None => return ToolResult::error("'stop' requires 'id' parameter"),
        };
        let id = LoopId(id_str.into());

        if self.engine.stop(&id) {
            ToolResult::text(format!("Loop {id} stopped."))
        } else {
            ToolResult::error(format!("Loop {id} not found or not running."))
        }
    }

    fn handle_list(&self) -> ToolResult {
        let loops = self.engine.all();
        if loops.is_empty() {
            return ToolResult::text("No loops.");
        }

        let mut lines = Vec::new();
        for s in &loops {
            lines.push(format!(
                "  {} | {} | {:?} | {}/{} iterations",
                s.def.id, s.def.name, s.status, s.current_iteration, s.def.max_iterations,
            ));
        }
        ToolResult::text(format!("{} loop(s):\n{}", loops.len(), lines.join("\n")))
    }
}

#[async_trait]
impl Tool for LoopTool {
    fn definition(&self) -> &ToolDefinition {
        &self.definition
    }

    async fn execute(&self, ctx: &ToolContext, params: Value) -> ToolResult {
        let action = match params.get("action").and_then(|v| v.as_str()) {
            Some(a) => a.to_string(),
            None => return ToolResult::error("Missing 'action' parameter"),
        };

        match action.as_str() {
            "run" => self.handle_run(ctx, &params).await,
            "status" => self.handle_status(&params),
            "stop" => self.handle_stop(&params),
            "list" => self.handle_list(),
            other => ToolResult::error(format!("Unknown action: {other}")),
        }
    }
}

/// Parse a break condition from a string like "contains:PASS", "exit:0", etc.
fn parse_break_condition(s: &str) -> BreakCondition {
    if let Some(text) = s.strip_prefix("contains:") {
        BreakCondition::Contains(text.to_string())
    } else if let Some(text) = s.strip_prefix("not_contains:") {
        BreakCondition::NotContains(text.to_string())
    } else if let Some(text) = s.strip_prefix("equals:") {
        BreakCondition::Equals(text.to_string())
    } else if let Some(code) = s.strip_prefix("exit:") {
        if let Ok(c) = code.parse::<i32>() {
            BreakCondition::ExitCode(c)
        } else {
            // Fall back to substring match
            BreakCondition::Contains(s.to_string())
        }
    } else {
        // Treat the whole string as a substring to match
        BreakCondition::Contains(s.to_string())
    }
}

/// Output from a shell command execution.
struct CommandOutput {
    stdout: String,
    exit_code: Option<i32>,
}

/// Run a shell command and capture output.
async fn run_shell_command(command: &str) -> Result<CommandOutput, String> {
    let output = Command::new("sh")
        .arg("-c")
        .arg(command)
        .output()
        .await
        .map_err(|e| format!("failed to spawn command: {e}"))?;

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();

    // Combine stdout + stderr (stderr appended if non-empty)
    let combined = if stderr.is_empty() {
        stdout
    } else {
        format!("{stdout}\n--- stderr ---\n{stderr}")
    };

    Ok(CommandOutput {
        stdout: combined,
        exit_code: output.status.code(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_break_contains() {
        let cond = parse_break_condition("contains:PASS");
        assert!(cond.check("all tests PASS", None));
        assert!(!cond.check("all tests FAIL", None));
    }

    #[test]
    fn parse_break_exit_code() {
        let cond = parse_break_condition("exit:0");
        assert!(cond.check("", Some(0)));
        assert!(!cond.check("", Some(1)));
    }

    #[test]
    fn parse_break_not_contains() {
        let cond = parse_break_condition("not_contains:error");
        assert!(cond.check("all good", None));
        assert!(!cond.check("found error", None));
    }

    #[test]
    fn parse_break_bare_string() {
        let cond = parse_break_condition("SUCCESS");
        assert!(cond.check("BUILD SUCCESS", None));
    }

    #[tokio::test]
    async fn run_loop_fixed_count() {
        let engine = Arc::new(LoopEngine::new());
        let tool = LoopTool::new(engine);
        let ctx = ToolContext::new("test".into(), Default::default(), None);

        let result = tool
            .execute(
                &ctx,
                json!({
                    "action": "run",
                    "name": "echo-test",
                    "command": "echo hello",
                    "max": 3
                }),
            )
            .await;

        assert!(!result.is_error);
        let text = match &result.content[0] {
            super::super::ToolResultContent::Text { text } => text,
            _ => panic!("expected text"),
        };
        assert!(text.contains("3 iteration(s)"));
        assert!(text.contains("hello"));
    }

    #[tokio::test]
    async fn run_loop_until_condition() {
        let engine = Arc::new(LoopEngine::new());
        let tool = LoopTool::new(engine);
        let ctx = ToolContext::new("test".into(), Default::default(), None);

        // This command always exits 0, so break_on exit:0 should stop after 1 iteration
        let result = tool
            .execute(
                &ctx,
                json!({
                    "action": "run",
                    "name": "exit-test",
                    "command": "echo done",
                    "break_on": "exit:0",
                    "max": 10
                }),
            )
            .await;

        assert!(!result.is_error);
        let text = match &result.content[0] {
            super::super::ToolResultContent::Text { text } => text,
            _ => panic!("expected text"),
        };
        assert!(text.contains("1 iteration(s)"));
    }
}
