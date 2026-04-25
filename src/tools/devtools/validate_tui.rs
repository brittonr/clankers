//! TUI validation tool — spawns clankers in a PTY, sends keystrokes,
//! and verifies screen content against assertions.
//!
//! This enables automated TUI testing as part of the self-validate workflow.
//! The agent describes a sequence of interactions and expected screen states,
//! and this tool executes them and reports pass/fail.

#![allow(unexpected_cfgs)]
#![cfg_attr(dylint_lib = "tigerstyle", allow(compound_assertion, ignored_result, no_unwrap, no_panic, no_todo, unjustified_no_todo_allow, no_recursion, unchecked_narrowing, unchecked_division, unbounded_loop, catch_all_on_enum, explicit_defaults, unbounded_channel, unbounded_collection_growth, assertion_density, raw_arithmetic_overflow, sentinel_fallback, acronym_style, bool_naming, negated_predicate, numeric_units, float_for_currency, function_length, nested_conditionals, platform_dependent_cast, usize_in_public_api, too_many_parameters, compound_condition, unjustified_allow, ambiguous_params, ambient_clock, verified_purity, contradictory_time, multi_lock_ordering, reason = "full workspace tigerstyle audit gate: legacy debt documented locally while cleanup proceeds incrementally"))]

use std::time::Duration;

use async_trait::async_trait;
use serde::Deserialize;
use serde_json::Value;
use tokio_util::sync::CancellationToken;

use super::pty_harness::PtyHarness;
use super::pty_harness::key_bytes;
use crate::tools::Tool;
use crate::tools::ToolContext;
use crate::tools::ToolDefinition;
use crate::tools::ToolResult;

// ── Step types ──────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
struct TuiStep {
    action: StepAction,
    #[serde(default)]
    wait_for: Option<String>,
    #[serde(default)]
    assert_absent: Option<String>,
    #[serde(default)]
    assert_visible: Option<String>,
    #[serde(default)]
    capture: bool,
    #[serde(default = "default_timeout")]
    timeout_ms: u64,
}

fn default_timeout() -> u64 {
    3000
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum StepAction {
    Type { text: String },
    Key { name: String },
    Wait { ms: u64 },
    SlashCommand { command: String },
}

// ── Tool implementation ─────────────────────────────────────────────

pub struct ValidateTuiTool {
    definition: ToolDefinition,
}

impl ValidateTuiTool {
    pub fn new() -> Self {
        Self {
            definition: ToolDefinition {
                name: "validate_tui".to_string(),
                description: "Spawn clankers in a PTY, execute a sequence of TUI interactions, and verify \
                    screen content. Use this to validate that TUI features like panels, keybindings, \
                    rendering, and navigation work correctly.\n\n\
                    Each step can type text, send keys, run slash commands, and assert screen content."
                    .to_string(),
                input_schema: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "steps": {
                            "type": "array",
                            "description": "Sequence of interaction steps to execute",
                            "items": {
                                "type": "object",
                                "properties": {
                                    "action": {
                                        "type": "object",
                                        "description": "Action to perform",
                                        "properties": {
                                            "type": {
                                                "type": "string",
                                                "enum": ["type", "key", "wait", "slash_command"],
                                                "description": "Action type"
                                            },
                                            "text": {
                                                "type": "string",
                                                "description": "For 'type': text to type"
                                            },
                                            "name": {
                                                "type": "string",
                                                "description": "For 'key': key name (enter, esc, up, down, left, right, tab, backspace, ctrl+c, backtick, space, etc)"
                                            },
                                            "ms": {
                                                "type": "number",
                                                "description": "For 'wait': milliseconds to wait"
                                            },
                                            "command": {
                                                "type": "string",
                                                "description": "For 'slash_command': the command including / prefix, e.g. '/todo add Task'"
                                            }
                                        },
                                        "required": ["type"]
                                    },
                                    "wait_for": {
                                        "type": "string",
                                        "description": "Text to wait for on screen after the action"
                                    },
                                    "assert_visible": {
                                        "type": "string",
                                        "description": "Text that must be on screen (fails if absent)"
                                    },
                                    "assert_absent": {
                                        "type": "string",
                                        "description": "Text that must NOT be on screen (fails if present)"
                                    },
                                    "capture": {
                                        "type": "boolean",
                                        "description": "Capture the screen content after this step (included in output)"
                                    },
                                    "timeout_ms": {
                                        "type": "number",
                                        "description": "Timeout in ms for wait_for (default: 3000)"
                                    }
                                },
                                "required": ["action"]
                            }
                        },
                        "rows": {
                            "type": "number",
                            "description": "PTY height in rows (default: 24)"
                        },
                        "cols": {
                            "type": "number",
                            "description": "PTY width in columns (default: 120)"
                        },
                        "description": {
                            "type": "string",
                            "description": "Human-readable description of what this test validates"
                        }
                    },
                    "required": ["steps"]
                }),
            },
        }
    }
}

impl Default for ValidateTuiTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for ValidateTuiTool {
    fn definition(&self) -> &ToolDefinition {
        &self.definition
    }

    async fn execute(&self, ctx: &ToolContext, params: Value) -> ToolResult {
        let description = params.get("description").and_then(|v| v.as_str()).unwrap_or("TUI validation").to_string();
        let rows = u16::try_from(params.get("rows").and_then(|v| v.as_u64()).unwrap_or(24)).unwrap_or(24);
        let cols = u16::try_from(params.get("cols").and_then(|v| v.as_u64()).unwrap_or(120)).unwrap_or(120);

        let steps: Vec<TuiStep> = match params.get("steps").and_then(|v| v.as_array()) {
            Some(arr) => {
                let mut parsed = Vec::new();
                for (i, step_val) in arr.iter().enumerate() {
                    match serde_json::from_value::<TuiStep>(step_val.clone()) {
                        Ok(step) => parsed.push(step),
                        Err(e) => return ToolResult::error(format!("Invalid step {}: {}", i, e)),
                    }
                }
                parsed
            }
            None => return ToolResult::error("Missing required 'steps' array"),
        };

        if steps.is_empty() {
            return ToolResult::error("Steps array is empty");
        }

        let signal = ctx.signal.clone();
        let result = tokio::task::spawn_blocking(move || run_tui_test(&description, rows, cols, &steps, &signal)).await;

        match result {
            Ok(Ok(report)) => ToolResult::text(report),
            Ok(Err(report)) => {
                let mut r = ToolResult::text(report);
                r.is_error = true;
                r
            }
            Err(e) => ToolResult::error(format!("TUI test panicked: {}", e)),
        }
    }
}

// ── Step execution helpers ──────────────────────────────────────────

fn execute_action(harness: &mut PtyHarness, action: &StepAction) -> Result<(), String> {
    match action {
        StepAction::Type { text } => harness.type_str(text),
        StepAction::Key { name } => match key_bytes(name) {
            Some(bytes) => harness.send(bytes),
            None => Err(format!("Unknown key name: {:?}", name)),
        },
        StepAction::Wait { ms } => {
            std::thread::sleep(Duration::from_millis(*ms));
            Ok(())
        }
        StepAction::SlashCommand { command } => {
            harness.type_str(&format!("i{}", command))?;
            std::thread::sleep(Duration::from_millis(200));
            harness.send(b"\r")
        }
    }
}

/// Check wait_for, assert_visible, assert_absent. Returns true if all passed.
fn check_assertions(
    harness: &PtyHarness,
    step: &TuiStep,
    report: &mut String,
    captures: &mut Vec<(usize, String)>,
    step_idx: usize,
) -> bool {
    use std::fmt::Write;

    if let Some(ref wait_text) = step.wait_for {
        let timeout = Duration::from_millis(step.timeout_ms);
        match harness.wait_for(wait_text, timeout) {
            Ok(()) => {
                writeln!(report, "    \u{2713} wait_for {:?} \u{2014} found", wait_text).ok();
            }
            Err(e) => {
                writeln!(report, "    \u{2717} FAIL \u{2014} {}", e).ok();
                if step.capture {
                    captures.push((step_idx, harness.screen_text()));
                }
                return false;
            }
        }
    }

    if let Some(ref visible) = step.assert_visible {
        if harness.screen_contains(visible) {
            writeln!(report, "    \u{2713} assert_visible {:?} \u{2014} found", visible).ok();
        } else {
            writeln!(report, "    \u{2717} FAIL \u{2014} assert_visible {:?} not found on screen", visible).ok();
            if step.capture {
                captures.push((step_idx, harness.screen_text()));
            }
            return false;
        }
    }

    if let Some(ref absent) = step.assert_absent {
        let timeout = Duration::from_millis(step.timeout_ms.min(1000));
        match harness.wait_for_absent(absent, timeout) {
            Ok(()) => {
                writeln!(report, "    \u{2713} assert_absent {:?} \u{2014} confirmed absent", absent).ok();
            }
            Err(_) => {
                writeln!(report, "    \u{2717} FAIL \u{2014} assert_absent {:?} is still on screen", absent).ok();
                if step.capture {
                    captures.push((step_idx, harness.screen_text()));
                }
                return false;
            }
        }
    }

    true
}

// ── Test runner ─────────────────────────────────────────────────────

fn run_tui_test(
    description: &str,
    rows: u16,
    cols: u16,
    steps: &[TuiStep],
    signal: &CancellationToken,
) -> Result<String, String> {
    use std::fmt::Write;

    let mut report = String::new();
    write!(report, "## TUI Validation: {}\n\n", description).ok();
    writeln!(report, "PTY size: {}x{}", cols, rows).ok();
    write!(report, "Steps: {}\n\n", steps.len()).ok();

    let mut harness = PtyHarness::spawn(rows, cols, &[])?;
    let mut passed = 0usize;
    let mut failed = 0usize;
    let mut captures: Vec<(usize, String)> = Vec::new();

    for (i, step) in steps.iter().enumerate() {
        if signal.is_cancelled() {
            write!(report, "\n\u{26a0} Cancelled at step {}\n", i + 1).ok();
            break;
        }

        let step_label = format!("Step {}", i + 1);

        // Log action
        match &step.action {
            StepAction::Type { text } => {
                writeln!(report, "  {} type: {:?}", step_label, text).ok();
            }
            StepAction::Key { name } => {
                writeln!(report, "  {} key: {}", step_label, name).ok();
            }
            StepAction::Wait { ms } => {
                writeln!(report, "  {} wait: {}ms", step_label, ms).ok();
            }
            StepAction::SlashCommand { command } => {
                writeln!(report, "  {} slash: {}", step_label, command).ok();
            }
        }

        // Execute action
        if let Err(e) = execute_action(&mut harness, &step.action) {
            writeln!(report, "    \u{2717} FAIL \u{2014} action error: {}", e).ok();
            failed += 1;
            continue;
        }

        std::thread::sleep(Duration::from_millis(150));

        // Check assertions
        if !check_assertions(&harness, step, &mut report, &mut captures, i + 1) {
            failed += 1;
            continue;
        }

        // Capture screen
        if step.capture {
            captures.push((i + 1, harness.screen_text()));
        }

        passed += 1;
    }

    harness.quit();

    // Summary
    write!(report, "\n## Results: {} passed, {} failed out of {} steps\n", passed, failed, steps.len()).ok();

    if failed == 0 {
        report.push_str("## Status: \u{2713} PASS\n");
    } else {
        report.push_str("## Status: \u{2717} FAIL\n");
    }

    if !captures.is_empty() {
        report.push_str("\n## Screen Captures\n");
        for (step_num, screen) in &captures {
            write!(report, "\n### Step {} screen:\n```\n{}\n```\n", step_num, screen).ok();
        }
    }

    if failed > 0 { Err(report) } else { Ok(report) }
}

// ── Tests ───────────────────────────────────────────────────────────

#[path = "validate_tui_tests.rs"]
#[cfg(test)]
mod tests;
