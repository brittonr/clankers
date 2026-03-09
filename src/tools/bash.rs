//! Shell command execution with live streaming

use async_trait::async_trait;
use regex::Regex;
use serde_json::Value;
use serde_json::json;
use tokio::io::AsyncBufReadExt;
use tokio::io::BufReader;
use tokio::process::Command;
use tokio::time::Duration;

use super::Tool;
use super::ToolContext;
use super::ToolDefinition;
use super::ToolResult;
use super::progress::ResultChunk;
use super::progress::ToolProgress;
use crate::util::ansi::strip_ansi;

/// Confirmation channel for dangerous commands.
/// When set, the bash tool sends a request and waits for approval.
pub type ConfirmTx = tokio::sync::mpsc::UnboundedSender<(String, tokio::sync::oneshot::Sender<bool>)>;
pub type ConfirmRx = tokio::sync::mpsc::UnboundedReceiver<(String, tokio::sync::oneshot::Sender<bool>)>;

/// Create a confirmation channel pair.
pub fn confirm_channel() -> (ConfirmTx, ConfirmRx) {
    tokio::sync::mpsc::unbounded_channel()
}

/// Patterns that trigger a confirmation prompt before execution.
///
/// Note: The .expect() calls on Regex::new() are justified because these are
/// compile-time constant patterns validated during development. A regex
/// compilation failure would be caught immediately during testing and indicates
/// a programmer error, not a runtime condition. LazyLock initialization happens
/// once at static init time, not in hot paths.
static DANGEROUS_PATTERNS: std::sync::LazyLock<Vec<(Regex, &'static str)>> = std::sync::LazyLock::new(|| {
    vec![
        (
            Regex::new(r"\brm\s+(-[a-zA-Z]*f[a-zA-Z]*\s+|.*-rf\b|.*--force\b)")
                .expect("dangerous pattern regex is valid"),
            "forced removal",
        ),
        (Regex::new(r"\bsudo\s+rm\b").expect("dangerous pattern regex is valid"), "sudo removal"),
        (
            Regex::new(r"(?i)\b(DROP|TRUNCATE|DELETE\s+FROM)\b").expect("dangerous pattern regex is valid"),
            "destructive SQL",
        ),
        (
            Regex::new(r"\bchmod\s+777\b").expect("dangerous pattern regex is valid"),
            "world-writable permissions",
        ),
        (Regex::new(r"\bmkfs\b").expect("dangerous pattern regex is valid"), "filesystem format"),
        (Regex::new(r"\bdd\s+if=").expect("dangerous pattern regex is valid"), "raw disk write"),
        (Regex::new(r">\s*/dev/sd[a-z]").expect("dangerous pattern regex is valid"), "raw device write"),
        (Regex::new(r"\bgit\s+push\s+.*--force\b").expect("dangerous pattern regex is valid"), "force push"),
        (Regex::new(r"\bgit\s+push\s+-f\b").expect("dangerous pattern regex is valid"), "force push"),
        (Regex::new(r"\bgit\s+reset\s+--hard\b").expect("dangerous pattern regex is valid"), "hard reset"),
        (
            Regex::new(r"\bgit\s+clean\s+-[a-zA-Z]*f").expect("dangerous pattern regex is valid"),
            "forced clean",
        ),
        (
            Regex::new(r"\bnix\s+profile\s+install\b").expect("dangerous pattern regex is valid"),
            "nix profile install (use nix-shell instead)",
        ),
    ]
});

/// Check whether a command matches a dangerous pattern.
/// Returns the reason string if dangerous, None if safe.
pub fn check_dangerous(command: &str) -> Option<&'static str> {
    for (re, reason) in DANGEROUS_PATTERNS.iter() {
        if re.is_match(command) {
            return Some(reason);
        }
    }
    None
}

pub struct BashTool {
    definition: ToolDefinition,
    confirm_tx: Option<ConfirmTx>,
    process_monitor: Option<crate::procmon::ProcessMonitorHandle>,
}

impl BashTool {
    pub fn new() -> Self {
        Self {
            definition: ToolDefinition {
                name: "bash".to_string(),
                description: "Execute bash commands. Returns stdout + stderr combined, along with exit code. Output is truncated to 2000 lines or 50KB.".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "command": {
                            "type": "string",
                            "description": "Shell command to execute"
                        },
                        "timeout": {
                            "type": "number",
                            "description": "Timeout in seconds (optional, 0 = no timeout)"
                        }
                    },
                    "required": ["command"]
                }),
            },
            confirm_tx: None,
            process_monitor: None,
        }
    }

    /// Create a BashTool with a confirmation channel for dangerous commands.
    pub fn with_confirm(confirm_tx: ConfirmTx) -> Self {
        let mut tool = Self::new();
        tool.confirm_tx = Some(confirm_tx);
        tool
    }

    /// Attach a process monitor to track spawned processes.
    pub fn with_process_monitor(mut self, monitor: crate::procmon::ProcessMonitorHandle) -> Self {
        self.process_monitor = Some(monitor);
        self
    }
}

impl Default for BashTool {
    fn default() -> Self {
        Self::new()
    }
}

/// Maximum length for command preview in process monitor
const COMMAND_PREVIEW_LEN: usize = 200;

impl BashTool {
    /// Check if a command is dangerous and request user confirmation if needed.
    /// Returns Ok(()) if safe or approved, Err(ToolResult) if blocked.
    async fn check_and_confirm_dangerous(&self, command: &str) -> Result<(), ToolResult> {
        if let Some(reason) = check_dangerous(command) {
            if let Some(ref tx) = self.confirm_tx {
                let (resp_tx, resp_rx) = tokio::sync::oneshot::channel();
                let msg = format!("⚠️  Dangerous command detected ({}): {}", reason, command);
                if tx.send((msg, resp_tx)).is_ok() {
                    match resp_rx.await {
                        Ok(true) => { /* approved, continue */ }
                        _ => {
                            return Err(ToolResult::error(format!(
                                "Command blocked by user ({}). Rephrase the command or ask the user for approval.",
                                reason
                            )));
                        }
                    }
                }
            } else {
                // No confirm channel (headless mode) — block with explanation
                return Err(ToolResult::error(format!(
                    "⚠️  Dangerous command blocked ({}): {}\n\
                     This command was flagged as potentially destructive. \
                     In interactive mode, a confirmation prompt would appear. \
                     In headless mode, such commands are blocked by default.",
                    reason, command
                )));
            }
        }
        Ok(())
    }

    /// Spawn a bash child process with sanitized environment and sandboxing.
    fn spawn_command(&self, command: &str) -> Result<tokio::process::Child, ToolResult> {
        // Build sanitized environment (strips secrets, API keys, SSH agent, etc.)
        let clean_env = crate::tools::sandbox::sanitized_env();

        let mut cmd = Command::new("bash");
        cmd.arg("-c")
            .arg(command)
            .env_clear()
            .envs(clean_env)
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped());

        // On Linux: apply Landlock filesystem sandbox to the child process.
        // This restricts what files the shell command can access at the kernel
        // level, even if the command tries to read ~/.ssh or /etc/shadow.
        #[cfg(target_os = "linux")]
        {
            let cwd_for_landlock = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
            unsafe {
                cmd.pre_exec(move || {
                    if let Err(e) = crate::tools::sandbox::apply_landlock_to_current(&cwd_for_landlock) {
                        tracing::warn!("sandbox: landlock on bash child failed: {}", e);
                        // Don't fail the command — degrade gracefully
                    }
                    Ok(())
                });
            }
        }

        cmd.spawn().map_err(|e| ToolResult::error(format!("Failed to spawn bash: {}", e)))
    }

    /// Stream stdout and stderr from a child process, emitting progress and collecting output.
    /// Returns (collected_output, line_count).
    async fn stream_output(
        &self,
        child: &mut tokio::process::Child,
        ctx: &ToolContext,
        timeout_secs: u64,
    ) -> Result<(String, usize), ToolResult> {
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| ToolResult::error("Failed to capture stdout from child process"))?;
        let stderr = child
            .stderr
            .take()
            .ok_or_else(|| ToolResult::error("Failed to capture stderr from child process"))?;

        let mut stdout_reader = BufReader::new(stdout).lines();
        let mut stderr_reader = BufReader::new(stderr).lines();

        let mut collected = String::new();
        let mut line_count: usize = 0;

        let deadline = if timeout_secs > 0 {
            Some(tokio::time::Instant::now() + Duration::from_secs(timeout_secs))
        } else {
            None
        };

        // Read lines from both streams, emitting updates as we go
        loop {
            // Check timeout
            if let Some(dl) = deadline
                && tokio::time::Instant::now() >= dl
            {
                let _ = child.start_kill();
                ctx.emit_structured_progress(ToolProgress::phase("Timeout", 1, Some(1)));
                return Err(ToolResult::error(format!("Command timeout after {}s", timeout_secs)));
            }

            tokio::select! {
                () = ctx.signal.cancelled() => {
                    let _ = child.start_kill();
                    ctx.emit_structured_progress(ToolProgress::phase("Cancelling", 1, Some(1)));
                    return Err(ToolResult::error("Command cancelled"));
                }
                line = stdout_reader.next_line() => {
                    match line {
                        Ok(Some(raw)) => {
                            let line = strip_ansi(&raw);
                            ctx.emit_progress(&line);
                            ctx.emit_result_chunk(ResultChunk::text(&line));
                            if !collected.is_empty() {
                                collected.push('\n');
                            }
                            collected.push_str(&line);
                            line_count += 1;
                            ctx.emit_structured_progress(ToolProgress::lines(line_count as u64, None));
                        }
                        Ok(None) => {
                            // stdout closed — drain remaining stderr then break
                            while let Ok(Some(raw)) = stderr_reader.next_line().await {
                                let line = strip_ansi(&raw);
                                ctx.emit_progress(&line);
                                ctx.emit_result_chunk(ResultChunk::text(&line));
                                if !collected.is_empty() {
                                    collected.push('\n');
                                }
                                collected.push_str(&line);
                                line_count += 1;
                                ctx.emit_structured_progress(ToolProgress::lines(line_count as u64, None));
                            }
                            break;
                        }
                        Err(e) => return Err(ToolResult::error(format!("Read error: {}", e))),
                    }
                }
                line = stderr_reader.next_line() => {
                    match line {
                        Ok(Some(raw)) => {
                            let line = strip_ansi(&raw);
                            ctx.emit_progress(&line);
                            ctx.emit_result_chunk(ResultChunk::text(&line));
                            if !collected.is_empty() {
                                collected.push('\n');
                            }
                            collected.push_str(&line);
                            line_count += 1;
                            ctx.emit_structured_progress(ToolProgress::lines(line_count as u64, None));
                        }
                        Ok(None) => {
                            // stderr closed — drain remaining stdout then break
                            while let Ok(Some(raw)) = stdout_reader.next_line().await {
                                let line = strip_ansi(&raw);
                                ctx.emit_progress(&line);
                                ctx.emit_result_chunk(ResultChunk::text(&line));
                                if !collected.is_empty() {
                                    collected.push('\n');
                                }
                                collected.push_str(&line);
                                line_count += 1;
                                ctx.emit_structured_progress(ToolProgress::lines(line_count as u64, None));
                            }
                            break;
                        }
                        Err(e) => return Err(ToolResult::error(format!("Read error: {}", e))),
                    }
                }
            }
        }

        Ok((collected, line_count))
    }

    /// Format the final tool result with truncation and exit code handling.
    fn format_result(&self, collected_output: String, exit_code: i32) -> ToolResult {
        const MAX_LINES: usize = 2000;
        const MAX_BYTES: usize = 50 * 1024;

        let (truncated_output, full_output_path) =
            crate::tools::truncation::truncate_tail(&collected_output, MAX_LINES, MAX_BYTES);

        let result_text = if exit_code == 0 {
            truncated_output
        } else {
            format!("Exit code: {}\n\n{}", exit_code, truncated_output)
        };

        let mut result = ToolResult::text(result_text);
        if let Some(path) = full_output_path {
            result.full_output_path = Some(path.display().to_string());
        }

        if exit_code != 0 {
            result.is_error = true;
        }

        result
    }
}

#[async_trait]
impl Tool for BashTool {
    fn definition(&self) -> &ToolDefinition {
        &self.definition
    }

    async fn execute(&self, ctx: &ToolContext, params: Value) -> ToolResult {
        // Parse parameters
        let command = match params.get("command").and_then(|v| v.as_str()) {
            Some(c) => c,
            None => return ToolResult::error("Missing required parameter: command"),
        };
        let timeout_secs = params.get("timeout").and_then(|v| v.as_u64()).unwrap_or(0);

        // Check for dangerous commands and request confirmation if needed
        if let Err(result) = self.check_and_confirm_dangerous(command).await {
            return result;
        }

        // Spawn the bash child process with sandboxing
        let mut child = match self.spawn_command(command) {
            Ok(c) => c,
            Err(result) => return result,
        };

        // Register process with monitor
        if let Some(ref monitor) = self.process_monitor
            && let Some(pid) = child.id()
        {
            let command_preview: String = command.chars().take(COMMAND_PREVIEW_LEN).collect();
            monitor.register(pid, crate::procmon::ProcessMeta {
                tool_name: "bash".to_string(),
                command: command_preview,
                call_id: ctx.call_id.clone(),
            });
        }

        // Stream output from the child process
        let (collected_output, _line_count) = match self.stream_output(&mut child, ctx, timeout_secs).await {
            Ok(result) => result,
            Err(result) => return result,
        };

        // Wait for the process to exit
        let status = match child.wait().await {
            Ok(s) => s,
            Err(e) => return ToolResult::error(format!("Failed to wait for command: {}", e)),
        };

        let exit_code = status.code().unwrap_or(-1); // Process terminated by signal or had no exit code

        // Format and return the final result
        self.format_result(collected_output, exit_code)
    }
}

#[cfg(test)]
mod tests {
    use super::check_dangerous;
    use crate::util::ansi::strip_ansi;

    #[test]
    fn strip_plain_text_unchanged() {
        assert_eq!(strip_ansi("hello world"), "hello world");
    }

    #[test]
    fn strip_csi_color_codes() {
        assert_eq!(strip_ansi("\x1b[32mOK\x1b[0m"), "OK");
        assert_eq!(strip_ansi("\x1b[1;31merror\x1b[0m: bad"), "error: bad");
    }

    #[test]
    fn strip_cursor_movement() {
        // CSI sequences like \x1b[2K (erase line), \x1b[1A (cursor up)
        assert_eq!(strip_ansi("\x1b[2Khello\x1b[1A"), "hello");
    }

    #[test]
    fn strip_osc_sequences() {
        // OSC title set: \x1b]0;title\x07
        assert_eq!(strip_ansi("\x1b]0;my title\x07content"), "content");
    }

    #[test]
    fn strip_carriage_returns() {
        assert_eq!(strip_ansi("progress\r100%"), "progress100%");
    }

    #[test]
    fn strip_complex_cargo_output() {
        let input = "\x1b[0m\x1b[1m\x1b[32m   Compiling\x1b[0m clankers v0.1.0";
        assert_eq!(strip_ansi(input), "   Compiling clankers v0.1.0");
    }

    // ── Dangerous command detection tests ─────────────────────────────

    #[test]
    fn dangerous_rm_rf() {
        assert!(check_dangerous("rm -rf /tmp/foo").is_some());
        assert!(check_dangerous("rm -f important.txt").is_some());
        assert!(check_dangerous("rm --force file").is_some());
    }

    #[test]
    fn dangerous_sudo_rm() {
        assert!(check_dangerous("sudo rm /etc/something").is_some());
    }

    #[test]
    fn dangerous_sql() {
        assert!(check_dangerous("psql -c 'DROP TABLE users'").is_some());
        assert!(check_dangerous("mysql -e 'TRUNCATE orders'").is_some());
        assert!(check_dangerous("sqlite3 db.sqlite 'DELETE FROM logs'").is_some());
    }

    #[test]
    fn dangerous_git() {
        assert!(check_dangerous("git push --force origin main").is_some());
        assert!(check_dangerous("git push -f").is_some());
        assert!(check_dangerous("git reset --hard HEAD~5").is_some());
        assert!(check_dangerous("git clean -fd").is_some());
    }

    #[test]
    fn dangerous_disk() {
        assert!(check_dangerous("mkfs.ext4 /dev/sda1").is_some());
        assert!(check_dangerous("dd if=/dev/zero of=/dev/sda").is_some());
        assert!(check_dangerous("chmod 777 /var/www").is_some());
    }

    #[test]
    fn dangerous_nix_profile_install() {
        assert!(check_dangerous("nix profile install nixpkgs#python3").is_some());
    }

    #[test]
    fn safe_commands_not_flagged() {
        assert!(check_dangerous("ls -la").is_none());
        assert!(check_dangerous("cat file.txt").is_none());
        assert!(check_dangerous("grep -r pattern .").is_none());
        assert!(check_dangerous("git push origin main").is_none());
        assert!(check_dangerous("git commit -m 'fix'").is_none());
        assert!(check_dangerous("rm file.txt").is_none()); // no -f
        assert!(check_dangerous("cargo build").is_none());
        assert!(check_dangerous("nix-shell -p python3 --run 'python3 script.py'").is_none());
        assert!(check_dangerous("nix run nixpkgs#hello").is_none());
    }
}
