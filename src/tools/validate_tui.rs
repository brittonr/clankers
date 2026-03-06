//! TUI validation tool — spawns clankers in a PTY, sends keystrokes,
//! and verifies screen content against assertions.
//!
//! This enables automated TUI testing as part of the self-validate workflow.
//! The agent describes a sequence of interactions and expected screen states,
//! and this tool executes them and reports pass/fail.

use std::io::Read;
use std::io::Write;
use std::sync::Arc;
use std::sync::Mutex;
use std::time::Duration;
use std::time::Instant;

use async_trait::async_trait;
use portable_pty::CommandBuilder;
use portable_pty::NativePtySystem;
use portable_pty::PtySize;
use portable_pty::PtySystem;
use serde::Deserialize;
use serde_json::Value;
use tokio_util::sync::CancellationToken;
use vt100::Parser;

use super::Tool;
use super::ToolContext;
use super::ToolDefinition;
use super::ToolResult;

// ── Step types ──────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
struct TuiStep {
    /// Action to perform
    action: StepAction,
    /// Text to wait for after the action (optional)
    #[serde(default)]
    wait_for: Option<String>,
    /// Text that must NOT be on screen after the action (optional)
    #[serde(default)]
    assert_absent: Option<String>,
    /// Text that must be on screen after the action (optional)
    #[serde(default)]
    assert_visible: Option<String>,
    /// Whether to capture the screen after this step (default: false)
    #[serde(default)]
    capture: bool,
    /// Timeout in ms for wait_for (default: 3000)
    #[serde(default = "default_timeout")]
    timeout_ms: u64,
}

fn default_timeout() -> u64 {
    3000
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum StepAction {
    /// Type a string
    Type { text: String },
    /// Send a named key
    Key { name: String },
    /// Wait for a duration (ms)
    Wait { ms: u64 },
    /// Enter insert mode, type a slash command, and submit
    SlashCommand { command: String },
}

// ── Key name mapping ────────────────────────────────────────────────

fn key_bytes(name: &str) -> Option<&'static [u8]> {
    Some(match name.to_lowercase().as_str() {
        "enter" | "return" | "cr" => b"\r",
        "esc" | "escape" => b"\x1b",
        "tab" => b"\t",
        "backspace" | "bs" => b"\x7f",
        "delete" | "del" => b"\x1b[3~",
        "up" => b"\x1b[A",
        "down" => b"\x1b[B",
        "right" => b"\x1b[C",
        "left" => b"\x1b[D",
        "home" => b"\x1b[H",
        "end" => b"\x1b[F",
        "pageup" | "pgup" => b"\x1b[5~",
        "pagedown" | "pgdn" => b"\x1b[6~",
        "ctrl+c" => b"\x03",
        "ctrl+d" => b"\x04",
        "ctrl+j" => b"\x0a",
        "ctrl+k" => b"\x0b",
        "ctrl+n" => b"\x0e",
        "ctrl+p" => b"\x10",
        "ctrl+t" => b"\x14",
        "ctrl+u" => b"\x15",
        "ctrl+w" => b"\x17",
        "ctrl+x" => b"\x18",
        "alt+enter" => b"\x1b\r",
        "space" => b" ",
        "backtick" | "`" => b"`",
        _ => return None,
    })
}

// ── PTY harness (reusable from test code) ───────────────────────────

struct PtyHarness {
    parser: Arc<Mutex<Parser>>,
    writer: Box<dyn Write + Send>,
    _child: Box<dyn portable_pty::Child + Send + Sync>,
    _reader_thread: std::thread::JoinHandle<()>,
    rows: u16,
    cols: u16,
}

impl PtyHarness {
    fn spawn(rows: u16, cols: u16, extra_args: &[String]) -> Result<Self, String> {
        let pty_system = NativePtySystem::default();
        let pair = pty_system
            .openpty(PtySize {
                rows,
                cols,
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(|e| format!("Failed to open PTY: {}", e))?;

        // Find clankers binary: prefer CARGO_BIN_EXE_clankers (set by cargo test),
        // then look next to our executable, then fall back to PATH
        let clankers_bin = if let Ok(bin) = std::env::var("CARGO_BIN_EXE_clankers") {
            std::path::PathBuf::from(bin)
        } else {
            let exe = std::env::current_exe().map_err(|e| format!("Can't find binary: {}", e))?;
            let dir = exe.parent().unwrap_or_else(|| std::path::Path::new("."));
            // Check sibling (e.g. target/debug/clankers next to target/debug/deps/clankers-xxx)
            let candidates = [
                dir.join("clankers"),
                dir.parent().map(|p| p.join("clankers")).unwrap_or_default(),
            ];
            candidates.iter().find(|p| p.exists() && p.is_file()).cloned().unwrap_or_else(|| {
                // Fall back to "clankers" on PATH
                std::path::PathBuf::from("clankers")
            })
        };
        let mut cmd = CommandBuilder::new(&clankers_bin);
        cmd.args(["--no-zellij"]);
        for arg in extra_args {
            cmd.arg(arg);
        }
        cmd.env("RUST_LOG", "off");
        cmd.env("TERM", "xterm-256color");

        let child = pair.slave.spawn_command(cmd).map_err(|e| format!("Failed to spawn clankers: {}", e))?;
        drop(pair.slave);

        let mut reader = pair.master.try_clone_reader().map_err(|e| format!("Failed to clone reader: {}", e))?;
        let writer = pair.master.take_writer().map_err(|e| format!("Failed to take writer: {}", e))?;

        let parser = Arc::new(Mutex::new(Parser::new(rows, cols, 0)));
        let parser_clone = Arc::clone(&parser);
        let reader_thread = std::thread::spawn(move || {
            let mut buf = [0u8; 4096];
            loop {
                match reader.read(&mut buf) {
                    Ok(0) => break,
                    Ok(n) => {
                        parser_clone.lock().unwrap_or_else(|e| e.into_inner()).process(&buf[..n]);
                    }
                    Err(_) => break,
                }
            }
        });

        let harness = Self {
            parser,
            writer,
            _child: child,
            _reader_thread: reader_thread,
            rows,
            cols,
        };

        // Wait for initial render
        harness
            .wait_for("NORMAL", Duration::from_secs(10))
            .map_err(|e| format!("TUI failed to start: {}", e))?;

        Ok(harness)
    }

    fn send(&mut self, data: &[u8]) -> Result<(), String> {
        self.writer.write_all(data).map_err(|e| format!("Write failed: {}", e))?;
        self.writer.flush().map_err(|e| format!("Flush failed: {}", e))?;
        Ok(())
    }

    fn type_str(&mut self, s: &str) -> Result<(), String> {
        self.send(s.as_bytes())
    }

    fn screen_text(&self) -> String {
        let parser = self.parser.lock().unwrap_or_else(|e| e.into_inner());
        let screen = parser.screen();
        let mut lines = Vec::new();
        for row in 0..self.rows {
            let mut line = String::new();
            for col in 0..self.cols {
                let Some(cell) = screen.cell(row, col) else { continue };
                line.push_str(cell.contents());
            }
            lines.push(line.trim_end().to_string());
        }
        lines.join("\n")
    }

    fn screen_contains(&self, needle: &str) -> bool {
        self.screen_text().contains(needle)
    }

    fn wait_for(&self, needle: &str, timeout: Duration) -> Result<(), String> {
        let start = Instant::now();
        loop {
            if self.screen_contains(needle) {
                return Ok(());
            }
            if start.elapsed() >= timeout {
                return Err(format!(
                    "Timed out after {:?} waiting for {:?}.\nScreen:\n{}",
                    timeout,
                    needle,
                    self.screen_text()
                ));
            }
            std::thread::sleep(Duration::from_millis(50));
        }
    }

    fn wait_for_absent(&self, needle: &str, timeout: Duration) -> Result<(), String> {
        let start = Instant::now();
        loop {
            if !self.screen_contains(needle) {
                return Ok(());
            }
            if start.elapsed() >= timeout {
                return Err(format!(
                    "Timed out after {:?} waiting for {:?} to disappear.\nScreen:\n{}",
                    timeout,
                    needle,
                    self.screen_text()
                ));
            }
            std::thread::sleep(Duration::from_millis(50));
        }
    }

    fn quit(&mut self) {
        let _ = self.send(b"\x1b"); // Esc
        std::thread::sleep(Duration::from_millis(100));
        let _ = self.type_str("q");
        std::thread::sleep(Duration::from_millis(300));
    }
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
        let rows = params.get("rows").and_then(|v| v.as_u64()).unwrap_or(24) as u16;
        let cols = params.get("cols").and_then(|v| v.as_u64()).unwrap_or(120) as u16;

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

        // Run in a blocking thread since PTY operations are synchronous
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

// ── Test runner ─────────────────────────────────────────────────────

fn run_tui_test(
    description: &str,
    rows: u16,
    cols: u16,
    steps: &[TuiStep],
    signal: &CancellationToken,
) -> Result<String, String> {
    let mut report = String::new();
    report.push_str(&format!("## TUI Validation: {}\n\n", description));
    report.push_str(&format!("PTY size: {}x{}\n", cols, rows));
    report.push_str(&format!("Steps: {}\n\n", steps.len()));

    let mut harness = PtyHarness::spawn(rows, cols, &[])?;
    let mut passed = 0usize;
    let mut failed = 0usize;
    let mut captures: Vec<(usize, String)> = Vec::new();

    for (i, step) in steps.iter().enumerate() {
        if signal.is_cancelled() {
            report.push_str(&format!("\n⚠ Cancelled at step {}\n", i + 1));
            break;
        }

        let step_label = format!("Step {}", i + 1);

        // Execute the action
        let action_result = match &step.action {
            StepAction::Type { text } => {
                report.push_str(&format!("  {} type: {:?}\n", step_label, text));
                harness.type_str(text)
            }
            StepAction::Key { name } => {
                report.push_str(&format!("  {} key: {}\n", step_label, name));
                match key_bytes(name) {
                    Some(bytes) => harness.send(bytes),
                    None => Err(format!("Unknown key name: {:?}", name)),
                }
            }
            StepAction::Wait { ms } => {
                report.push_str(&format!("  {} wait: {}ms\n", step_label, ms));
                std::thread::sleep(Duration::from_millis(*ms));
                Ok(())
            }
            StepAction::SlashCommand { command } => {
                report.push_str(&format!("  {} slash: {}\n", step_label, command));
                // Enter insert mode, type command, submit
                harness.type_str(&format!("i{}", command))?;
                std::thread::sleep(Duration::from_millis(200));
                harness.send(b"\r")
            }
        };

        if let Err(e) = action_result {
            report.push_str(&format!("    ✗ FAIL — action error: {}\n", e));
            failed += 1;
            continue;
        }

        // Small settle time after each action
        std::thread::sleep(Duration::from_millis(150));

        // Wait for expected text
        if let Some(ref wait_text) = step.wait_for {
            let timeout = Duration::from_millis(step.timeout_ms);
            match harness.wait_for(wait_text, timeout) {
                Ok(()) => {
                    report.push_str(&format!("    ✓ wait_for {:?} — found\n", wait_text));
                }
                Err(e) => {
                    report.push_str(&format!("    ✗ FAIL — {}\n", e));
                    failed += 1;
                    if step.capture {
                        captures.push((i + 1, harness.screen_text()));
                    }
                    continue;
                }
            }
        }

        // Assert visible
        if let Some(ref visible) = step.assert_visible {
            if harness.screen_contains(visible) {
                report.push_str(&format!("    ✓ assert_visible {:?} — found\n", visible));
            } else {
                report.push_str(&format!("    ✗ FAIL — assert_visible {:?} not found on screen\n", visible));
                failed += 1;
                if step.capture {
                    captures.push((i + 1, harness.screen_text()));
                }
                continue;
            }
        }

        // Assert absent
        if let Some(ref absent) = step.assert_absent {
            let timeout = Duration::from_millis(step.timeout_ms.min(1000));
            match harness.wait_for_absent(absent, timeout) {
                Ok(()) => {
                    report.push_str(&format!("    ✓ assert_absent {:?} — confirmed absent\n", absent));
                }
                Err(_) => {
                    report.push_str(&format!("    ✗ FAIL — assert_absent {:?} is still on screen\n", absent));
                    failed += 1;
                    if step.capture {
                        captures.push((i + 1, harness.screen_text()));
                    }
                    continue;
                }
            }
        }

        // Capture screen
        if step.capture {
            captures.push((i + 1, harness.screen_text()));
        }

        passed += 1;
    }

    // Clean up
    harness.quit();

    // Summary
    report.push_str(&format!("\n## Results: {} passed, {} failed out of {} steps\n", passed, failed, steps.len()));

    if failed == 0 {
        report.push_str("## Status: ✓ PASS\n");
    } else {
        report.push_str("## Status: ✗ FAIL\n");
    }

    // Append captures
    if !captures.is_empty() {
        report.push_str("\n## Screen Captures\n");
        for (step_num, screen) in &captures {
            report.push_str(&format!("\n### Step {} screen:\n```\n{}\n```\n", step_num, screen));
        }
    }

    if failed > 0 { Err(report) } else { Ok(report) }
}

// ── Tests ───────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tools::ToolResultContent;

    #[tokio::test]
    async fn validate_tui_basic_smoke() {
        let tool = ValidateTuiTool::new();
        let params = serde_json::json!({
            "description": "Basic smoke test",
            "rows": 24,
            "cols": 100,
            "steps": [
                {
                    "action": { "type": "wait", "ms": 300 },
                    "assert_visible": "NORMAL",
                    "capture": true
                }
            ]
        });
        let result =
            tool.execute(&ToolContext::new("test-1".to_string(), CancellationToken::new(), None), params).await;
        assert!(!result.is_error, "Should pass: {:?}", result);
        let text = match &result.content[0] {
            ToolResultContent::Text { text } => text,
            _ => panic!("Expected text"),
        };
        assert!(text.contains("PASS"), "Should contain PASS: {}", text);
    }

    #[tokio::test]
    async fn validate_tui_slash_command_and_panel() {
        let tool = ValidateTuiTool::new();
        let params = serde_json::json!({
            "description": "Todo panel appears after /todo add",
            "rows": 24,
            "cols": 120,
            "steps": [
                {
                    "action": { "type": "slash_command", "command": "/todo add Test item" },
                    "wait_for": "Added todo #1",
                    "timeout_ms": 3000
                },
                {
                    "action": { "type": "wait", "ms": 300 },
                    "assert_visible": "Todo (",
                    "capture": true
                }
            ]
        });
        let result =
            tool.execute(&ToolContext::new("test-2".to_string(), CancellationToken::new(), None), params).await;
        assert!(!result.is_error, "Should pass: {:?}", result);
        let text = match &result.content[0] {
            ToolResultContent::Text { text } => text,
            _ => panic!("Expected text"),
        };
        assert!(text.contains("PASS"), "Should contain PASS: {}", text);
    }

    #[tokio::test]
    async fn validate_tui_panel_focus_with_backtick() {
        let tool = ValidateTuiTool::new();
        let params = serde_json::json!({
            "description": "Panel focus via backtick and spatial h/l navigation",
            "rows": 24,
            "cols": 200,
            "steps": [
                {
                    "action": { "type": "slash_command", "command": "/todo add Task one" },
                    "wait_for": "Added todo #1"
                },
                {
                    "action": { "type": "key", "name": "esc" },
                    "wait_for": "NORMAL"
                },
                {
                    "action": { "type": "key", "name": "backtick" },
                    "assert_visible": "j/k Tab",
                    "capture": true
                },
                {
                    // h from right panel → focus chat (spatial: chat is to the left)
                    "action": { "type": "type", "text": "h" },
                    "assert_absent": "j/k Tab",
                    "assert_visible": "Main"
                },
                {
                    // l from main → focus right panel again (spatial: right panels are to the right)
                    "action": { "type": "type", "text": "l" },
                    "assert_visible": "j/k Tab",
                    "capture": true
                },
                {
                    "action": { "type": "key", "name": "esc" },
                    "assert_absent": "j/k Tab"
                }
            ]
        });
        let result =
            tool.execute(&ToolContext::new("test-3".to_string(), CancellationToken::new(), None), params).await;
        assert!(!result.is_error, "Should pass: {:?}", result);
        let text = match &result.content[0] {
            ToolResultContent::Text { text } => text,
            _ => panic!("Expected text"),
        };
        assert!(text.contains("PASS"), "Should contain PASS:\n{}", text);
    }

    #[tokio::test]
    async fn validate_tui_failing_assertion() {
        let tool = ValidateTuiTool::new();
        let params = serde_json::json!({
            "description": "Should fail — looking for text that doesn't exist",
            "rows": 24,
            "cols": 100,
            "steps": [
                {
                    "action": { "type": "wait", "ms": 200 },
                    "assert_visible": "THIS TEXT DOES NOT EXIST",
                    "capture": true
                }
            ]
        });
        let result =
            tool.execute(&ToolContext::new("test-4".to_string(), CancellationToken::new(), None), params).await;
        assert!(result.is_error, "Should fail");
        let text = match &result.content[0] {
            ToolResultContent::Text { text } => text,
            _ => panic!("Expected text"),
        };
        assert!(text.contains("FAIL"), "Should contain FAIL: {}", text);
    }
}
