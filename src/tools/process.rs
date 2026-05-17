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
use chrono::DateTime;
use chrono::Utc;
use clankers_db::process_jobs::StoredProcessJobBackendKind;
use clankers_db::process_jobs::StoredProcessJobCapabilitySummary;
use clankers_db::process_jobs::StoredProcessJobCwd;
use clankers_db::process_jobs::StoredProcessJobLogRef;
use clankers_db::process_jobs::StoredProcessJobOwnerScope;
use clankers_db::process_jobs::StoredProcessJobRecord;
use clankers_db::process_jobs::StoredProcessJobResourcePolicy;
use clankers_db::process_jobs::StoredProcessJobStatus;
use clankers_db::process_jobs::StoredProcessJobStream;
use clankers_runtime::RuntimeError;
use clankers_runtime::process_jobs::BackendRef;
use clankers_runtime::process_jobs::ProcessJobBackendKind;
use clankers_runtime::process_jobs::ProcessJobFilter;
use clankers_runtime::process_jobs::ProcessJobId;
use clankers_runtime::process_jobs::ProcessJobLogChunk;
use clankers_runtime::process_jobs::ProcessJobLogCursor;
use clankers_runtime::process_jobs::ProcessJobLogRange;
use clankers_runtime::process_jobs::ProcessJobOperation;
use clankers_runtime::process_jobs::ProcessJobReceipt;
use clankers_runtime::process_jobs::ProcessJobService;
use clankers_runtime::process_jobs::ProcessJobStatus;
use clankers_runtime::process_jobs::ProcessJobSummary;
use clankers_runtime::process_jobs::StartProcessJobRequest;
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
    started_at_wall: DateTime<Utc>,
    backend_ref: Option<BackendRef>,
    output: std::sync::Mutex<Vec<String>>,
    poll_cursor: std::sync::Mutex<usize>,
    status: std::sync::Mutex<ProcessStatus>,
    stdin: tokio::sync::Mutex<Option<ChildStdin>>,
    kill_tx: std::sync::Mutex<Option<oneshot::Sender<()>>>,
}

impl ProcessEntry {
    fn new(
        id: String,
        command: String,
        stdin: Option<ChildStdin>,
        kill_tx: oneshot::Sender<()>,
        pid: Option<u32>,
    ) -> Self {
        Self {
            id,
            command,
            started_at: Instant::now(),
            started_at_wall: Utc::now(),
            backend_ref: pid.map(|pid| BackendRef(format!("pid:{pid}"))),
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

    fn summary(&self) -> ProcessJobSummary {
        ProcessJobSummary {
            id: ProcessJobId(self.id.clone()),
            backend: ProcessJobBackendKind::Native,
            backend_ref: self.backend_ref.clone(),
            owner: clankers_runtime::process_jobs::ProcessJobOwnerScope::DaemonGlobal,
            status: status_to_job_status(&self.status()),
            command_preview: self.command.chars().take(MAX_COMMAND_PREVIEW_LEN).collect(),
            cwd: clankers_runtime::process_jobs::ProcessJobCwd::Inherited,
            started_at: Some(self.started_at_wall),
            updated_at: Utc::now(),
            completed_at: self.status().is_done().then(Utc::now),
            log_refs: Vec::new(),
        }
    }
}

#[derive(Debug, Default, Clone, Copy)]
pub struct NativeProcessJobService;

#[async_trait]
impl ProcessJobService for NativeProcessJobService {
    async fn start(&self, request: StartProcessJobRequest) -> Result<ProcessJobReceipt, RuntimeError> {
        if request.backend != ProcessJobBackendKind::Native {
            return Ok(ProcessJobReceipt::unsupported(
                ProcessJobOperation::Start,
                None,
                request.backend,
                "start",
                "current process tool default service supports only native backend",
            ));
        }

        let (display_command, mut child) = spawn_from_start_request(&request)?;
        let pid = child.id();
        let stdin = child.stdin.take();
        let stdout = child.stdout.take().ok_or_else(|| {
            RuntimeError::InvalidTool("failed to capture stdout from native background process".to_string())
        })?;
        let stderr = child.stderr.take().ok_or_else(|| {
            RuntimeError::InvalidTool("failed to capture stderr from native background process".to_string())
        })?;
        let (kill_tx, kill_rx) = oneshot::channel();
        let id = ProcessTool::next_id();
        let entry = Arc::new(ProcessEntry::new(id.clone(), display_command, stdin, kill_tx, pid));
        let backend_ref = entry.backend_ref.clone();
        ProcessTool::insert(entry.clone());
        spawn_reader(entry.clone(), "stdout", stdout);
        spawn_reader(entry, "stderr", stderr);
        spawn_waiter(ProcessTool::get(&id).expect("inserted native process entry"), child, pid, kill_rx, None);

        Ok(ProcessJobReceipt {
            operation: ProcessJobOperation::Start,
            id: Some(ProcessJobId(id.clone())),
            backend: Some(ProcessJobBackendKind::Native),
            status: Some(ProcessJobStatus::Running),
            backend_ref,
            log_refs: Vec::new(),
            summary: format!(
                "Started background process {id} (pid: {})",
                pid.map(|p| p.to_string()).unwrap_or_else(|| "unknown".to_string())
            ),
            error: None,
        })
    }

    async fn list(&self, filter: ProcessJobFilter) -> Result<Vec<ProcessJobSummary>, RuntimeError> {
        let mut entries = ProcessTool::all_entries();
        entries.sort_by_key(|entry| entry.id.clone());
        let summaries = entries
            .into_iter()
            .map(|entry| entry.summary())
            .filter(|summary| filter.backend.is_none_or(|backend| backend == summary.backend))
            .filter(|summary| filter.include_terminal || !summary.status.is_terminal())
            .collect();
        Ok(summaries)
    }

    async fn poll(
        &self,
        id: ProcessJobId,
        _cursor: Option<ProcessJobLogCursor>,
    ) -> Result<ProcessJobReceipt, RuntimeError> {
        let entry = native_entry(&id)?;
        let output = entry.drain_new_output();
        Ok(ProcessJobReceipt {
            operation: ProcessJobOperation::Poll,
            id: Some(id),
            backend: Some(ProcessJobBackendKind::Native),
            status: Some(status_to_job_status(&entry.status())),
            backend_ref: entry.backend_ref.clone(),
            log_refs: Vec::new(),
            summary: if output.is_empty() {
                "No new output.".to_string()
            } else {
                output.join("\n")
            },
            error: None,
        })
    }

    async fn log(&self, id: ProcessJobId, range: ProcessJobLogRange) -> Result<ProcessJobLogChunk, RuntimeError> {
        let entry = native_entry(&id)?;
        let output = entry.snapshot_output();
        let start = range
            .offset
            .and_then(|offset| usize::try_from(offset).ok())
            .unwrap_or_else(|| output.len().saturating_sub(DEFAULT_LOG_LIMIT));
        let limit = usize::try_from(range.limit_bytes).unwrap_or(DEFAULT_LOG_LIMIT).min(DEFAULT_LOG_LIMIT);
        let end = output.len().min(start.saturating_add(limit));
        let text = output.get(start..end).unwrap_or(&[]).join("\n");
        Ok(ProcessJobLogChunk {
            id,
            backend: ProcessJobBackendKind::Native,
            stream: range.stream,
            cursor: ProcessJobLogCursor {
                stream: range.stream,
                offset: u64::try_from(start).unwrap_or(u64::MAX),
            },
            next_cursor: Some(ProcessJobLogCursor {
                stream: range.stream,
                offset: u64::try_from(end).unwrap_or(u64::MAX),
            }),
            text,
            truncated: end < output.len(),
        })
    }

    async fn wait(&self, id: ProcessJobId, timeout: Option<Duration>) -> Result<ProcessJobReceipt, RuntimeError> {
        let entry = native_entry(&id)?;
        let timeout = timeout.unwrap_or(Duration::from_secs(30));
        let deadline = Instant::now() + timeout;
        while !entry.status().is_done() {
            if !timeout.is_zero() && Instant::now() >= deadline {
                return Ok(native_receipt(ProcessJobOperation::Wait, &entry, format!("{} still running", entry.id)));
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        let output = entry.drain_new_output();
        let mut summary = format!("{} finished with status: {}", entry.id, entry.status().label());
        if !output.is_empty() {
            summary.push('\n');
            summary.push_str(&output.join("\n"));
        }
        Ok(native_receipt(ProcessJobOperation::Wait, &entry, summary))
    }

    async fn kill(&self, id: ProcessJobId) -> Result<ProcessJobReceipt, RuntimeError> {
        let entry = native_entry(&id)?;
        if entry.status().is_done() {
            return Ok(native_receipt(
                ProcessJobOperation::Kill,
                &entry,
                format!("{} is already {}", entry.id, entry.status().label()),
            ));
        }
        let tx = entry.kill_tx.lock().expect("process kill lock poisoned").take();
        let summary = if let Some(tx) = tx {
            tx.send(()).ok();
            format!("Kill requested for {}", entry.id)
        } else {
            format!("Kill already requested for {}", entry.id)
        };
        Ok(native_receipt(ProcessJobOperation::Kill, &entry, summary))
    }

    async fn restart(&self, id: ProcessJobId) -> Result<ProcessJobReceipt, RuntimeError> {
        Ok(ProcessJobReceipt::unsupported(
            ProcessJobOperation::Restart,
            Some(id),
            ProcessJobBackendKind::Native,
            "restart",
            "native process restart is not implemented yet",
        ))
    }

    async fn write_stdin(
        &self,
        id: ProcessJobId,
        data: Vec<u8>,
        newline: bool,
    ) -> Result<ProcessJobReceipt, RuntimeError> {
        let entry = native_entry(&id)?;
        if entry.status().is_done() {
            return Err(RuntimeError::InvalidTool(format!("{} is not running ({})", entry.id, entry.status().label())));
        }
        let mut stdin = entry.stdin.lock().await;
        let Some(stdin) = stdin.as_mut() else {
            return Err(RuntimeError::InvalidTool(format!("{} has no open stdin", entry.id)));
        };
        stdin.write_all(&data).await.map_err(|e| RuntimeError::InvalidTool(e.to_string()))?;
        if newline {
            stdin.write_all(b"\n").await.map_err(|e| RuntimeError::InvalidTool(e.to_string()))?;
        }
        stdin.flush().await.map_err(|e| RuntimeError::InvalidTool(e.to_string()))?;
        Ok(native_receipt(
            ProcessJobOperation::WriteStdin,
            &entry,
            format!("Wrote {} bytes to {}", data.len() + usize::from(newline), entry.id),
        ))
    }

    async fn close_stdin(&self, id: ProcessJobId) -> Result<ProcessJobReceipt, RuntimeError> {
        let entry = native_entry(&id)?;
        let mut stdin = entry.stdin.lock().await;
        let summary = if stdin.take().is_some() {
            format!("Closed stdin for {}", entry.id)
        } else {
            format!("Stdin already closed for {}", entry.id)
        };
        Ok(native_receipt(ProcessJobOperation::CloseStdin, &entry, summary))
    }

    async fn adopt(
        &self,
        _backend: ProcessJobBackendKind,
        backend_ref: BackendRef,
    ) -> Result<ProcessJobReceipt, RuntimeError> {
        Ok(ProcessJobReceipt::unsupported(
            ProcessJobOperation::Adopt,
            None,
            ProcessJobBackendKind::Native,
            "adopt",
            format!("native adoption is not implemented for {}", backend_ref.0),
        ))
    }

    async fn garbage_collect(&self, _filter: ProcessJobFilter) -> Result<ProcessJobReceipt, RuntimeError> {
        Ok(ProcessJobReceipt::unsupported(
            ProcessJobOperation::GarbageCollect,
            None,
            ProcessJobBackendKind::Native,
            "garbage_collect",
            "native process garbage collection is not implemented yet",
        ))
    }
}

fn native_entry(id: &ProcessJobId) -> Result<Arc<ProcessEntry>, RuntimeError> {
    ProcessTool::get(&id.0).ok_or_else(|| RuntimeError::InvalidTool(format!("Unknown process session_id: {}", id.0)))
}

fn native_receipt(
    operation: ProcessJobOperation,
    entry: &ProcessEntry,
    summary: impl Into<String>,
) -> ProcessJobReceipt {
    ProcessJobReceipt {
        operation,
        id: Some(ProcessJobId(entry.id.clone())),
        backend: Some(ProcessJobBackendKind::Native),
        status: Some(status_to_job_status(&entry.status())),
        backend_ref: entry.backend_ref.clone(),
        log_refs: Vec::new(),
        summary: summary.into(),
        error: None,
    }
}

fn status_to_job_status(status: &ProcessStatus) -> ProcessJobStatus {
    match status {
        ProcessStatus::Running => ProcessJobStatus::Running,
        ProcessStatus::Exited { code, .. } => {
            if code.unwrap_or_default() == 0 {
                ProcessJobStatus::Succeeded { exit_code: *code }
            } else {
                ProcessJobStatus::Failed {
                    exit_code: *code,
                    reason: "process exited non-zero".to_string(),
                }
            }
        }
        ProcessStatus::Killed { .. } => ProcessJobStatus::Killed,
        ProcessStatus::Failed { message, .. } => ProcessJobStatus::Failed {
            exit_code: None,
            reason: message.clone(),
        },
    }
}

fn spawn_from_start_request(request: &StartProcessJobRequest) -> Result<(String, tokio::process::Child), RuntimeError> {
    match (request.shell_command.as_deref(), request.program.as_deref()) {
        (Some(_), Some(_)) => {
            Err(RuntimeError::InvalidTool("provide either shell_command or program, not both".to_string()))
        }
        (Some(command), None) => {
            if let Some(reason) = crate::tools::bash::check_dangerous(command) {
                return Err(RuntimeError::InvalidTool(format!("dangerous command blocked ({reason}): {command}")));
            }
            ProcessTool::spawn_shell_command(command)
                .map(|child| (command.to_string(), child))
                .map_err(tool_error_to_runtime)
        }
        (None, Some(program)) => ProcessTool::spawn_direct(program, &request.args)
            .map(|child| (format_direct_command(program, &request.args), child))
            .map_err(tool_error_to_runtime),
        (None, None) => Err(RuntimeError::InvalidTool("missing command or program".to_string())),
    }
}

fn tool_error_to_runtime(error: ToolResult) -> RuntimeError {
    RuntimeError::InvalidTool(
        error
            .content
            .iter()
            .filter_map(|content| match content {
                super::ToolResultContent::Text { text } => Some(text.as_str()),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("\n"),
    )
}

fn stored_record_from_entry(entry: &ProcessEntry) -> StoredProcessJobRecord {
    StoredProcessJobRecord {
        schema_version: clankers_db::process_jobs::PROCESS_JOB_RECORD_SCHEMA_VERSION,
        id: entry.id.clone(),
        backend: StoredProcessJobBackendKind::Native,
        backend_ref: entry.backend_ref.as_ref().map(|backend_ref| backend_ref.0.clone()),
        command_preview: entry.command.chars().take(MAX_COMMAND_PREVIEW_LEN).collect(),
        cwd: StoredProcessJobCwd::Inherited,
        owner: StoredProcessJobOwnerScope::DaemonGlobal,
        status: stored_status_from_process(&entry.status()),
        started_at: entry.started_at_wall,
        updated_at: Utc::now(),
        completed_at: entry.status().is_done().then(Utc::now),
        os_pid: entry
            .backend_ref
            .as_ref()
            .and_then(|backend_ref| backend_ref.0.strip_prefix("pid:"))
            .and_then(|pid| pid.parse().ok()),
        process_group: entry
            .backend_ref
            .as_ref()
            .and_then(|backend_ref| backend_ref.0.strip_prefix("pid:"))
            .and_then(|pid| pid.parse().ok()),
        log_refs: vec![StoredProcessJobLogRef {
            stream: StoredProcessJobStream::Combined,
            reference: format!("native:{}/combined.log", entry.id),
            retained_until: None,
            max_bytes: Some(u64::try_from(entry.snapshot_output().len()).unwrap_or(u64::MAX)),
        }],
        resource_policy: StoredProcessJobResourcePolicy {
            timeout_seconds: None,
            memory_max_bytes: None,
            cpu_quota_percent: None,
            max_log_bytes: None,
        },
        capability_summary: StoredProcessJobCapabilitySummary {
            can_observe: true,
            can_read_logs: true,
            can_start: true,
            can_kill: true,
            can_restart: false,
            can_write_stdin: true,
            can_select_backend: false,
        },
        safe_metadata: Default::default(),
    }
}

fn stored_status_from_process(status: &ProcessStatus) -> StoredProcessJobStatus {
    match status {
        ProcessStatus::Running => StoredProcessJobStatus::Running,
        ProcessStatus::Exited { code, .. } => {
            if code.unwrap_or_default() == 0 {
                StoredProcessJobStatus::Succeeded { exit_code: *code }
            } else {
                StoredProcessJobStatus::Failed {
                    exit_code: *code,
                    reason: "process exited non-zero".to_string(),
                }
            }
        }
        ProcessStatus::Killed { .. } => StoredProcessJobStatus::Killed,
        ProcessStatus::Failed { message, .. } => StoredProcessJobStatus::Failed {
            exit_code: None,
            reason: message.clone(),
        },
    }
}

fn stored_status_label(status: &StoredProcessJobStatus) -> String {
    match status {
        StoredProcessJobStatus::Pending => "pending".to_string(),
        StoredProcessJobStatus::Running => "running".to_string(),
        StoredProcessJobStatus::Waiting => "waiting".to_string(),
        StoredProcessJobStatus::Succeeded { exit_code } => {
            format!("exited({})", exit_code.map(|code| code.to_string()).unwrap_or_else(|| "ok".to_string()))
        }
        StoredProcessJobStatus::Failed { exit_code, reason } => format!(
            "failed({}:{reason})",
            exit_code.map(|code| code.to_string()).unwrap_or_else(|| "unknown".to_string())
        ),
        StoredProcessJobStatus::Killed => "killed".to_string(),
        StoredProcessJobStatus::Cancelled => "cancelled".to_string(),
        StoredProcessJobStatus::LostAfterRestart => "lost-after-restart".to_string(),
        StoredProcessJobStatus::ReattachedLogIncomplete => "reattached-log-incomplete".to_string(),
        StoredProcessJobStatus::BackendUnavailable { reason } => format!("backend-unavailable({reason})"),
        StoredProcessJobStatus::Unknown { raw } => format!("unknown({raw})"),
    }
}

fn stored_record_line(record: &StoredProcessJobRecord) -> String {
    format!(
        "{:<12} {:<16} {:<8} {}",
        record.id,
        stored_status_label(&record.status),
        "durable",
        record.command_preview
    )
}

async fn persist_entry(db: Option<&clankers_db::Db>, entry: &ProcessEntry) {
    let Some(db) = db else {
        return;
    };
    if let Err(error) = db.async_process_jobs().upsert(stored_record_from_entry(entry)).await {
        tracing::warn!("failed to persist native process job metadata: {error}");
    }
}

async fn durable_record(db: Option<&clankers_db::Db>, id: &str) -> Option<StoredProcessJobRecord> {
    let db = db?;
    match db.async_process_jobs().get(id.to_string()).await {
        Ok(record) => record,
        Err(error) => {
            tracing::warn!("failed to read durable process job metadata: {error}");
            None
        }
    }
}

fn format_log_refs(refs: &[StoredProcessJobLogRef]) -> String {
    if refs.is_empty() {
        return "none".to_string();
    }
    refs.iter().map(|log_ref| log_ref.reference.as_str()).collect::<Vec<_>>().join(", ")
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

    async fn handle_start(&self, ctx: &ToolContext, params: &Value) -> ToolResult {
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
        let entry = Arc::new(ProcessEntry::new(id.clone(), display_command.clone(), stdin, kill_tx, pid));
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

        persist_entry(ctx.db(), &entry).await;
        spawn_reader(entry.clone(), "stdout", stdout);
        spawn_reader(entry.clone(), "stderr", stderr);
        spawn_waiter(entry.clone(), child, pid, kill_rx, ctx.db().cloned());

        ToolResult::text(format!(
            "Started background process {id} (pid: {})",
            pid.map(|p| p.to_string()).unwrap_or_else(|| "unknown".to_string())
        ))
    }

    async fn handle_list(ctx: &ToolContext) -> ToolResult {
        let mut entries = Self::all_entries();
        entries.sort_by_key(|entry| entry.id.clone());
        let mut durable = Vec::new();
        if let Some(db) = ctx.db() {
            match db.async_process_jobs().list().await {
                Ok(records) => {
                    let live_ids =
                        entries.iter().map(|entry| entry.id.as_str()).collect::<std::collections::BTreeSet<_>>();
                    durable = records.into_iter().filter(|record| !live_ids.contains(record.id.as_str())).collect();
                    durable.sort_by(|left, right| left.id.cmp(&right.id));
                }
                Err(error) => tracing::warn!("failed to read durable process job metadata: {error}"),
            }
        }
        if entries.is_empty() && durable.is_empty() {
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
        lines.extend(durable.iter().map(stored_record_line));
        ToolResult::text(lines.join("\n"))
    }

    async fn handle_poll(ctx: &ToolContext, params: &Value) -> ToolResult {
        let session_id = match Self::required_session(params) {
            Ok(id) => id,
            Err(result) => return result,
        };
        let entry = match Self::get(&session_id) {
            Some(entry) => entry,
            None => {
                if let Some(record) = durable_record(ctx.db(), &session_id).await {
                    return ToolResult::text(format!(
                        "{} status: {}\nNo live output stream; durable log refs: {}",
                        record.id,
                        stored_status_label(&record.status),
                        format_log_refs(&record.log_refs)
                    ));
                }
                return ToolResult::error(format!("Unknown process session_id: {session_id}"));
            }
        };
        persist_entry(ctx.db(), &entry).await;
        let output = entry.drain_new_output();
        let mut text = format!("{} status: {}\n", entry.id, entry.status().label());
        if output.is_empty() {
            text.push_str("No new output.");
        } else {
            text.push_str(&output.join("\n"));
        }
        ToolResult::text(text)
    }

    async fn handle_log(ctx: &ToolContext, params: &Value) -> ToolResult {
        let session_id = match Self::required_session(params) {
            Ok(id) => id,
            Err(result) => return result,
        };
        let entry = match Self::get(&session_id) {
            Some(entry) => entry,
            None => {
                if let Some(record) = durable_record(ctx.db(), &session_id).await {
                    return ToolResult::text(format!(
                        "{} live log stream is unavailable (durable status: {}, refs: {}).",
                        record.id,
                        stored_status_label(&record.status),
                        format_log_refs(&record.log_refs)
                    ));
                }
                return ToolResult::error(format!("Unknown process session_id: {session_id}"));
            }
        };
        persist_entry(ctx.db(), &entry).await;
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

    async fn handle_wait(ctx: &ToolContext, params: &Value) -> ToolResult {
        let session_id = match Self::required_session(params) {
            Ok(id) => id,
            Err(result) => return result,
        };
        let entry = match Self::get(&session_id) {
            Some(entry) => entry,
            None => {
                if let Some(record) = durable_record(ctx.db(), &session_id).await {
                    return ToolResult::text(format!(
                        "{} durable status: {} (live wait unavailable; refs: {})",
                        record.id,
                        stored_status_label(&record.status),
                        format_log_refs(&record.log_refs)
                    ));
                }
                return ToolResult::error(format!("Unknown process session_id: {session_id}"));
            }
        };
        let timeout_secs = params.get("timeout").and_then(|v| v.as_u64()).unwrap_or(30);
        let deadline = Instant::now() + Duration::from_secs(timeout_secs);
        while !entry.status().is_done() {
            if timeout_secs > 0 && Instant::now() >= deadline {
                persist_entry(ctx.db(), &entry).await;
                return ToolResult::text(format!("{} still running after {}s", entry.id, timeout_secs));
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        persist_entry(ctx.db(), &entry).await;
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
            "start" => self.handle_start(ctx, &params).await,
            "list" => Self::handle_list(ctx).await,
            "poll" => Self::handle_poll(ctx, &params).await,
            "log" => Self::handle_log(ctx, &params).await,
            "wait" => Self::handle_wait(ctx, &params).await,
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
    db: Option<clankers_db::Db>,
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
        persist_entry(db.as_ref(), &entry).await;
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

    fn make_ctx_with_db(db: clankers_db::Db) -> ToolContext {
        make_ctx().with_db(db)
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

    fn native_start_request(command: &str) -> StartProcessJobRequest {
        StartProcessJobRequest {
            backend: ProcessJobBackendKind::Native,
            command_preview: command.to_string(),
            program: None,
            args: Vec::new(),
            shell_command: Some(command.to_string()),
            cwd: clankers_runtime::process_jobs::ProcessJobCwd::Inherited,
            owner: clankers_runtime::process_jobs::ProcessJobOwnerScope::DaemonGlobal,
            resource_policy: clankers_runtime::process_jobs::ProcessJobResourcePolicy::default(),
            notification_policy: clankers_runtime::process_jobs::ProcessJobNotificationPolicy::default(),
            metadata: Default::default(),
        }
    }

    #[tokio::test]
    async fn native_process_job_service_preserves_default_start_list_wait_flow() {
        let service = NativeProcessJobService;
        let started = service.start(native_start_request("printf service-ok")).await.expect("start succeeds");
        assert_eq!(started.backend, Some(ProcessJobBackendKind::Native));
        let id = started.id.clone().expect("receipt has stable process id");

        let listed = service
            .list(ProcessJobFilter {
                include_terminal: true,
                ..ProcessJobFilter::default()
            })
            .await
            .expect("list succeeds");
        assert!(listed.iter().any(|summary| summary.id == id && summary.backend == ProcessJobBackendKind::Native));

        let waited = service.wait(id, Some(Duration::from_secs(2))).await.expect("wait succeeds");
        assert!(waited.summary.contains("service-ok"), "{}", waited.summary);
    }

    #[tokio::test]
    async fn process_tool_persists_native_metadata_and_uses_durable_fallback() {
        let db = clankers_db::Db::in_memory().expect("db opens");
        let ctx = make_ctx_with_db(db.clone());
        let tool = ProcessTool::new();
        let started = tool.execute(&ctx, json!({"action": "start", "command": "printf durable-ok"})).await;
        assert!(!started.is_error, "{started:?}");
        let id = extract_process_id(&started);
        let waited = tool.execute(&ctx, json!({"action": "wait", "session_id": id, "timeout": 2})).await;
        assert!(!waited.is_error, "{waited:?}");

        let stored = db.async_process_jobs().get(id.clone()).await.expect("db read").expect("record stored");
        assert_eq!(stored.id, id);
        assert!(matches!(stored.status, StoredProcessJobStatus::Succeeded { .. }));
        assert!(!stored.log_refs.is_empty());

        let mut durable_only = StoredProcessJobRecord::new_native(
            "proc_durable_only",
            "printf old",
            StoredProcessJobOwnerScope::DaemonGlobal,
        );
        durable_only.status = StoredProcessJobStatus::Succeeded { exit_code: Some(0) };
        durable_only.log_refs = vec![StoredProcessJobLogRef {
            stream: StoredProcessJobStream::Combined,
            reference: "native:proc_durable_only/combined.log".to_string(),
            retained_until: None,
            max_bytes: Some(10),
        }];
        db.async_process_jobs().upsert(durable_only).await.expect("insert durable-only record");

        let listed = tool.execute(&ctx, json!({"action": "list"})).await;
        assert!(text(&listed).contains("proc_durable_only"), "{}", text(&listed));
        let logged = tool.execute(&ctx, json!({"action": "log", "session_id": "proc_durable_only"})).await;
        assert!(text(&logged).contains("durable status"), "{}", text(&logged));
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
