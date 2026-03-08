//! Nix build/develop/run tool with structured output streaming
//!
//! Parses nix's `--log-format internal-json` to provide clean, meaningful
//! progress updates instead of raw terminal noise. Supports all common nix
//! subcommands: build, develop, run, shell, flake check/show/update.

use std::collections::HashMap;

use async_trait::async_trait;
use serde::Deserialize;
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
use crate::util::ansi::strip_ansi;

// ── Nix internal-json protocol types ────────────────────────────────────────

/// Activity types from nix's internal-json format.
/// See: https://github.com/NixOS/nix/blob/master/src/libutil/logging.hh
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u64)]
enum ActivityType {
    Unknown = 0,
    CopyPath = 100,
    FileTransfer = 101,
    Realise = 102,
    CopyPaths = 103,
    Builds = 104,
    Build = 105,
    OptimiseStore = 106,
    VerifyPaths = 107,
    Substitute = 108,
    QueryPathInfo = 109,
    PostBuildHook = 110,
    BuildWaiting = 111,
    FetchTree = 112,
}

impl ActivityType {
    fn from_u64(v: u64) -> Self {
        match v {
            100 => Self::CopyPath,
            101 => Self::FileTransfer,
            102 => Self::Realise,
            103 => Self::CopyPaths,
            104 => Self::Builds,
            105 => Self::Build,
            106 => Self::OptimiseStore,
            107 => Self::VerifyPaths,
            108 => Self::Substitute,
            109 => Self::QueryPathInfo,
            110 => Self::PostBuildHook,
            111 => Self::BuildWaiting,
            112 => Self::FetchTree,
            _ => Self::Unknown,
        }
    }

    fn label(&self) -> &'static str {
        match self {
            Self::Unknown => "working",
            Self::CopyPath => "copying",
            Self::FileTransfer => "downloading",
            Self::Realise => "realising",
            Self::CopyPaths => "copying paths",
            Self::Builds => "building",
            Self::Build => "building",
            Self::OptimiseStore => "optimising store",
            Self::VerifyPaths => "verifying",
            Self::Substitute => "fetching",
            Self::QueryPathInfo => "querying",
            Self::PostBuildHook => "post-build hook",
            Self::BuildWaiting => "waiting for build",
            Self::FetchTree => "fetching tree",
        }
    }
}

/// Result types from nix's internal-json format.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u64)]
enum ResultType {
    FileLinked = 100,
    BuildLogLine = 101,
    UntrustedPath = 102,
    CorruptedPath = 103,
    SetPhase = 104,
    Progress = 105,
}

impl ResultType {
    fn from_u64(v: u64) -> Option<Self> {
        match v {
            100 => Some(Self::FileLinked),
            101 => Some(Self::BuildLogLine),
            102 => Some(Self::UntrustedPath),
            103 => Some(Self::CorruptedPath),
            104 => Some(Self::SetPhase),
            105 => Some(Self::Progress),
            _ => None,
        }
    }
}

/// Nix log level
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[repr(u64)]
enum NixLogLevel {
    Error = 0,
    Warn = 1,
    Notice = 2,
    Info = 3,
    Talkative = 4,
    Chatty = 5,
    Debug = 6,
    Vomit = 7,
}

impl NixLogLevel {
    fn from_u64(v: u64) -> Self {
        match v {
            0 => Self::Error,
            1 => Self::Warn,
            2 => Self::Notice,
            3 => Self::Info,
            4 => Self::Talkative,
            5 => Self::Chatty,
            6 => Self::Debug,
            _ => Self::Vomit,
        }
    }

}

/// Parsed nix internal-json event
#[derive(Debug, Deserialize)]
struct NixEvent {
    action: String,
    #[serde(default)]
    id: u64,
    #[serde(default)]
    level: u64,
    #[serde(default)]
    text: String,
    #[serde(default)]
    msg: String,
    #[serde(default)]
    raw_msg: String,
    #[serde(rename = "type", default)]
    activity_type: u64,
    #[serde(default)]
    fields: Vec<Value>,
    #[serde(default)]
    #[allow(dead_code)] // deserialized by serde, not read directly
    parent: u64,
}

/// Tracks active nix activities for progress display
struct ActivityTracker {
    activities: HashMap<u64, TrackedActivity>,
}

struct TrackedActivity {
    text: String,
    activity_type: ActivityType,
    #[allow(dead_code)] // retained for future activity display
    phase: Option<String>,
}

impl ActivityTracker {
    fn new() -> Self {
        Self {
            activities: HashMap::new(),
        }
    }

    fn start(&mut self, id: u64, text: String, activity_type: ActivityType) {
        self.activities.insert(id, TrackedActivity {
            text,
            activity_type,
            phase: None,
        });
    }

    fn stop(&mut self, id: u64) {
        self.activities.remove(&id);
    }

    fn set_phase(&mut self, id: u64, phase: String) {
        if let Some(a) = self.activities.get_mut(&id) {
            a.phase = Some(phase);
        }
    }

    fn get(&self, id: u64) -> Option<&TrackedActivity> {
        self.activities.get(&id)
    }
}

// ── Nix subcommands ─────────────────────────────────────────────────────────

/// Which nix subcommands support `--log-format internal-json`
fn supports_structured_logging(subcommand: &str) -> bool {
    matches!(
        subcommand,
        "build" | "develop" | "run" | "shell" | "flake" | "eval" | "profile" | "store" | "derivation" | "log"
    )
}

// ── nom (nix-output-monitor) detection ──────────────────────────────────────

// NOTE: nix-output-monitor (nom) was evaluated as a wrapper but rejected.
// nom is a TUI app that uses cursor control sequences ([1G, [2K, [1F) and
// box-drawing characters even when piped or with TERM=dumb. Its output cannot
// be streamed line-by-line to panes. The internal-json parser below provides
// cleaner, more controllable streaming output.

// ── Tool implementation ─────────────────────────────────────────────────────

pub struct NixTool {
    definition: ToolDefinition,
}

impl NixTool {
    pub fn new() -> Self {
        Self {
            definition: ToolDefinition {
                name: "nix".to_string(),
                description: "Run nix commands with streaming build output. Supports build, develop, run, shell, flake, eval, and other nix subcommands. Parses nix's internal-json structured logging for clean progress display (builds, downloads, fetches, phases). Use this instead of bash for nix commands.".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "subcommand": {
                            "type": "string",
                            "description": "Nix subcommand (build, develop, run, shell, flake, eval, store, etc.)"
                        },
                        "args": {
                            "type": "array",
                            "items": { "type": "string" },
                            "description": "Arguments to pass after the subcommand (e.g. [\".#myPackage\", \"--no-link\"])"
                        },
                        "timeout": {
                            "type": "number",
                            "description": "Timeout in seconds (default: 600 for builds, 0 = no timeout)"
                        }
                    },
                    "required": ["subcommand"]
                }),
            },
        }
    }
}

impl Default for NixTool {
    fn default() -> Self {
        Self::new()
    }
}

// ── Helper functions ────────────────────────────────────────────────────────

/// Spawn a nix command with appropriate flags and sandboxing
fn spawn_nix_command(
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
async fn stream_nix_output(
    ctx: &ToolContext,
    child: &mut tokio::process::Child,
    use_structured: bool,
    timeout_secs: u64,
    subcommand: &str,
) -> Result<(i32, Vec<String>, Vec<String>, Vec<String>, Vec<String>), ToolResult> {
    let stdout = child.stdout.take()
        .ok_or_else(|| ToolResult::error("Failed to capture stdout"))?;
    let stderr = child.stderr.take()
        .ok_or_else(|| ToolResult::error("Failed to capture stderr"))?;

    let mut stdout_reader = BufReader::new(stdout).lines();
    let mut stderr_reader = BufReader::new(stderr).lines();

    let deadline = if timeout_secs > 0 {
        Some(tokio::time::Instant::now() + Duration::from_secs(timeout_secs))
    } else {
        None
    };

    // Collected outputs
    let mut stdout_lines: Vec<String> = Vec::new();
    let mut build_log_lines: Vec<String> = Vec::new();
    let mut messages: Vec<String> = Vec::new();
    let mut errors: Vec<String> = Vec::new();

    let mut tracker = ActivityTracker::new();
    let mut last_progress_text: Option<String> = None;

    // Stream and parse output
    loop {
        if let Some(dl) = deadline {
            if tokio::time::Instant::now() >= dl {
                let _ = child.start_kill();
                return Err(ToolResult::error(format!("nix {} timed out after {}s", subcommand, timeout_secs)));
            }
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
                                process_nix_line(
                                    &line, ctx, &mut tracker,
                                    &mut build_log_lines, &mut messages, &mut errors,
                                    &mut last_progress_text,
                                );
                            } else if !line.is_empty() {
                                ctx.emit_progress(&line);
                                messages.push(line);
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
                            process_nix_line(
                                &line, ctx, &mut tracker,
                                &mut build_log_lines, &mut messages, &mut errors,
                                &mut last_progress_text,
                            );
                        } else if !line.is_empty() {
                            ctx.emit_progress(&line);
                            messages.push(line);
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

    let status = child.wait().await
        .map_err(|e| ToolResult::error(format!("Failed to wait for nix: {}", e)))?;

    let exit_code = status.code().unwrap_or(-1);
    Ok((exit_code, stdout_lines, build_log_lines, messages, errors))
}

/// Format and truncate the nix result for LLM consumption
fn format_and_truncate_result(
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

#[async_trait]
impl Tool for NixTool {
    fn definition(&self) -> &ToolDefinition {
        &self.definition
    }

    async fn execute(&self, ctx: &ToolContext, params: Value) -> ToolResult {
        let subcommand = match params.get("subcommand").and_then(|v| v.as_str()) {
            Some(s) => s.to_string(),
            None => return ToolResult::error("Missing required parameter: subcommand"),
        };

        let args: Vec<String> = params
            .get("args")
            .and_then(|v| v.as_array())
            .map(|arr| arr.iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect())
            .unwrap_or_default();

        let timeout_secs = params.get("timeout").and_then(|v| v.as_u64()).unwrap_or(0);

        // Decide whether to use structured logging
        let use_structured = supports_structured_logging(&subcommand);

        // Spawn the nix command
        let mut child = match spawn_nix_command(&subcommand, &args, use_structured) {
            Ok(c) => c,
            Err(e) => return ToolResult::error(e),
        };

        // Stream output and collect results
        let (exit_code, stdout_lines, build_log_lines, messages, errors) = 
            match stream_nix_output(ctx, &mut child, use_structured, timeout_secs, &subcommand).await {
                Ok(result) => result,
                Err(e) => return e,
            };

        // Format and truncate the result
        format_and_truncate_result(&subcommand, exit_code, &stdout_lines, &build_log_lines, &messages, &errors)
    }
}

/// Parse a single stderr line from nix's internal-json output
fn process_nix_line(
    line: &str,
    ctx: &ToolContext,
    tracker: &mut ActivityTracker,
    build_log_lines: &mut Vec<String>,
    messages: &mut Vec<String>,
    errors: &mut Vec<String>,
    last_progress_text: &mut Option<String>,
) {
    // Internal-json lines start with "@nix "
    let json_str = match line.strip_prefix("@nix ") {
        Some(s) => s,
        None => {
            // Not a nix structured line — treat as raw output
            if !line.is_empty() {
                ctx.emit_progress(line);
                messages.push(line.to_string());
            }
            return;
        }
    };

    let event: NixEvent = match serde_json::from_str(json_str) {
        Ok(e) => e,
        Err(_) => return,
    };

    match event.action.as_str() {
        "start" => {
            let activity_type = ActivityType::from_u64(event.activity_type);
            let level = NixLogLevel::from_u64(event.level);
            let text = event.text.clone();

            tracker.start(event.id, text.clone(), activity_type);

            // Only emit progress for interesting activities
            match activity_type {
                ActivityType::Build | ActivityType::Substitute => {
                    // Extract derivation name from text like "building '/nix/store/...-name.drv'"
                    let display = shorten_drv_path(&text);
                    let msg = format!("⚙ {} {}", activity_type.label(), display);
                    ctx.emit_progress(&msg);
                    messages.push(msg);
                }
                ActivityType::FileTransfer => {
                    if level <= NixLogLevel::Talkative {
                        let msg = format!("↓ {}", shorten_url(&text));
                        ctx.emit_progress(&msg);
                    }
                }
                ActivityType::FetchTree => {
                    let msg = format!("🌲 fetching {}", shorten_url(&text));
                    ctx.emit_progress(&msg);
                    messages.push(msg);
                }
                ActivityType::CopyPath | ActivityType::CopyPaths => {
                    if level <= NixLogLevel::Info && !text.is_empty() {
                        let msg = format!("📦 {}", shorten_store_path(&text));
                        // Dedup noisy copy messages
                        if last_progress_text.as_deref() != Some(&msg) {
                            ctx.emit_progress(&msg);
                            *last_progress_text = Some(msg);
                        }
                    }
                }
                ActivityType::PostBuildHook => {
                    let msg = format!("🪝 post-build hook: {}", text);
                    ctx.emit_progress(&msg);
                    messages.push(msg);
                }
                _ => {
                    // Emit non-trivial activities at info level or below
                    if level <= NixLogLevel::Info && !text.is_empty() {
                        ctx.emit_progress(&text);
                    }
                }
            }
        }
        "stop" => {
            tracker.stop(event.id);
        }
        "result" => {
            if let Some(result_type) = ResultType::from_u64(event.activity_type) {
                match result_type {
                    ResultType::BuildLogLine => {
                        // fields[0] is the log line
                        if let Some(log_line) = event.fields.first().and_then(|v| v.as_str()) {
                            let clean = strip_ansi(log_line);
                            ctx.emit_progress(&format!("  │ {}", clean));
                            build_log_lines.push(clean);
                        }
                    }
                    ResultType::SetPhase => {
                        if let Some(phase) = event.fields.first().and_then(|v| v.as_str()) {
                            tracker.set_phase(event.id, phase.to_string());
                            // Get the activity name for context
                            let activity_name =
                                tracker.get(event.id).map(|a| shorten_drv_path(&a.text)).unwrap_or_default();
                            let msg = format!("  ▸ phase: {} ({})", phase, activity_name);
                            ctx.emit_progress(&msg);
                            messages.push(msg);
                        }
                    }
                    ResultType::Progress => {
                        // fields: [done, expected, running, failed]
                        // Only emit when there's meaningful progress
                        if event.fields.len() >= 2 {
                            let done = event.fields[0].as_u64().unwrap_or(0);
                            let expected = event.fields[1].as_u64().unwrap_or(0);
                            if expected > 0 && done > 0 {
                                // Look up what activity this belongs to
                                let label =
                                    tracker.get(event.id).map(|a| a.activity_type.label()).unwrap_or("progress");
                                let msg = format!("  {} {}/{}", label, done, expected);
                                if last_progress_text.as_deref() != Some(&msg) {
                                    ctx.emit_progress(&msg);
                                    *last_progress_text = Some(msg);
                                }
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
        "msg" => {
            let level = NixLogLevel::from_u64(event.level);
            // Use raw_msg if available (cleaner), fall back to msg
            let text = if !event.raw_msg.is_empty() {
                strip_ansi(&event.raw_msg)
            } else {
                strip_ansi(&event.msg)
            };

            if text.is_empty() {
                return;
            }

            match level {
                NixLogLevel::Error => {
                    let msg = format!("✗ {}", text);
                    ctx.emit_progress(&msg);
                    errors.push(text);
                }
                NixLogLevel::Warn => {
                    let msg = format!("⚠ {}", text);
                    ctx.emit_progress(&msg);
                    messages.push(msg);
                }
                _ => {
                    ctx.emit_progress(&text);
                    messages.push(text);
                }
            }
        }
        _ => {}
    }
}

/// Format the final result for the LLM
fn format_nix_result(
    subcommand: &str,
    exit_code: i32,
    stdout_lines: &[String],
    build_log_lines: &[String],
    messages: &[String],
    errors: &[String],
) -> String {
    let mut parts = Vec::new();

    if exit_code != 0 {
        parts.push(format!("nix {} failed (exit code {})", subcommand, exit_code));
    }

    // Stdout (command output)
    if !stdout_lines.is_empty() {
        parts.push(stdout_lines.join("\n"));
    }

    // Build log (if any)
    if !build_log_lines.is_empty() {
        parts.push(format!("Build log:\n{}", build_log_lines.join("\n")));
    }

    // Errors
    if !errors.is_empty() {
        parts.push(format!("Errors:\n{}", errors.join("\n")));
    }

    // Info messages (only if there's no other output)
    if stdout_lines.is_empty() && build_log_lines.is_empty() && errors.is_empty() && !messages.is_empty() {
        parts.push(messages.join("\n"));
    }

    if parts.is_empty() {
        if exit_code == 0 {
            format!("nix {} completed successfully", subcommand)
        } else {
            format!("nix {} failed with exit code {}", subcommand, exit_code)
        }
    } else {
        parts.join("\n\n")
    }
}

/// Shorten a nix store path for display
/// "/nix/store/abc123-foo-1.0" -> "foo-1.0"
fn shorten_store_path(path: &str) -> String {
    if let Some(rest) = path.strip_prefix("/nix/store/") {
        // Skip the 32-char hash + dash
        if rest.len() > 33 {
            return rest[33..].to_string();
        }
    }
    // Try extracting from longer text like "copying '/nix/store/...'"
    if let Some(start) = path.find("/nix/store/") {
        let from = start + "/nix/store/".len();
        let rest = &path[from..];
        // Find end of path (quote, space, or end)
        let end = rest.find(['\'', '"', ' ']).unwrap_or(rest.len());
        let store_suffix = &rest[..end];
        if store_suffix.len() > 33 {
            return store_suffix[33..].to_string();
        }
    }
    path.to_string()
}

/// Shorten a derivation path for display
/// "building '/nix/store/abc...-foo.drv'" -> "foo"
fn shorten_drv_path(text: &str) -> String {
    if let Some(start) = text.find("/nix/store/") {
        let from = start + "/nix/store/".len();
        let rest = &text[from..];
        let end = rest.find(['\'', '"', ' ']).unwrap_or(rest.len());
        let name = &rest[..end];
        // Strip hash prefix
        if name.len() > 33 {
            let short = &name[33..];
            // Strip .drv extension
            return short.strip_suffix(".drv").unwrap_or(short).to_string();
        }
    }
    text.to_string()
}

/// Shorten a URL for display
fn shorten_url(url: &str) -> String {
    // For github URLs, show just the relevant part
    if let Some(rest) = url.strip_prefix("https://github.com/")
        && rest.len() > 60 {
            return format!("github:{}", &rest[..57].rsplit_once('/').map(|(l, _)| l).unwrap_or(&rest[..57]));
        }
    // Trim long URLs
    if url.len() > 80 {
        format!("{}...", &url[..77])
    } else {
        url.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shorten_store_path_strips_hash() {
        assert_eq!(shorten_store_path("/nix/store/aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa1-hello-2.12"), "hello-2.12");
    }

    #[test]
    fn shorten_store_path_from_text() {
        assert_eq!(
            shorten_store_path("copying '/nix/store/aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa1-glibc-2.39'"),
            "glibc-2.39"
        );
    }

    #[test]
    fn shorten_store_path_passthrough() {
        assert_eq!(shorten_store_path("something else"), "something else");
    }

    #[test]
    fn shorten_drv_path_extracts_name() {
        assert_eq!(
            shorten_drv_path("building '/nix/store/aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa1-hello-2.12.drv'"),
            "hello-2.12"
        );
    }

    #[test]
    fn shorten_url_github() {
        let url =
            "https://github.com/NixOS/nixpkgs/archive/abc123def456abc123def456abc123def456abc123def456abc123.tar.gz";
        let short = shorten_url(url);
        assert!(short.len() < url.len());
        assert!(short.starts_with("github:"));
    }

    #[test]
    fn shorten_url_passthrough_short() {
        assert_eq!(shorten_url("https://example.com/short"), "https://example.com/short");
    }

    #[test]
    fn activity_type_labels() {
        assert_eq!(ActivityType::Build.label(), "building");
        assert_eq!(ActivityType::FileTransfer.label(), "downloading");
        assert_eq!(ActivityType::Substitute.label(), "fetching");
    }

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

    #[test]
    fn format_result_success_empty() {
        let result = format_nix_result("build", 0, &[], &[], &[], &[]);
        assert_eq!(result, "nix build completed successfully");
    }

    #[test]
    fn format_result_with_stdout() {
        let result = format_nix_result("build", 0, &["/nix/store/abc-hello".into()], &[], &[], &[]);
        assert_eq!(result, "/nix/store/abc-hello");
    }

    #[test]
    fn format_result_failure_with_errors() {
        let result = format_nix_result("build", 1, &[], &["make: error".into()], &[], &["builder failed".into()]);
        assert!(result.contains("failed (exit code 1)"));
        assert!(result.contains("Build log:"));
        assert!(result.contains("make: error"));
        assert!(result.contains("Errors:"));
        assert!(result.contains("builder failed"));
    }

    #[test]
    fn parse_nix_start_event() {
        let json = r#"{"action":"start","fields":["/nix/store/abc-test.drv","",1,1],"id":123,"level":3,"parent":0,"text":"building '/nix/store/abc-test.drv'","type":105}"#;
        let event: NixEvent = serde_json::from_str(json).unwrap();
        assert_eq!(event.action, "start");
        assert_eq!(event.id, 123);
        assert_eq!(ActivityType::from_u64(event.activity_type), ActivityType::Build);
    }

    #[test]
    fn parse_nix_msg_event() {
        let json = r#"{"action":"msg","level":0,"msg":"error: build failed","raw_msg":"build failed"}"#;
        let event: NixEvent = serde_json::from_str(json).unwrap();
        assert_eq!(event.action, "msg");
        assert_eq!(NixLogLevel::from_u64(event.level), NixLogLevel::Error);
        assert_eq!(event.raw_msg, "build failed");
    }

    #[test]
    fn parse_nix_result_build_log() {
        let json = r#"{"action":"result","fields":["compiling main.rs"],"id":123,"type":101}"#;
        let event: NixEvent = serde_json::from_str(json).unwrap();
        assert_eq!(event.action, "result");
        assert_eq!(ResultType::from_u64(event.activity_type), Some(ResultType::BuildLogLine));
        assert_eq!(event.fields[0].as_str().unwrap(), "compiling main.rs");
    }
}
