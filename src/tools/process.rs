//! Agent-visible background process management.
//!
//! This complements the foreground `bash` tool by keeping long-running child
//! processes alive behind stable session IDs. Agents can poll incremental
//! output, inspect logs, wait, send stdin, and terminate processes.

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::LazyLock;
use std::time::Duration;
use std::time::Instant;

use async_trait::async_trait;
use serde_json::Value;
use serde_json::json;
use tokio::io::AsyncBufReadExt;
use tokio::io::AsyncWriteExt;
use tokio::io::BufReader;
use tokio::process::ChildStdin;
use tokio::process::Command;
use tokio::sync::oneshot;

use super::Tool;
use super::ToolContext;
use super::ToolDefinition;
use super::ToolResult;
use crate::util::ansi::strip_ansi;

const DEFAULT_LOG_LIMIT: usize = 200;
const MAX_COMMAND_PREVIEW_LEN: usize = 200;

static REGISTRY: LazyLock<std::sync::Mutex<ProcessRegistry>> =
    LazyLock::new(|| std::sync::Mutex::new(ProcessRegistry::default()));

#[derive(Default)]
struct ProcessRegistry {
    next_id: u64,
    entries: HashMap<String, Arc<ProcessEntry>>,
}

#[derive(Clone, Debug)]
enum ProcessStatus {
    Running,
    Exited { code: Option<i32>, elapsed: Duration },
    Killed { elapsed: Duration },
    Failed { message: String, elapsed: Duration },
}

impl ProcessStatus {
    fn is_done(&self) -> bool {
        !matches!(self, Self::Running)
    }

    fn label(&self) -> String {
        match self {
            Self::Running => "running".to_string(),
            Self::Exited { code, elapsed } => {
                format!(
                    "exited({})@{}",
                    code.map(|c| c.to_string()).unwrap_or_else(|| "signal".to_string()),
                    format_duration(*elapsed)
                )
            }
            Self::Killed { elapsed } => format!("killed@{}", format_duration(*elapsed)),
            Self::Failed { message, elapsed } => format!("failed@{}({message})", format_duration(*elapsed)),
        }
    }
}

struct ProcessEntry {
    id: String,
    command: String,
    started_at: Instant,
    output: std::sync::Mutex<Vec<String>>,
    poll_cursor: std::sync::Mutex<usize>,
    status: std::sync::Mutex<ProcessStatus>,
    stdin: tokio::sync::Mutex<Option<ChildStdin>>,
    kill_tx: std::sync::Mutex<Option<oneshot::Sender<()>>>,
}

impl ProcessEntry {
    fn new(id: String, command: String, stdin: Option<ChildStdin>, kill_tx: oneshot::Sender<()>) -> Self {
        Self {
            id,
            command,
            started_at: Instant::now(),
            output: std::sync::Mutex::new(Vec::new()),
            poll_cursor: std::sync::Mutex::new(0),
            status: std::sync::Mutex::new(ProcessStatus::Running),
            stdin: tokio::sync::Mutex::new(stdin),
            kill_tx: std::sync::Mutex::new(Some(kill_tx)),
        }
    }

    fn push_output(&self, stream: &str, raw: &str) {
        let line = strip_ansi(raw);
        let mut output = self.output.lock().expect("process output lock poisoned");
        output.push(format!("[{stream}] {line}"));
    }

    fn set_status(&self, status: ProcessStatus) {
        let mut current = self.status.lock().expect("process status lock poisoned");
        *current = status;
    }

    fn status(&self) -> ProcessStatus {
        self.status.lock().expect("process status lock poisoned").clone()
    }

    fn snapshot_output(&self) -> Vec<String> {
        self.output.lock().expect("process output lock poisoned").clone()
    }

    fn drain_new_output(&self) -> Vec<String> {
        let output = self.output.lock().expect("process output lock poisoned");
        let mut cursor = self.poll_cursor.lock().expect("process poll cursor lock poisoned");
        let new = output.get(*cursor..).unwrap_or(&[]).to_vec();
        *cursor = output.len();
        new
    }

    fn elapsed(&self) -> Duration {
        self.started_at.elapsed()
    }
}

pub struct ProcessTool {
    definition: ToolDefinition,
    process_monitor: Option<crate::procmon::ProcessMonitorHandle>,
}

impl ProcessTool {
    pub fn new() -> Self {
        Self {
            definition: ToolDefinition {
                name: "process".to_string(),
                description: concat!(
                    "Manage background processes by session ID. Use for servers, watchers, ",
                    "long-running tests/builds, and commands that need stdin. Actions: start, list, ",
                    "poll, log, wait, kill, write, submit, close. Start with either `command` ",
                    "(shell mode) or `program` + `args` (direct exec mode). Prefer this over shell-level &, ",
                    "nohup, disown, or foreground bash for long-lived processes."
                )
                .to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "action": {
                            "type": "string",
                            "enum": ["start", "list", "poll", "log", "wait", "kill", "write", "submit", "close"],
                            "description": "Action to perform"
                        },
                        "command": {
                            "type": "string",
                            "description": "Shell command to start in bash -c mode (start requires command or program)"
                        },
                        "program": {
                            "type": "string",
                            "description": "Executable to start directly without a shell (start requires command or program)"
                        },
                        "args": {
                            "type": "array",
                            "items": { "type": "string" },
                            "description": "Arguments for direct exec mode"
                        },
                        "session_id": {
                            "type": "string",
                            "description": "Background process session ID"
                        },
                        "timeout": {
                            "type": "number",
                            "description": "Seconds to wait for wait action (default: 30)"
                        },
                        "offset": {
                            "type": "number",
                            "description": "Line offset for log action (default: last limit lines)"
                        },
                        "limit": {
                            "type": "number",
                            "description": "Maximum log lines to return (default: 200)"
                        },
                        "data": {
                            "type": "string",
                            "description": "Data to send to stdin for write/submit actions"
                        }
                    },
                    "required": ["action"]
                }),
            },
            process_monitor: None,
        }
    }

    pub fn with_process_monitor(mut self, monitor: crate::procmon::ProcessMonitorHandle) -> Self {
        self.process_monitor = Some(monitor);
        self
    }

    fn next_id() -> String {
        let mut registry = REGISTRY.lock().expect("process registry lock poisoned");
        registry.next_id += 1;
        format!("proc_{}", registry.next_id)
    }

    fn insert(entry: Arc<ProcessEntry>) {
        let mut registry = REGISTRY.lock().expect("process registry lock poisoned");
        registry.entries.insert(entry.id.clone(), entry);
    }

    fn get(session_id: &str) -> Option<Arc<ProcessEntry>> {
        let registry = REGISTRY.lock().expect("process registry lock poisoned");
        registry.entries.get(session_id).cloned()
    }

    fn all_entries() -> Vec<Arc<ProcessEntry>> {
        let registry = REGISTRY.lock().expect("process registry lock poisoned");
        registry.entries.values().cloned().collect()
    }

    fn required_session(params: &Value) -> Result<String, ToolResult> {
        params
            .get("session_id")
            .and_then(|v| v.as_str())
            .map(str::to_string)
            .filter(|s| !s.is_empty())
            .ok_or_else(|| ToolResult::error("Missing required parameter: session_id"))
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

    fn configure_child(cmd: &mut Command) {
        cmd.env_clear()
            .envs(crate::tools::sandbox::sanitized_env())
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped());

        #[cfg(target_os = "linux")]
        {
            let cwd_for_landlock = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
            unsafe {
                cmd.pre_exec(move || {
                    // Put the process and all descendants into a dedicated process
                    // group so `process.kill` can clean up servers/watchers that
                    // spawn child processes instead of killing only the launcher.
                    if libc::setpgid(0, 0) != 0 {
                        return Err(std::io::Error::last_os_error());
                    }
                    if let Err(e) = crate::tools::sandbox::apply_landlock_to_current(&cwd_for_landlock) {
                        tracing::warn!("sandbox: landlock on background process child failed: {}", e);
                    }
                    Ok(())
                });
            }
        }
    }

    fn spawn_shell_command(command: &str) -> Result<tokio::process::Child, ToolResult> {
        let mut cmd = Command::new("bash");
        cmd.arg("-c").arg(command);
        Self::configure_child(&mut cmd);
        cmd.spawn().map_err(|e| ToolResult::error(format!("Failed to spawn shell background process: {e}")))
    }

    fn spawn_direct(program: &str, args: &[String]) -> Result<tokio::process::Child, ToolResult> {
        let mut cmd = Command::new(program);
        cmd.args(args);
        Self::configure_child(&mut cmd);
        cmd.spawn()
            .map_err(|e| ToolResult::error(format!("Failed to spawn direct background process: {e}")))
    }

    fn start_spec(params: &Value) -> Result<(String, tokio::process::Child), ToolResult> {
        let command = params.get("command").and_then(|v| v.as_str()).filter(|s| !s.trim().is_empty());
        let program = params.get("program").and_then(|v| v.as_str()).filter(|s| !s.trim().is_empty());
        match (command, program) {
            (Some(_), Some(_)) => Err(ToolResult::error("Provide either 'command' or 'program', not both.")),
            (Some(command), None) => {
                if let Some(reason) = crate::tools::bash::check_dangerous(command) {
                    return Err(ToolResult::error(format!(
                        "Dangerous command blocked ({reason}): {command}\nUse foreground bash with interactive confirmation or ask the user for guidance."
                    )));
                }
                let child = Self::spawn_shell_command(command)?;
                Ok((command.to_string(), child))
            }
            (None, Some(program)) => {
                let args = Self::parse_args(params)?;
                let child = Self::spawn_direct(program, &args)?;
                Ok((format_direct_command(program, &args), child))
            }
            (None, None) => Err(ToolResult::error("Missing required parameter: command or program")),
        }
    }

    fn handle_start(&self, ctx: &ToolContext, params: &Value) -> ToolResult {
        let (display_command, mut child) = match Self::start_spec(params) {
            Ok(spec) => spec,
            Err(result) => return result,
        };
        let pid = child.id();
        let stdin = child.stdin.take();
        let stdout = match child.stdout.take() {
            Some(stdout) => stdout,
            None => return ToolResult::error("Failed to capture stdout from background process"),
        };
        let stderr = match child.stderr.take() {
            Some(stderr) => stderr,
            None => return ToolResult::error("Failed to capture stderr from background process"),
        };
        let (kill_tx, kill_rx) = oneshot::channel();
        let id = Self::next_id();
        let entry = Arc::new(ProcessEntry::new(id.clone(), display_command.clone(), stdin, kill_tx));
        Self::insert(entry.clone());

        if let Some(ref monitor) = self.process_monitor
            && let Some(pid) = pid
        {
            let command_preview: String = display_command.chars().take(MAX_COMMAND_PREVIEW_LEN).collect();
            monitor.register(pid, crate::procmon::ProcessMeta {
                tool_name: "process".to_string(),
                command: command_preview,
                call_id: ctx.call_id.clone(),
            });
        }

        spawn_reader(entry.clone(), "stdout", stdout);
        spawn_reader(entry.clone(), "stderr", stderr);
        spawn_waiter(entry.clone(), child, pid, kill_rx);

        ToolResult::text(format!(
            "Started background process {id} (pid: {})",
            pid.map(|p| p.to_string()).unwrap_or_else(|| "unknown".to_string())
        ))
    }

    fn handle_list() -> ToolResult {
        let mut entries = Self::all_entries();
        entries.sort_by_key(|entry| entry.id.clone());
        if entries.is_empty() {
            return ToolResult::text("No background processes.");
        }

        let mut lines = vec![format!("{:<12} {:<16} {:<8} {}", "SESSION", "STATUS", "AGE", "COMMAND")];
        lines.push("─".repeat(80));
        for entry in entries {
            let command_preview: String = entry.command.chars().take(MAX_COMMAND_PREVIEW_LEN).collect();
            lines.push(format!(
                "{:<12} {:<16} {:<8} {}",
                entry.id,
                entry.status().label(),
                format_duration(entry.elapsed()),
                command_preview
            ));
        }
        ToolResult::text(lines.join("\n"))
    }

    fn handle_poll(params: &Value) -> ToolResult {
        let session_id = match Self::required_session(params) {
            Ok(id) => id,
            Err(result) => return result,
        };
        let entry = match Self::get(&session_id) {
            Some(entry) => entry,
            None => return ToolResult::error(format!("Unknown process session_id: {session_id}")),
        };
        let output = entry.drain_new_output();
        let mut text = format!("{} status: {}\n", entry.id, entry.status().label());
        if output.is_empty() {
            text.push_str("No new output.");
        } else {
            text.push_str(&output.join("\n"));
        }
        ToolResult::text(text)
    }

    fn handle_log(params: &Value) -> ToolResult {
        let session_id = match Self::required_session(params) {
            Ok(id) => id,
            Err(result) => return result,
        };
        let entry = match Self::get(&session_id) {
            Some(entry) => entry,
            None => return ToolResult::error(format!("Unknown process session_id: {session_id}")),
        };
        let output = entry.snapshot_output();
        let limit = params
            .get("limit")
            .and_then(|v| v.as_u64())
            .and_then(|v| usize::try_from(v).ok())
            .unwrap_or(DEFAULT_LOG_LIMIT);
        let start = params
            .get("offset")
            .and_then(|v| v.as_u64())
            .and_then(|v| usize::try_from(v).ok())
            .unwrap_or_else(|| output.len().saturating_sub(limit));
        let end = output.len().min(start.saturating_add(limit));
        let lines = output.get(start..end).unwrap_or(&[]);
        if lines.is_empty() {
            ToolResult::text(format!("{} log is empty (status: {}).", entry.id, entry.status().label()))
        } else {
            ToolResult::text(format!(
                "{} log lines {}..{} of {} (status: {})\n{}",
                entry.id,
                start,
                end,
                output.len(),
                entry.status().label(),
                lines.join("\n")
            ))
        }
    }

    async fn handle_wait(params: &Value) -> ToolResult {
        let session_id = match Self::required_session(params) {
            Ok(id) => id,
            Err(result) => return result,
        };
        let entry = match Self::get(&session_id) {
            Some(entry) => entry,
            None => return ToolResult::error(format!("Unknown process session_id: {session_id}")),
        };
        let timeout_secs = params.get("timeout").and_then(|v| v.as_u64()).unwrap_or(30);
        let deadline = Instant::now() + Duration::from_secs(timeout_secs);
        while !entry.status().is_done() {
            if timeout_secs > 0 && Instant::now() >= deadline {
                return ToolResult::text(format!("{} still running after {}s", entry.id, timeout_secs));
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        let output = entry.drain_new_output();
        let mut text = format!("{} finished with status: {}", entry.id, entry.status().label());
        if !output.is_empty() {
            text.push('\n');
            text.push_str(&output.join("\n"));
        }
        ToolResult::text(text)
    }

    fn handle_kill(params: &Value) -> ToolResult {
        let session_id = match Self::required_session(params) {
            Ok(id) => id,
            Err(result) => return result,
        };
        let entry = match Self::get(&session_id) {
            Some(entry) => entry,
            None => return ToolResult::error(format!("Unknown process session_id: {session_id}")),
        };
        if entry.status().is_done() {
            return ToolResult::text(format!("{} is already {}", entry.id, entry.status().label()));
        }
        let tx = entry.kill_tx.lock().expect("process kill lock poisoned").take();
        match tx {
            Some(tx) => {
                tx.send(()).ok();
                ToolResult::text(format!("Kill requested for {}", entry.id))
            }
            None => ToolResult::text(format!("Kill already requested for {}", entry.id)),
        }
    }

    async fn handle_write(params: &Value, newline: bool) -> ToolResult {
        let session_id = match Self::required_session(params) {
            Ok(id) => id,
            Err(result) => return result,
        };
        let data = params.get("data").and_then(|v| v.as_str()).unwrap_or("");
        let entry = match Self::get(&session_id) {
            Some(entry) => entry,
            None => return ToolResult::error(format!("Unknown process session_id: {session_id}")),
        };
        if entry.status().is_done() {
            return ToolResult::error(format!("{} is not running ({})", entry.id, entry.status().label()));
        }
        let mut stdin = entry.stdin.lock().await;
        let Some(stdin) = stdin.as_mut() else {
            return ToolResult::error(format!("{} has no open stdin", entry.id));
        };
        if let Err(e) = stdin.write_all(data.as_bytes()).await {
            return ToolResult::error(format!("Failed to write stdin for {}: {e}", entry.id));
        }
        if newline && let Err(e) = stdin.write_all(b"\n").await {
            return ToolResult::error(format!("Failed to write newline for {}: {e}", entry.id));
        }
        if let Err(e) = stdin.flush().await {
            return ToolResult::error(format!("Failed to flush stdin for {}: {e}", entry.id));
        }
        ToolResult::text(format!("Wrote {} bytes to {}", data.len() + usize::from(newline), entry.id))
    }

    async fn handle_close(params: &Value) -> ToolResult {
        let session_id = match Self::required_session(params) {
            Ok(id) => id,
            Err(result) => return result,
        };
        let entry = match Self::get(&session_id) {
            Some(entry) => entry,
            None => return ToolResult::error(format!("Unknown process session_id: {session_id}")),
        };
        let mut stdin = entry.stdin.lock().await;
        if stdin.take().is_some() {
            ToolResult::text(format!("Closed stdin for {}", entry.id))
        } else {
            ToolResult::text(format!("Stdin already closed for {}", entry.id))
        }
    }
}

impl Default for ProcessTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for ProcessTool {
    fn definition(&self) -> &ToolDefinition {
        &self.definition
    }

    async fn execute(&self, ctx: &ToolContext, params: Value) -> ToolResult {
        let action = match params.get("action").and_then(|v| v.as_str()) {
            Some(action) => action,
            None => return ToolResult::error("Missing required parameter: action"),
        };

        match action {
            "start" => self.handle_start(ctx, &params),
            "list" => Self::handle_list(),
            "poll" => Self::handle_poll(&params),
            "log" => Self::handle_log(&params),
            "wait" => Self::handle_wait(&params).await,
            "kill" => Self::handle_kill(&params),
            "write" => Self::handle_write(&params, false).await,
            "submit" => Self::handle_write(&params, true).await,
            "close" => Self::handle_close(&params).await,
            other => ToolResult::error(format!("Unknown process action: {other}")),
        }
    }
}

fn spawn_reader<R>(entry: Arc<ProcessEntry>, stream: &'static str, reader: R)
where R: tokio::io::AsyncRead + Unpin + Send + 'static {
    tokio::spawn(async move {
        let mut lines = BufReader::new(reader).lines();
        while let Ok(Some(line)) = lines.next_line().await {
            entry.push_output(stream, &line);
        }
    });
}

fn spawn_waiter(
    entry: Arc<ProcessEntry>,
    mut child: tokio::process::Child,
    pid: Option<u32>,
    mut kill_rx: oneshot::Receiver<()>,
) {
    tokio::spawn(async move {
        let started_at = entry.started_at;
        tokio::select! {
            status = child.wait() => {
                let elapsed = started_at.elapsed();
                match status {
                    Ok(status) => entry.set_status(ProcessStatus::Exited { code: status.code(), elapsed }),
                    Err(e) => entry.set_status(ProcessStatus::Failed { message: e.to_string(), elapsed }),
                }
            }
            _ = &mut kill_rx => {
                terminate_process_group(pid, &mut child).await;
                entry.set_status(ProcessStatus::Killed { elapsed: started_at.elapsed() });
            }
        }
    });
}

async fn terminate_process_group(pid: Option<u32>, child: &mut tokio::process::Child) {
    #[cfg(unix)]
    if let Some(pid) = pid.and_then(|pid| i32::try_from(pid).ok()) {
        // Negative PID targets the process group whose ID is `pid`.
        unsafe {
            libc::kill(-pid, libc::SIGTERM);
        }
        if tokio::time::timeout(Duration::from_secs(2), child.wait()).await.is_ok() {
            return;
        }
        unsafe {
            libc::kill(-pid, libc::SIGKILL);
        }
        let _ = child.wait().await;
        return;
    }

    child.start_kill().ok();
    let _ = child.wait().await;
}

fn format_direct_command(program: &str, args: &[String]) -> String {
    std::iter::once(program.to_string())
        .chain(args.iter().map(|arg| shell_display_quote(arg)))
        .collect::<Vec<_>>()
        .join(" ")
}

fn shell_display_quote(value: &str) -> String {
    if value.chars().all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-' | '.' | '/' | ':')) {
        value.to_string()
    } else {
        format!("'{}'", value.replace('\'', "'\\''"))
    }
}

fn format_duration(duration: Duration) -> String {
    let total = duration.as_secs();
    let minutes = total / 60;
    let seconds = total % 60;
    format!("{minutes}:{seconds:02}")
}

#[cfg(test)]
mod tests {
    use serde_json::json;
    use tokio_util::sync::CancellationToken;

    use super::*;

    fn make_ctx() -> ToolContext {
        ToolContext::new("process-test".to_string(), CancellationToken::new(), None)
    }

    fn text(result: &ToolResult) -> String {
        result
            .content
            .iter()
            .filter_map(|content| match content {
                super::super::ToolResultContent::Text { text } => Some(text.as_str()),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    fn extract_process_id(result: &ToolResult) -> String {
        let text = text(result);
        text.split_whitespace()
            .find(|word| word.starts_with("proc_"))
            .expect("result contains process id")
            .to_string()
    }

    #[tokio::test]
    async fn starts_and_waits_for_process() {
        let tool = ProcessTool::new();
        let started = tool.execute(&make_ctx(), json!({"action": "start", "command": "printf hello"})).await;
        assert!(!started.is_error, "{started:?}");
        let id = extract_process_id(&started);
        let waited = tool.execute(&make_ctx(), json!({"action": "wait", "session_id": id, "timeout": 2})).await;
        assert!(!waited.is_error, "{waited:?}");
        assert!(text(&waited).contains("hello"), "{}", text(&waited));
    }

    #[tokio::test]
    async fn starts_direct_program_with_args() {
        let tool = ProcessTool::new();
        let started = tool
            .execute(&make_ctx(), json!({"action": "start", "program": "printf", "args": ["direct:%s", "ok"]}))
            .await;
        assert!(!started.is_error, "{started:?}");
        let id = extract_process_id(&started);
        let waited = tool.execute(&make_ctx(), json!({"action": "wait", "session_id": id, "timeout": 2})).await;
        assert!(!waited.is_error, "{waited:?}");
        assert!(text(&waited).contains("direct:ok"), "{}", text(&waited));
    }

    #[tokio::test]
    async fn start_rejects_command_and_program_together() {
        let tool = ProcessTool::new();
        let result = tool
            .execute(
                &make_ctx(),
                json!({"action": "start", "command": "printf shell", "program": "printf", "args": ["direct"]}),
            )
            .await;
        assert!(result.is_error);
        assert!(text(&result).contains("either 'command' or 'program'"), "{}", text(&result));
    }

    #[tokio::test]
    async fn direct_args_must_be_strings() {
        let tool = ProcessTool::new();
        let result = tool.execute(&make_ctx(), json!({"action": "start", "program": "printf", "args": [1]})).await;
        assert!(result.is_error);
        assert!(text(&result).contains("array of strings"), "{}", text(&result));
    }

    #[tokio::test]
    async fn submit_writes_line_to_stdin() {
        let tool = ProcessTool::new();
        let started =
            tool.execute(&make_ctx(), json!({"action": "start", "command": "read line; echo got:$line"})).await;
        let id = extract_process_id(&started);
        let submitted = tool.execute(&make_ctx(), json!({"action": "submit", "session_id": id, "data": "ping"})).await;
        assert!(!submitted.is_error, "{submitted:?}");
        let waited = tool.execute(&make_ctx(), json!({"action": "wait", "session_id": id, "timeout": 2})).await;
        assert!(text(&waited).contains("got:ping"), "{}", text(&waited));
    }

    #[tokio::test]
    async fn poll_returns_incremental_output() {
        let tool = ProcessTool::new();
        let started = tool
            .execute(
                &make_ctx(),
                json!({"action": "start", "command": "printf first; sleep 0.1; printf '\\nsecond\\n'"}),
            )
            .await;
        let id = extract_process_id(&started);
        tokio::time::sleep(Duration::from_millis(250)).await;
        let first = tool.execute(&make_ctx(), json!({"action": "poll", "session_id": id})).await;
        assert!(text(&first).contains("first"), "{}", text(&first));
        let second = tool.execute(&make_ctx(), json!({"action": "poll", "session_id": id})).await;
        assert!(text(&second).contains("No new output"), "{}", text(&second));
    }

    #[tokio::test]
    async fn kill_stops_running_process() {
        let tool = ProcessTool::new();
        let started = tool.execute(&make_ctx(), json!({"action": "start", "command": "sleep 10"})).await;
        let id = extract_process_id(&started);
        let killed = tool.execute(&make_ctx(), json!({"action": "kill", "session_id": id})).await;
        assert!(!killed.is_error, "{killed:?}");
        let waited = tool.execute(&make_ctx(), json!({"action": "wait", "session_id": id, "timeout": 2})).await;
        assert!(text(&waited).contains("killed"), "{}", text(&waited));
    }

    #[tokio::test]
    async fn kill_stops_shell_descendants() {
        let tool = ProcessTool::new();
        let marker = std::env::temp_dir().join(format!("clankers-process-kill-{}", std::process::id()));
        std::fs::remove_file(&marker).ok();
        let command = format!("(trap 'exit 0' TERM; sleep 10; touch {}) & wait", marker.display());
        let started = tool.execute(&make_ctx(), json!({"action": "start", "command": command})).await;
        let id = extract_process_id(&started);
        let killed = tool.execute(&make_ctx(), json!({"action": "kill", "session_id": id})).await;
        assert!(!killed.is_error, "{killed:?}");
        let waited = tool.execute(&make_ctx(), json!({"action": "wait", "session_id": id, "timeout": 3})).await;
        assert!(text(&waited).contains("killed"), "{}", text(&waited));
        tokio::time::sleep(Duration::from_millis(200)).await;
        assert!(!marker.exists(), "child process survived process-group kill");
    }
}
