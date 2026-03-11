//! Nix command execution and output streaming

use tokio::io::AsyncBufReadExt;
use tokio::io::BufReader;
use tokio::process::Command;
use tokio::time::Duration;

use super::super::ToolContext;
use super::super::ToolResult;
use super::parser::NixOutputState;
use super::parser::format_nix_result;
use super::parser::process_nix_line;
use crate::util::ansi::strip_ansi;

// ── nom (nix-output-monitor) detection ──────────────────────────────────────

// NOTE: nix-output-monitor (nom) was evaluated as a wrapper but rejected.
// nom is a TUI app that uses cursor control sequences ([1G, [2K, [1F) and
// box-drawing characters even when piped or with TERM=dumb. Its output cannot
// be streamed line-by-line to panes. The internal-json parser below provides
// cleaner, more controllable streaming output.

// ── Nix subcommands ─────────────────────────────────────────────────────────

/// Which nix subcommands support `--log-format internal-json`
pub fn supports_structured_logging(subcommand: &str) -> bool {
    matches!(
        subcommand,
        "build" | "develop" | "run" | "shell" | "flake" | "eval" | "profile" | "store" | "derivation" | "log"
    )
}

/// Spawn a nix command with appropriate flags and sandboxing
pub fn spawn_nix_command(
    subcommand: &str,
    args: &[String],
    use_structured: bool,
) -> Result<tokio::process::Child, String> {
    let clean_env = crate::tools::sandbox::sanitized_env();

    let mut cmd = Command::new("nix");
    cmd.arg(subcommand);

    // Inject --log-format internal-json for structured output
    if use_structured {
        cmd.arg("--log-format").arg("internal-json");
        // Also print build logs so we get BuildLogLine events
        cmd.arg("-L");
    }

    // Add user args (skip if user already passed --log-format or -L)
    for arg in args {
        if arg == "--log-format" || arg == "-L" || arg == "--print-build-logs" {
            continue; // We already added these
        }
        cmd.arg(arg);
    }

    cmd.env_clear()
        .envs(clean_env)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());

    // Apply Landlock sandbox on Linux
    #[cfg(target_os = "linux")]
    {
        let cwd_for_landlock = std::env::current_dir().unwrap_or_default();
        unsafe {
            cmd.pre_exec(move || {
                if let Err(e) = crate::tools::sandbox::apply_landlock_to_current(&cwd_for_landlock) {
                    tracing::warn!("sandbox: landlock on nix child failed: {}", e);
                }
                Ok(())
            });
        }
    }

    cmd.spawn().map_err(|e| format!("Failed to spawn nix: {}", e))
}

/// Stream and parse nix output with structured logging support
pub async fn stream_nix_output(
    ctx: &ToolContext,
    child: &mut tokio::process::Child,
    use_structured: bool,
    timeout_secs: u64,
    subcommand: &str,
) -> Result<(i32, Vec<String>, Vec<String>, Vec<String>, Vec<String>), ToolResult> {
    let stdout = child.stdout.take().ok_or_else(|| ToolResult::error("Failed to capture stdout"))?;
    let stderr = child.stderr.take().ok_or_else(|| ToolResult::error("Failed to capture stderr"))?;

    let mut stdout_reader = BufReader::new(stdout).lines();
    let mut stderr_reader = BufReader::new(stderr).lines();

    let deadline = if timeout_secs > 0 {
        Some(tokio::time::Instant::now() + Duration::from_secs(timeout_secs))
    } else {
        None
    };

    // Collected outputs
    let mut stdout_lines: Vec<String> = Vec::new();
    let mut nix_state = NixOutputState::new();

    // Stream and parse output
    loop {
        if let Some(dl) = deadline
            && tokio::time::Instant::now() >= dl
        {
            let _ = child.start_kill();
            return Err(ToolResult::error(format!("nix {} timed out after {}s", subcommand, timeout_secs)));
        }

        tokio::select! {
            () = ctx.signal.cancelled() => {
                let _ = child.start_kill();
                return Err(ToolResult::error("nix command cancelled"));
            }
            line = stdout_reader.next_line() => {
                match line {
                    Ok(Some(raw)) => {
                        let line = strip_ansi(&raw);
                        if !line.is_empty() {
                            ctx.emit_progress(&line);
                            stdout_lines.push(line);
                        }
                    }
                    Ok(None) => {
                        // stdout closed — drain stderr
                        while let Ok(Some(raw)) = stderr_reader.next_line().await {
                            let line = strip_ansi(&raw);
                            if use_structured {
                                process_nix_line(&line, ctx, &mut nix_state);
                            } else if !line.is_empty() {
                                ctx.emit_progress(&line);
                                nix_state.messages.push(line);
                            }
                        }
                        break;
                    }
                    Err(e) => return Err(ToolResult::error(format!("stdout read error: {}", e))),
                }
            }
            line = stderr_reader.next_line() => {
                match line {
                    Ok(Some(raw)) => {
                        let line = strip_ansi(&raw);
                        if use_structured {
                            process_nix_line(&line, ctx, &mut nix_state);
                        } else if !line.is_empty() {
                            ctx.emit_progress(&line);
                            nix_state.messages.push(line);
                        }
                    }
                    Ok(None) => {
                        // stderr closed — drain stdout
                        while let Ok(Some(raw)) = stdout_reader.next_line().await {
                            let line = strip_ansi(&raw);
                            if !line.is_empty() {
                                ctx.emit_progress(&line);
                                stdout_lines.push(line);
                            }
                        }
                        break;
                    }
                    Err(e) => return Err(ToolResult::error(format!("stderr read error: {}", e))),
                }
            }
        }
    }

    let status = child.wait().await.map_err(|e| ToolResult::error(format!("Failed to wait for nix: {}", e)))?;

    let exit_code = status.code().unwrap_or(-1);
    Ok((exit_code, stdout_lines, nix_state.build_log_lines, nix_state.messages, nix_state.errors))
}

/// Format and truncate the nix result for LLM consumption
pub fn format_and_truncate_result(
    subcommand: &str,
    exit_code: i32,
    stdout_lines: &[String],
    build_log_lines: &[String],
    messages: &[String],
    errors: &[String],
) -> ToolResult {
    let output = format_nix_result(subcommand, exit_code, stdout_lines, build_log_lines, messages, errors);

    // Apply truncation
    const MAX_LINES: usize = 2000;
    const MAX_BYTES: usize = 50 * 1024;
    let (truncated, full_path) = crate::tools::truncation::truncate_tail(&output, MAX_LINES, MAX_BYTES);

    let mut result = ToolResult::text(truncated);
    if let Some(path) = full_path {
        result.full_output_path = Some(path.display().to_string());
    }
    if exit_code != 0 {
        result.is_error = true;
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn supports_structured_for_build() {
        assert!(supports_structured_logging("build"));
        assert!(supports_structured_logging("develop"));
        assert!(supports_structured_logging("run"));
        assert!(supports_structured_logging("flake"));
    }

    #[test]
    fn no_structured_for_unknown() {
        assert!(!supports_structured_logging("repl"));
        assert!(!supports_structured_logging("search"));
        assert!(!supports_structured_logging("doctor"));
    }
}
