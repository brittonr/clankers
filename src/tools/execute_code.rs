//! Execute Rust cargo scripts for structured one-off automation.
//!
//! This is the Rust analogue of Hermes' Python `execute_code` tool. The agent
//! supplies a complete single-file Rust program; clankers writes it to a
//! temporary `.rs` file and runs it with Cargo's nightly `-Zscript` support.

use std::path::PathBuf;

use async_trait::async_trait;
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

const DEFAULT_TIMEOUT_SECS: u64 = 300;
const MAX_LINES: usize = 2000;
const MAX_BYTES: usize = 50 * 1024;

pub struct ExecuteCodeTool {
    definition: ToolDefinition,
}

impl ExecuteCodeTool {
    pub fn new() -> Self {
        Self {
            definition: ToolDefinition {
                name: "execute_code".to_string(),
                description: concat!(
                    "Run a complete Rust cargo script for structured one-off automation. ",
                    "Use this instead of long bash/Python when you need typed logic, JSON/TOML parsing, ",
                    "loops, or several filesystem/command steps with reduced context output. The `code` ",
                    "must be a full single-file Rust program. It is executed with `cargo -Zscript`; include ",
                    "an embedded Cargo manifest in `//! ```cargo` comments if crates are needed. Prints ",
                    "stdout/stderr plus exit code; output is truncated to 2000 lines or 50KB."
                )
                .to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "code": {
                            "type": "string",
                            "description": "Complete single-file Rust program to run with cargo -Zscript"
                        },
                        "args": {
                            "type": "array",
                            "items": { "type": "string" },
                            "description": "Optional argv entries passed to the Rust script"
                        },
                        "timeout": {
                            "type": "number",
                            "description": "Timeout in seconds (default 300; 0 means no timeout)"
                        }
                    },
                    "required": ["code"]
                }),
            },
        }
    }

    fn parse_args(params: &Value) -> Result<Vec<String>, ToolResult> {
        let Some(value) = params.get("args") else {
            return Ok(Vec::new());
        };
        let Some(values) = value.as_array() else {
            return Err(ToolResult::error("Parameter 'args' must be an array of strings."));
        };
        let mut args = Vec::with_capacity(values.len());
        for value in values {
            let Some(arg) = value.as_str() else {
                return Err(ToolResult::error("Parameter 'args' must be an array of strings."));
            };
            args.push(arg.to_string());
        }
        Ok(args)
    }

    fn make_script_path() -> Result<PathBuf, ToolResult> {
        let mut dir = std::env::temp_dir();
        dir.push("clankers-cargo-scripts");
        std::fs::create_dir_all(&dir)
            .map_err(|e| ToolResult::error(format!("Failed to create cargo script temp dir: {e}")))?;
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|e| ToolResult::error(format!("System clock error: {e}")))?;
        dir.push(format!("script-{}-{}.rs", std::process::id(), now.as_nanos()));
        Ok(dir)
    }

    fn target_dir() -> PathBuf {
        let mut dir = std::env::temp_dir();
        dir.push("clankers-cargo-script-target");
        dir
    }

    fn cargo_home() -> PathBuf {
        let mut dir = std::env::temp_dir();
        dir.push("clankers-cargo-script-home");
        dir
    }

    fn spawn_script(path: &PathBuf, args: &[String]) -> Result<tokio::process::Child, ToolResult> {
        let clean_env = crate::tools::sandbox::sanitized_env();
        let mut cmd = Command::new("cargo");
        cmd.arg("-q")
            .arg("-Zscript")
            .arg(path)
            .args(args)
            .env_clear()
            .envs(clean_env)
            .env("CARGO_HOME", Self::cargo_home())
            .env("CARGO_TARGET_DIR", Self::target_dir())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped());

        #[cfg(target_os = "linux")]
        {
            let cwd_for_landlock = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
            unsafe {
                cmd.pre_exec(move || {
                    if let Err(e) = crate::tools::sandbox::apply_landlock_to_current(&cwd_for_landlock) {
                        tracing::warn!("sandbox: landlock on cargo script child failed: {}", e);
                    }
                    Ok(())
                });
            }
        }

        cmd.spawn().map_err(|e| ToolResult::error(format!("Failed to spawn cargo script: {e}")))
    }

    async fn stream_output(
        child: &mut tokio::process::Child,
        ctx: &ToolContext,
        timeout_secs: u64,
    ) -> Result<String, ToolResult> {
        let stdout =
            child.stdout.take().ok_or_else(|| ToolResult::error("Failed to capture stdout from cargo script"))?;
        let stderr =
            child.stderr.take().ok_or_else(|| ToolResult::error("Failed to capture stderr from cargo script"))?;
        let mut out = BufReader::new(stdout).lines();
        let mut err = BufReader::new(stderr).lines();
        let mut collected = String::new();
        let mut line_count = 0usize;
        let deadline = (timeout_secs > 0).then(|| tokio::time::Instant::now() + Duration::from_secs(timeout_secs));

        loop {
            if deadline.is_some_and(|dl| tokio::time::Instant::now() >= dl) {
                child.start_kill().ok();
                return Err(ToolResult::error(format!("Cargo script timeout after {timeout_secs}s")));
            }
            tokio::select! {
                () = ctx.signal.cancelled() => {
                    child.start_kill().ok();
                    return Err(ToolResult::error("Cargo script cancelled"));
                }
                line = out.next_line() => match line {
                    Ok(Some(raw)) => collect_line(&raw, ctx, &mut collected, &mut line_count),
                    Ok(None) => { drain_reader(&mut err, ctx, &mut collected, &mut line_count).await; break; }
                    Err(e) => return Err(ToolResult::error(format!("Read error: {e}"))),
                },
                line = err.next_line() => match line {
                    Ok(Some(raw)) => collect_line(&raw, ctx, &mut collected, &mut line_count),
                    Ok(None) => { drain_reader(&mut out, ctx, &mut collected, &mut line_count).await; break; }
                    Err(e) => return Err(ToolResult::error(format!("Read error: {e}"))),
                },
            }
        }
        Ok(collected)
    }

    fn format_result(collected_output: String, exit_code: i32) -> ToolResult {
        let (truncated_output, full_output_path) =
            crate::tools::truncation::truncate_tail(&collected_output, MAX_LINES, MAX_BYTES);

        let result_text = if exit_code == 0 {
            truncated_output
        } else {
            format!("Exit code: {exit_code}\n\n{truncated_output}")
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

impl Default for ExecuteCodeTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for ExecuteCodeTool {
    fn definition(&self) -> &ToolDefinition {
        &self.definition
    }

    async fn execute(&self, ctx: &ToolContext, params: Value) -> ToolResult {
        let code = match params.get("code").and_then(|v| v.as_str()) {
            Some(code) if !code.trim().is_empty() => code,
            _ => return ToolResult::error("Missing required parameter: code"),
        };
        let args = match Self::parse_args(&params) {
            Ok(args) => args,
            Err(result) => return result,
        };
        let timeout_secs = params.get("timeout").and_then(|v| v.as_u64()).unwrap_or(DEFAULT_TIMEOUT_SECS);

        let script_path = match Self::make_script_path() {
            Ok(path) => path,
            Err(result) => return result,
        };
        if let Err(e) = tokio::fs::write(&script_path, code).await {
            return ToolResult::error(format!("Failed to write cargo script: {e}"));
        }

        let mut child = match Self::spawn_script(&script_path, &args) {
            Ok(child) => child,
            Err(result) => {
                tokio::fs::remove_file(&script_path).await.ok();
                return result;
            }
        };

        let collected_output = match Self::stream_output(&mut child, ctx, timeout_secs).await {
            Ok(output) => output,
            Err(result) => {
                tokio::fs::remove_file(&script_path).await.ok();
                return result;
            }
        };

        let status = match child.wait().await {
            Ok(status) => status,
            Err(e) => {
                tokio::fs::remove_file(&script_path).await.ok();
                return ToolResult::error(format!("Failed to wait for cargo script: {e}"));
            }
        };
        tokio::fs::remove_file(&script_path).await.ok();
        Self::format_result(collected_output, status.code().unwrap_or(-1))
    }
}

fn collect_line(raw: &str, ctx: &ToolContext, collected: &mut String, line_count: &mut usize) {
    let line = strip_ansi(raw);
    ctx.emit_progress(&line);
    ctx.emit_result_chunk(ResultChunk::text(&line));
    if !collected.is_empty() {
        collected.push('\n');
    }
    collected.push_str(&line);
    *line_count += 1;
    ctx.emit_structured_progress(ToolProgress::lines(*line_count as u64, None));
}

async fn drain_reader(
    reader: &mut tokio::io::Lines<BufReader<impl tokio::io::AsyncRead + Unpin>>,
    ctx: &ToolContext,
    collected: &mut String,
    line_count: &mut usize,
) {
    while let Ok(Some(raw)) = reader.next_line().await {
        collect_line(&raw, ctx, collected, line_count);
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;
    use tokio_util::sync::CancellationToken;

    use super::*;

    fn make_ctx() -> ToolContext {
        ToolContext::new("test-call".to_string(), CancellationToken::new(), None)
    }

    #[test]
    fn definition_uses_execute_code_name() {
        let tool = ExecuteCodeTool::new();
        assert_eq!(tool.definition().name, "execute_code");
    }

    #[test]
    fn parse_args_rejects_non_strings() {
        let params = json!({"args": ["ok", 1]});
        assert!(ExecuteCodeTool::parse_args(&params).is_err());
    }

    #[tokio::test]
    async fn runs_simple_cargo_script() {
        let tool = ExecuteCodeTool::new();
        let result = tool
            .execute(
                &make_ctx(),
                json!({
                    "code": "fn main() { println!(\"hello from cargo script\"); }",
                    "timeout": 120,
                }),
            )
            .await;
        assert!(!result.is_error, "{result:?}");
        let text = result
            .content
            .iter()
            .filter_map(|content| match content {
                super::super::ToolResultContent::Text { text } => Some(text.as_str()),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("\n");
        assert!(text.contains("hello from cargo script"), "{text}");
    }

    #[tokio::test]
    async fn passes_args_to_script() {
        let tool = ExecuteCodeTool::new();
        let result = tool
            .execute(
                &make_ctx(),
                json!({
                    "code": "fn main() { for arg in std::env::args().skip(1) { println!(\"{arg}\"); } }",
                    "args": ["one", "two"],
                    "timeout": 120,
                }),
            )
            .await;
        assert!(!result.is_error, "{result:?}");
        let text = result
            .content
            .iter()
            .filter_map(|content| match content {
                super::super::ToolResultContent::Text { text } => Some(text.as_str()),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("\n");
        assert!(text.contains("one"), "{text}");
        assert!(text.contains("two"), "{text}");
    }
}
