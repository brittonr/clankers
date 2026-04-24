//! Loop tool — create and run iterative workflows.
//!
//! Wraps `clanker_loop::LoopEngine` as an agent tool. The LLM can run
//! commands or prompts repeatedly with break conditions.
//!
//! Three modes:
//! - **run** — Execute a command N times or until a condition matches.
//! - **status** — Check the state of a running loop.
//! - **stop** — Stop a running loop.

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use clanker_loop::BreakCondition;
use clanker_loop::LoopDef;
use clanker_loop::LoopEngine;
use clanker_loop::LoopId;
use clanker_loop::parse_break_condition;
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
                )
                .to_string(),
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
        let parsed = match parse_run_params(params) {
            Ok(p) => p,
            Err(e) => return e,
        };

        let loop_id = match self.register_loop(&parsed) {
            Ok(id) => id,
            Err(e) => return e,
        };
        self.engine.start(&loop_id);

        info!("loop started: {} ({}) — max {} iterations", parsed.name, loop_id, parsed.max);
        ctx.emit_progress(&format!("Starting loop '{}' (max {} iterations)", parsed.name, parsed.max));

        self.run_loop_iterations(ctx, &loop_id, &parsed).await
    }

    fn register_loop(&self, params: &RunParams) -> Result<LoopId, ToolResult> {
        let action_payload = json!({"command": params.command});

        let def = if params.interval_secs > 0 {
            LoopDef::poll(&params.name, params.interval_secs, params.break_condition.clone(), None, action_payload)
                .with_max_iterations(params.max)
        } else if matches!(params.break_condition, BreakCondition::Never) {
            LoopDef::fixed(&params.name, params.max, action_payload)
        } else {
            LoopDef::until(&params.name, params.break_condition.clone(), action_payload).with_max_iterations(params.max)
        };

        self.engine.register(def).ok_or_else(|| {
            ToolResult::error("too many active loops — wait for existing loops to finish or stop them first")
        })
    }

    #[cfg_attr(
        dylint_lib = "tigerstyle",
        allow(unbounded_loop, reason = "event loop; bounded by max iterations")
    )]
    async fn run_loop_iterations(&self, ctx: &ToolContext, loop_id: &LoopId, params: &RunParams) -> ToolResult {
        let name = &params.name;
        let mut iteration = 0u32;
        let mut last_output = String::new();

        loop {
            if ctx.signal.is_cancelled() {
                self.engine.stop(loop_id);
                return ToolResult::text(format!(
                    "Loop '{name}' cancelled after {iteration} iteration(s).\nLast output:\n{last_output}"
                ));
            }

            if let Err(result) = self.wait_interval(ctx, loop_id, params, iteration).await {
                return result;
            }

            ctx.emit_progress(&format!("[{name}] iteration {iteration}"));

            let output = match run_shell_command(&params.command).await {
                Ok(o) => o,
                Err(e) => {
                    self.engine.fail(loop_id);
                    return ToolResult::error(format!("Loop '{name}' failed on iteration {iteration}: {e}"));
                }
            };

            output.stdout.clone_into(&mut last_output);
            let exit_code = output.exit_code;
            let should_continue = self.engine.record_iteration(loop_id, output.stdout.clone(), exit_code);

            ctx.emit_progress(&format!(
                "[{name}] iteration {iteration}: exit={} ({} bytes output)",
                exit_code.unwrap_or(-1),
                output.stdout.len(),
            ));

            iteration = iteration.saturating_add(1);

            if !should_continue {
                break;
            }
        }

        format_loop_result(&self.engine, loop_id, name, iteration, &last_output)
    }

    /// Wait for the poll interval between iterations. Returns Err(ToolResult) on cancel.
    async fn wait_interval(
        &self,
        ctx: &ToolContext,
        loop_id: &LoopId,
        params: &RunParams,
        iteration: u32,
    ) -> Result<(), ToolResult> {
        if params.interval_secs == 0 || iteration == 0 {
            return Ok(());
        }
        tokio::select! {
            () = tokio::time::sleep(Duration::from_secs(params.interval_secs)) => Ok(()),
            () = ctx.signal.cancelled() => {
                self.engine.stop(loop_id);
                Err(ToolResult::text(format!(
                    "Loop '{}' cancelled during wait after {} iteration(s).",
                    params.name, iteration,
                )))
            }
        }
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
                let last_output = state.results.last().map(|r| r.output.as_str()).unwrap_or("(no output yet)");
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

// ── Run parameter parsing (pure) ────────────────────────────────────────────

/// Parsed parameters for a `run` action.
struct RunParams {
    command: String,
    name: String,
    max: u32,
    interval_secs: u64,
    break_condition: BreakCondition,
}

/// Parse and validate `run` action parameters. Pure function — no side effects.
fn parse_run_params(params: &Value) -> Result<RunParams, ToolResult> {
    let command = params
        .get("command")
        .and_then(|v| v.as_str())
        .ok_or_else(|| ToolResult::error("'run' requires 'command' parameter"))?
        .to_string();
    let name = params.get("name").and_then(|v| v.as_str()).unwrap_or("loop").to_string();
    let max = params.get("max").and_then(|v| v.as_u64()).unwrap_or(10).min(u64::from(u32::MAX)) as u32;
    let interval_secs = params.get("interval").and_then(|v| v.as_u64()).unwrap_or(0);
    let break_condition = match params.get("break_on").and_then(|v| v.as_str()) {
        Some(s) => parse_break_condition(s),
        None => BreakCondition::Never,
    };

    debug_assert!(!command.is_empty(), "command must not be empty");

    Ok(RunParams {
        command,
        name,
        max,
        interval_secs,
        break_condition,
    })
}

/// Build the final ToolResult summary from loop engine state.
fn format_loop_result(
    engine: &LoopEngine,
    loop_id: &LoopId,
    name: &str,
    iteration: u32,
    last_output: &str,
) -> ToolResult {
    let state = engine.get(loop_id);
    let status = state.as_ref().map(|s| format!("{:?}", s.status)).unwrap_or_else(|| "unknown".into());
    let elapsed = state.as_ref().map(|s| s.elapsed_secs()).unwrap_or(0);

    ToolResult::text(format!(
        "Loop '{name}' {status} after {iteration} iteration(s) ({elapsed}s).\nLast output:\n{last_output}"
    ))
}

// ── Shell command execution ─────────────────────────────────────────────────

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
#[cfg_attr(
    dylint_lib = "tigerstyle",
    allow(no_panic, no_unwrap, reason = "test code — panics are assertions")
)]
mod tests {
    use tokio_util::sync::CancellationToken;

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
        let ctx = ToolContext::new("test".into(), CancellationToken::default(), None);

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
        let ctx = ToolContext::new("test".into(), CancellationToken::default(), None);

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
