//! Agent-visible background process management.
//!
//! This complements the foreground `bash` tool by keeping long-running child
//! processes alive behind stable session IDs. Agents can poll incremental
//! output, inspect logs, wait, send stdin, and terminate processes.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::LazyLock;
use std::sync::atomic::AtomicU64;
use std::sync::atomic::Ordering;
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
use clankers_runtime::process_jobs::AdoptProcessJobRequest;
use clankers_runtime::process_jobs::BackendRef;
use clankers_runtime::process_jobs::DefaultProcessJobNotificationPolicyEngine;
use clankers_runtime::process_jobs::ProcessJobBackendKind;
use clankers_runtime::process_jobs::ProcessJobCwd;
use clankers_runtime::process_jobs::ProcessJobError;
use clankers_runtime::process_jobs::ProcessJobErrorCode;
use clankers_runtime::process_jobs::ProcessJobEventId;
use clankers_runtime::process_jobs::ProcessJobFilter;
use clankers_runtime::process_jobs::ProcessJobGarbageCollectionFailure;
use clankers_runtime::process_jobs::ProcessJobGarbageCollectionReceipt;
use clankers_runtime::process_jobs::ProcessJobId;
use clankers_runtime::process_jobs::ProcessJobListProjection;
use clankers_runtime::process_jobs::ProcessJobLogChunk;
use clankers_runtime::process_jobs::ProcessJobLogCursor;
use clankers_runtime::process_jobs::ProcessJobLogRange;
use clankers_runtime::process_jobs::ProcessJobLogRef;
use clankers_runtime::process_jobs::ProcessJobNotificationDecision;
use clankers_runtime::process_jobs::ProcessJobNotificationEvent;
use clankers_runtime::process_jobs::ProcessJobNotificationKind;
use clankers_runtime::process_jobs::ProcessJobNotificationObservation;
use clankers_runtime::process_jobs::ProcessJobNotificationPolicy;
use clankers_runtime::process_jobs::ProcessJobNotificationPolicyEngine;
use clankers_runtime::process_jobs::ProcessJobNotificationPolicyState;
use clankers_runtime::process_jobs::ProcessJobOperation;
use clankers_runtime::process_jobs::ProcessJobOwnerScope;
use clankers_runtime::process_jobs::ProcessJobProjectionBounds;
use clankers_runtime::process_jobs::ProcessJobReceipt;
use clankers_runtime::process_jobs::ProcessJobReleasedLogRef;
use clankers_runtime::process_jobs::ProcessJobRetentionPolicy;
use clankers_runtime::process_jobs::ProcessJobService;
use clankers_runtime::process_jobs::ProcessJobStatus;
use clankers_runtime::process_jobs::ProcessJobStream;
use clankers_runtime::process_jobs::ProcessJobSummary;
use clankers_runtime::process_jobs::StartProcessJobRequest;
use clankers_runtime::process_jobs::project_process_job_list;
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
const MAX_NATIVE_ACTIVE_PROCESS_JOBS: usize = 32;
const NATIVE_KILL_GRACE: Duration = Duration::from_secs(2);
const ADOPTED_NATIVE_ID_PREFIX: &str = "native_pid_";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct NativeAdmissionDecision {
    accepted: bool,
    active: usize,
    limit: usize,
}

impl NativeAdmissionDecision {
    fn summary(&self) -> String {
        format!("native process admission denied: active process limit reached ({}/{})", self.active, self.limit)
    }
}

fn native_admission_decision(active: usize, limit: usize) -> NativeAdmissionDecision {
    NativeAdmissionDecision {
        accepted: active < limit,
        active,
        limit,
    }
}

static REGISTRY: LazyLock<std::sync::Mutex<ProcessRegistry>> =
    LazyLock::new(|| std::sync::Mutex::new(ProcessRegistry::default()));

#[derive(Default)]
struct ProcessRegistry {
    next_id: u64,
    entries: HashMap<String, Arc<ProcessEntry>>,
    reserved_starts: usize,
}

impl ProcessRegistry {
    fn active_or_reserved_count(&self) -> usize {
        self.entries.values().filter(|entry| !entry.status().is_done()).count() + self.reserved_starts
    }

    fn admission_decision(&self, limit: usize) -> NativeAdmissionDecision {
        native_admission_decision(self.active_or_reserved_count(), limit)
    }

    fn reserve_start(&mut self, limit: usize) -> Result<NativeAdmissionReservation, NativeAdmissionDecision> {
        let decision = self.admission_decision(limit);
        if !decision.accepted {
            return Err(decision);
        }
        self.reserved_starts += 1;
        Ok(NativeAdmissionReservation { released: false })
    }

    fn release_start_reservation(&mut self) {
        self.reserved_starts = self.reserved_starts.saturating_sub(1);
    }
}

struct NativeAdmissionReservation {
    released: bool,
}

impl NativeAdmissionReservation {
    fn release(mut self) {
        self.release_inner();
    }

    fn release_inner(&mut self) {
        if self.released {
            return;
        }
        REGISTRY.lock().expect("process registry lock poisoned").release_start_reservation();
        self.released = true;
    }
}

impl Drop for NativeAdmissionReservation {
    fn drop(&mut self) {
        self.release_inner();
    }
}

#[derive(Clone, Debug)]
enum ProcessStatus {
    Running,
    Exited {
        code: Option<i32>,
        elapsed: Duration,
    },
    Killed {
        elapsed: Duration,
        outcome: NativeTerminationOutcome,
    },
    Failed {
        message: String,
        elapsed: Duration,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum NativeTerminationOutcome {
    GracefulTerm,
    EscalatedKill,
    DirectKill,
}

impl NativeTerminationOutcome {
    fn label(self) -> &'static str {
        match self {
            Self::GracefulTerm => "graceful-term",
            Self::EscalatedKill => "escalated-sigkill",
            Self::DirectKill => "direct-kill",
        }
    }
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
            Self::Killed { elapsed, outcome } => format!("killed({})@{}", outcome.label(), format_duration(*elapsed)),
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
    notification_policy: ProcessJobNotificationPolicy,
    notification_state: tokio::sync::Mutex<ProcessJobNotificationPolicyState>,
    notifications: std::sync::Mutex<Vec<ProcessJobNotificationEvent>>,
    notification_cursor: std::sync::Mutex<usize>,
    next_notification_seq: AtomicU64,
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
        notification_policy: ProcessJobNotificationPolicy,
    ) -> Self {
        Self {
            id,
            command,
            started_at: Instant::now(),
            started_at_wall: Utc::now(),
            backend_ref: pid.map(|pid| BackendRef(format!("pid:{pid}"))),
            output: std::sync::Mutex::new(Vec::new()),
            poll_cursor: std::sync::Mutex::new(0),
            notification_policy,
            notification_state: tokio::sync::Mutex::new(ProcessJobNotificationPolicyState::default()),
            notifications: std::sync::Mutex::new(Vec::new()),
            notification_cursor: std::sync::Mutex::new(0),
            next_notification_seq: AtomicU64::new(0),
            status: std::sync::Mutex::new(ProcessStatus::Running),
            stdin: tokio::sync::Mutex::new(stdin),
            kill_tx: std::sync::Mutex::new(Some(kill_tx)),
        }
    }

    fn push_output(&self, stream: &str, raw: &str) -> String {
        let line = strip_ansi(raw);
        let mut output = self.output.lock().expect("process output lock poisoned");
        output.push(format!("[{stream}] {line}"));
        line
    }

    async fn evaluate_output_notification(&self, line: String) {
        self.evaluate_notification(ProcessJobNotificationObservation {
            status: self.job_status(),
            line: Some(line),
            tick: self.started_at.elapsed().as_secs(),
        })
        .await;
    }

    async fn evaluate_completion_notification(&self) {
        let excerpt = self.snapshot_output().last().cloned();
        self.evaluate_notification(ProcessJobNotificationObservation {
            status: self.job_status(),
            line: excerpt,
            tick: self.started_at.elapsed().as_secs(),
        })
        .await;
    }

    async fn evaluate_notification(&self, observation: ProcessJobNotificationObservation) {
        let engine = DefaultProcessJobNotificationPolicyEngine;
        let mut state = self.notification_state.lock().await;
        let decisions = engine.evaluate(&self.notification_policy, &mut state, observation).await;
        drop(state);
        for decision in decisions {
            self.record_notification(decision);
        }
    }

    fn record_notification(&self, decision: ProcessJobNotificationDecision) {
        let event = ProcessJobNotificationEvent {
            event_id: ProcessJobEventId(format!(
                "{}_evt_{}",
                self.id,
                self.next_notification_seq.fetch_add(1, Ordering::Relaxed) + 1
            )),
            id: ProcessJobId(self.id.clone()),
            backend: ProcessJobBackendKind::Native,
            owner: ProcessJobOwnerScope::DaemonGlobal,
            kind: decision.kind,
            status: self.job_status(),
            created_at: Utc::now(),
            summary: decision.summary,
            log_excerpt: decision.log_excerpt,
            log_refs: Vec::new(),
        };
        self.notifications.lock().expect("process notification lock poisoned").push(event);
    }

    fn drain_new_notifications(&self) -> Vec<ProcessJobNotificationEvent> {
        let notifications = self.notifications.lock().expect("process notification lock poisoned");
        let mut cursor = self.notification_cursor.lock().expect("process notification cursor lock poisoned");
        let new = notifications.get(*cursor..).unwrap_or(&[]).to_vec();
        *cursor = notifications.len();
        new
    }

    fn job_status(&self) -> ProcessJobStatus {
        status_to_job_status(&self.status())
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
        let admission = match ProcessTool::reserve_native_start() {
            Ok(admission) => admission,
            Err(decision) => return Ok(ProcessTool::admission_denied_receipt(decision)),
        };

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
        let entry = Arc::new(ProcessEntry::new(
            id.clone(),
            display_command,
            stdin,
            kill_tx,
            pid,
            request.notification_policy.clone(),
        ));
        let backend_ref = entry.backend_ref.clone();
        ProcessTool::insert(entry.clone());
        admission.release();
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

    async fn adopt(&self, request: AdoptProcessJobRequest) -> Result<ProcessJobReceipt, RuntimeError> {
        if request.backend != ProcessJobBackendKind::Native {
            return Ok(ProcessJobReceipt::unsupported(
                ProcessJobOperation::Adopt,
                None,
                request.backend,
                "adopt",
                "native process service only supports native adoption requests",
            ));
        }
        if !request.is_authorized() {
            return Ok(ProcessJobReceipt::permission_denied(
                ProcessJobOperation::Adopt,
                ProcessJobBackendKind::Native,
                "adopt",
                "native pid adoption denied by caller identity or capability scope",
            ));
        }
        let pid = native_pid_from_backend_ref(&request.backend_ref)?;
        if !native_pid_is_alive(pid) {
            return Ok(ProcessJobReceipt {
                operation: ProcessJobOperation::Adopt,
                id: None,
                backend: Some(ProcessJobBackendKind::Native),
                status: Some(ProcessJobStatus::LostAfterRestart),
                backend_ref: Some(request.backend_ref),
                log_refs: Vec::new(),
                summary: format!("native pid {pid} is not signalable; refusing adoption"),
                error: Some(ProcessJobError {
                    code: ProcessJobErrorCode::NotFound,
                    operation: ProcessJobOperation::Adopt,
                    id: None,
                    backend: Some(ProcessJobBackendKind::Native),
                    action: Some("adopt".to_string()),
                    message: format!("native pid {pid} is not signalable"),
                }),
            });
        }
        let id = ProcessJobId(format!("{ADOPTED_NATIVE_ID_PREFIX}{pid}"));
        Ok(ProcessJobReceipt {
            operation: ProcessJobOperation::Adopt,
            id: Some(id),
            backend: Some(ProcessJobBackendKind::Native),
            status: Some(ProcessJobStatus::ReattachedLogIncomplete),
            backend_ref: Some(BackendRef(format!("pid:{pid}"))),
            log_refs: Vec::new(),
            summary: format!(
                "Adopted native pid {pid} as metadata-only process job; live stdout/stderr streams are unavailable"
            ),
            error: None,
        })
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

#[async_trait]
pub trait PueueRunner: Send + Sync {
    async fn run(&self, args: &[String]) -> Result<String, RuntimeError>;
}

#[derive(Debug, Default, Clone, Copy)]
pub struct PueueCliRunner;

#[async_trait]
impl PueueRunner for PueueCliRunner {
    async fn run(&self, args: &[String]) -> Result<String, RuntimeError> {
        let output = Command::new("pueue")
            .args(args)
            .output()
            .await
            .map_err(|e| RuntimeError::InvalidTool(format!("failed to execute pueue: {e}")))?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
            let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
            let message = if stderr.is_empty() { stdout } else { stderr };
            return Err(RuntimeError::InvalidTool(format!("pueue {:?} failed: {message}", args)));
        }
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }
}

#[derive(Clone)]
pub struct PueueProcessJobService<R = PueueCliRunner> {
    runner: Arc<R>,
    enabled: bool,
}

impl Default for PueueProcessJobService<PueueCliRunner> {
    fn default() -> Self {
        Self::new(PueueCliRunner)
    }
}

impl<R> PueueProcessJobService<R> {
    pub fn new(runner: R) -> Self {
        Self {
            runner: Arc::new(runner),
            enabled: true,
        }
    }

    #[cfg(test)]
    fn disabled(runner: R) -> Self {
        Self {
            runner: Arc::new(runner),
            enabled: false,
        }
    }
}

impl<R: PueueRunner> PueueProcessJobService<R> {
    async fn ensure_available(
        &self,
        operation: ProcessJobOperation,
    ) -> Result<Option<ProcessJobReceipt>, RuntimeError> {
        if !self.enabled {
            return Ok(Some(pueue_backend_unavailable(operation, "pueue backend is disabled by configuration")));
        }
        match self.runner.run(&["--version".to_string()]).await {
            Ok(_) => Ok(None),
            Err(error) => Ok(Some(pueue_backend_unavailable(operation, error.to_string()))),
        }
    }

    async fn pueue_tasks(&self) -> Result<Vec<PueueTaskProjection>, RuntimeError> {
        let json = self.runner.run(&["status".to_string(), "--json".to_string()]).await?;
        Ok(parse_pueue_tasks(&json))
    }

    async fn pueue_task(&self, id: &ProcessJobId) -> Result<PueueTaskProjection, RuntimeError> {
        let task_id = pueue_task_id(id)?;
        self.pueue_tasks()
            .await?
            .into_iter()
            .find(|task| task.task_id == task_id)
            .ok_or_else(|| RuntimeError::InvalidTool(format!("Unknown pueue process session_id: {}", id.0)))
    }
}

#[async_trait]
impl<R: PueueRunner> ProcessJobService for PueueProcessJobService<R> {
    async fn start(&self, request: StartProcessJobRequest) -> Result<ProcessJobReceipt, RuntimeError> {
        if request.backend != ProcessJobBackendKind::Pueue {
            return Ok(ProcessJobReceipt::unsupported(
                ProcessJobOperation::Start,
                None,
                request.backend,
                "start",
                "pueue process service only supports pueue backend requests",
            ));
        }
        if let Some(receipt) = self.ensure_available(ProcessJobOperation::Start).await? {
            return Ok(receipt);
        }
        let command = pueue_command_from_request(&request)?;
        let mut args = vec!["add".to_string(), "--print-task-id".to_string()];
        if let ProcessJobCwd::Explicit(cwd) = &request.cwd {
            args.push("--working-directory".to_string());
            args.push(cwd.display().to_string());
        }
        if let Some(group) = request.metadata.get("pueue_group").or_else(|| request.metadata.get("group")) {
            args.push("--group".to_string());
            args.push(group.clone());
        }
        if let Some(label) = request.metadata.get("label") {
            args.push("--label".to_string());
            args.push(label.clone());
        }
        args.push(command);
        let output = self.runner.run(&args).await?;
        let task_id = output
            .lines()
            .find_map(|line| line.trim().parse::<u64>().ok())
            .ok_or_else(|| RuntimeError::InvalidTool(format!("pueue add did not return a task id: {output}")))?;
        let id = ProcessJobId(format!("pueue_{task_id}"));
        Ok(ProcessJobReceipt {
            operation: ProcessJobOperation::Start,
            id: Some(id.clone()),
            backend: Some(ProcessJobBackendKind::Pueue),
            status: Some(ProcessJobStatus::Pending),
            backend_ref: Some(BackendRef(format!("pueue:{task_id}"))),
            log_refs: pueue_log_refs(&id),
            summary: format!("Started pueue task {task_id} as {}", id.0),
            error: None,
        })
    }

    async fn list(&self, filter: ProcessJobFilter) -> Result<Vec<ProcessJobSummary>, RuntimeError> {
        if self.ensure_available(ProcessJobOperation::List).await?.is_some() {
            return Ok(Vec::new());
        }
        let summaries = self
            .pueue_tasks()
            .await?
            .into_iter()
            .map(|task| task.summary())
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
        let task = self.pueue_task(&id).await?;
        Ok(task.receipt(ProcessJobOperation::Poll, format!("{} status: {}", id.0, task.status_label)))
    }

    async fn log(&self, id: ProcessJobId, range: ProcessJobLogRange) -> Result<ProcessJobLogChunk, RuntimeError> {
        let task_id = pueue_task_id(&id)?;
        let lines = range.limit_bytes.clamp(1, DEFAULT_LOG_LIMIT as u64).to_string();
        let output = self
            .runner
            .run(&[
                "log".to_string(),
                "--json".to_string(),
                "--lines".to_string(),
                lines,
                task_id.to_string(),
            ])
            .await?;
        let text = parse_pueue_log_text(&output, task_id);
        let start = range.offset.unwrap_or(0);
        let len = u64::try_from(text.lines().count()).unwrap_or(u64::MAX);
        Ok(ProcessJobLogChunk {
            id,
            backend: ProcessJobBackendKind::Pueue,
            stream: range.stream,
            cursor: ProcessJobLogCursor {
                stream: range.stream,
                offset: start,
            },
            next_cursor: Some(ProcessJobLogCursor {
                stream: range.stream,
                offset: start.saturating_add(len),
            }),
            text,
            truncated: false,
        })
    }

    async fn wait(&self, id: ProcessJobId, timeout: Option<Duration>) -> Result<ProcessJobReceipt, RuntimeError> {
        let deadline = Instant::now() + timeout.unwrap_or(Duration::from_secs(30));
        loop {
            let task = self.pueue_task(&id).await?;
            if task.status.is_terminal() {
                return Ok(task.receipt(
                    ProcessJobOperation::Wait,
                    format!("{} finished with status: {}", id.0, task.status_label),
                ));
            }
            if Instant::now() >= deadline {
                return Ok(task.receipt(
                    ProcessJobOperation::Wait,
                    format!("{} still running as pueue task {}", id.0, task.task_id),
                ));
            }
            tokio::time::sleep(Duration::from_millis(250)).await;
        }
    }

    async fn kill(&self, id: ProcessJobId) -> Result<ProcessJobReceipt, RuntimeError> {
        let task_id = pueue_task_id(&id)?;
        self.runner.run(&["kill".to_string(), task_id.to_string()]).await?;
        Ok(ProcessJobReceipt {
            operation: ProcessJobOperation::Kill,
            id: Some(id),
            backend: Some(ProcessJobBackendKind::Pueue),
            status: Some(ProcessJobStatus::Killed),
            backend_ref: Some(BackendRef(format!("pueue:{task_id}"))),
            log_refs: Vec::new(),
            summary: format!("Kill requested for pueue task {task_id}"),
            error: None,
        })
    }

    async fn restart(&self, id: ProcessJobId) -> Result<ProcessJobReceipt, RuntimeError> {
        let task_id = pueue_task_id(&id)?;
        self.runner.run(&["restart".to_string(), "--in-place".to_string(), task_id.to_string()]).await?;
        Ok(ProcessJobReceipt {
            operation: ProcessJobOperation::Restart,
            id: Some(id),
            backend: Some(ProcessJobBackendKind::Pueue),
            status: Some(ProcessJobStatus::Pending),
            backend_ref: Some(BackendRef(format!("pueue:{task_id}"))),
            log_refs: Vec::new(),
            summary: format!("Restart requested for pueue task {task_id}"),
            error: None,
        })
    }

    async fn write_stdin(
        &self,
        id: ProcessJobId,
        _data: Vec<u8>,
        _newline: bool,
    ) -> Result<ProcessJobReceipt, RuntimeError> {
        Ok(ProcessJobReceipt::unsupported(
            ProcessJobOperation::WriteStdin,
            Some(id),
            ProcessJobBackendKind::Pueue,
            "write_stdin",
            "pueue backend stdin mutation is not supported by the process tool",
        ))
    }

    async fn close_stdin(&self, id: ProcessJobId) -> Result<ProcessJobReceipt, RuntimeError> {
        Ok(ProcessJobReceipt::unsupported(
            ProcessJobOperation::CloseStdin,
            Some(id),
            ProcessJobBackendKind::Pueue,
            "close_stdin",
            "pueue backend stdin close is not supported by the process tool",
        ))
    }

    async fn adopt(&self, request: AdoptProcessJobRequest) -> Result<ProcessJobReceipt, RuntimeError> {
        if request.backend != ProcessJobBackendKind::Pueue {
            return Ok(ProcessJobReceipt::unsupported(
                ProcessJobOperation::Adopt,
                None,
                request.backend,
                "adopt",
                "pueue process service only supports pueue adoption requests",
            ));
        }
        if !request.is_authorized() {
            return Ok(ProcessJobReceipt::permission_denied(
                ProcessJobOperation::Adopt,
                ProcessJobBackendKind::Pueue,
                "adopt",
                "pueue task adoption denied by caller identity, capability scope, or backend-selection grant",
            ));
        }
        if let Some(receipt) = self.ensure_available(ProcessJobOperation::Adopt).await? {
            return Ok(receipt);
        }
        let task_id = pueue_task_id_from_backend_ref(&request.backend_ref)?;
        let task = self
            .pueue_tasks()
            .await?
            .into_iter()
            .find(|task| task.task_id == task_id)
            .ok_or_else(|| RuntimeError::InvalidTool(format!("Unknown pueue task id for adoption: {task_id}")))?;
        Ok(
            task.receipt(
                ProcessJobOperation::Adopt,
                format!("Adopted pueue task {task_id} as {}", task.process_id().0),
            ),
        )
    }

    async fn garbage_collect(&self, _filter: ProcessJobFilter) -> Result<ProcessJobReceipt, RuntimeError> {
        Ok(ProcessJobReceipt::unsupported(
            ProcessJobOperation::GarbageCollect,
            None,
            ProcessJobBackendKind::Pueue,
            "garbage_collect",
            "pueue retention is owned by pueue cleanup policies for now",
        ))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PueueTaskProjection {
    task_id: u64,
    command: String,
    group: Option<String>,
    status: ProcessJobStatus,
    status_label: String,
    started_at: Option<DateTime<Utc>>,
    updated_at: DateTime<Utc>,
    completed_at: Option<DateTime<Utc>>,
}

impl PueueTaskProjection {
    fn process_id(&self) -> ProcessJobId {
        ProcessJobId(format!("pueue_{}", self.task_id))
    }

    fn backend_ref(&self) -> BackendRef {
        BackendRef(format!("pueue:{}", self.task_id))
    }

    fn summary(&self) -> ProcessJobSummary {
        let id = self.process_id();
        let mut metadata = self.command.clone();
        if let Some(group) = &self.group {
            metadata = format!("[{group}] {metadata}");
        }
        ProcessJobSummary {
            id: id.clone(),
            backend: ProcessJobBackendKind::Pueue,
            backend_ref: Some(self.backend_ref()),
            owner: ProcessJobOwnerScope::DaemonGlobal,
            status: self.status.clone(),
            command_preview: metadata.chars().take(MAX_COMMAND_PREVIEW_LEN).collect(),
            cwd: ProcessJobCwd::Inherited,
            started_at: self.started_at,
            updated_at: self.updated_at,
            completed_at: self.completed_at,
            log_refs: pueue_log_refs(&id),
        }
    }

    fn receipt(&self, operation: ProcessJobOperation, summary: String) -> ProcessJobReceipt {
        let id = self.process_id();
        ProcessJobReceipt {
            operation,
            id: Some(id.clone()),
            backend: Some(ProcessJobBackendKind::Pueue),
            status: Some(self.status.clone()),
            backend_ref: Some(self.backend_ref()),
            log_refs: pueue_log_refs(&id),
            summary,
            error: None,
        }
    }
}

fn pueue_command_from_request(request: &StartProcessJobRequest) -> Result<String, RuntimeError> {
    match (&request.shell_command, &request.program) {
        (Some(command), None) => Ok(command.clone()),
        (None, Some(program)) => Ok(format_direct_command(program, &request.args)),
        (Some(_), Some(_)) => Err(RuntimeError::InvalidTool(
            "pueue start requires either shell_command or program, not both".to_string(),
        )),
        (None, None) => Err(RuntimeError::InvalidTool("pueue start requires shell_command or program".to_string())),
    }
}

fn pueue_task_id(id: &ProcessJobId) -> Result<u64, RuntimeError> {
    id.0.strip_prefix("pueue_")
        .and_then(|raw| raw.parse::<u64>().ok())
        .ok_or_else(|| RuntimeError::InvalidTool(format!("{} is not a pueue process id", id.0)))
}

fn pueue_log_refs(id: &ProcessJobId) -> Vec<ProcessJobLogRef> {
    vec![ProcessJobLogRef {
        stream: ProcessJobStream::Combined,
        reference: format!("pueue:{}:log", id.0.trim_start_matches("pueue_")),
        retained_until: None,
        max_bytes: None,
    }]
}

fn pueue_backend_unavailable(operation: ProcessJobOperation, reason: impl Into<String>) -> ProcessJobReceipt {
    let reason = reason.into();
    ProcessJobReceipt {
        operation,
        id: None,
        backend: Some(ProcessJobBackendKind::Pueue),
        status: Some(ProcessJobStatus::BackendUnavailable { reason: reason.clone() }),
        backend_ref: None,
        log_refs: Vec::new(),
        summary: reason.clone(),
        error: Some(ProcessJobError {
            code: ProcessJobErrorCode::BackendUnavailable,
            operation,
            id: None,
            backend: Some(ProcessJobBackendKind::Pueue),
            action: Some(format!("{:?}", operation).to_ascii_lowercase()),
            message: reason,
        }),
    }
}

fn parse_pueue_tasks(raw: &str) -> Vec<PueueTaskProjection> {
    let Ok(value) = serde_json::from_str::<Value>(raw) else {
        return Vec::new();
    };
    let Some(tasks) = value.get("tasks").and_then(Value::as_object) else {
        return Vec::new();
    };
    tasks.values().filter_map(parse_pueue_task).collect()
}

fn parse_pueue_task(value: &Value) -> Option<PueueTaskProjection> {
    let task_id = value.get("id")?.as_u64()?;
    let command = value
        .get("original_command")
        .or_else(|| value.get("command"))
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    let group = value.get("group").and_then(Value::as_str).map(str::to_string);
    let (status, status_label, terminal_time) = parse_pueue_status(value.get("status"));
    let created_at = value.get("created_at").and_then(Value::as_str).and_then(parse_pueue_time);
    let started_at = value
        .get("start")
        .or_else(|| value.get("started_at"))
        .and_then(Value::as_str)
        .and_then(parse_pueue_time)
        .or(created_at);
    let updated_at = terminal_time.or(started_at).unwrap_or_else(Utc::now);
    Some(PueueTaskProjection {
        task_id,
        command,
        group,
        status,
        status_label,
        started_at,
        updated_at,
        completed_at: terminal_time,
    })
}

fn parse_pueue_status(status: Option<&Value>) -> (ProcessJobStatus, String, Option<DateTime<Utc>>) {
    let Some(status) = status else {
        return (
            ProcessJobStatus::Unknown {
                raw: "missing".to_string(),
            },
            "missing".to_string(),
            None,
        );
    };
    let Some((name, detail)) = status.as_object().and_then(|object| object.iter().next()) else {
        return (
            ProcessJobStatus::Unknown {
                raw: status.to_string(),
            },
            status.to_string(),
            None,
        );
    };
    let lower = name.to_ascii_lowercase();
    let completed_at = detail
        .get("finished_at")
        .or_else(|| detail.get("end"))
        .or_else(|| detail.get("done_at"))
        .and_then(Value::as_str)
        .and_then(parse_pueue_time);
    let exit_code = detail
        .get("exit_code")
        .or_else(|| detail.get("code"))
        .and_then(Value::as_i64)
        .and_then(|code| i32::try_from(code).ok());
    let projected = match lower.as_str() {
        "running" => ProcessJobStatus::Running,
        "queued" | "stashed" | "paused" | "locked" => ProcessJobStatus::Pending,
        "done" | "success" | "succeeded" => ProcessJobStatus::Succeeded {
            exit_code: exit_code.or(Some(0)),
        },
        "failed" => ProcessJobStatus::Failed {
            exit_code,
            reason: detail.get("reason").and_then(Value::as_str).unwrap_or("pueue task failed").to_string(),
        },
        "killed" => ProcessJobStatus::Killed,
        _ if lower.contains("failed") => ProcessJobStatus::Failed {
            exit_code,
            reason: name.clone(),
        },
        _ => ProcessJobStatus::Unknown { raw: name.clone() },
    };
    (projected, lower, completed_at)
}

fn parse_pueue_log_text(raw: &str, task_id: u64) -> String {
    let Ok(value) = serde_json::from_str::<Value>(raw) else {
        return raw.to_string();
    };
    let task = value
        .get("tasks")
        .and_then(Value::as_object)
        .and_then(|tasks| tasks.get(&task_id.to_string()))
        .or_else(|| value.get(&task_id.to_string()))
        .or_else(|| value.get("output"));
    let Some(task) = task else {
        return String::new();
    };
    for key in ["output", "log", "stdout", "stderr"] {
        if let Some(text) = task.get(key).and_then(Value::as_str) {
            return text.to_string();
        }
    }
    if let Some(lines) = task.get("lines").and_then(Value::as_array) {
        return lines.iter().filter_map(Value::as_str).collect::<Vec<_>>().join("\n");
    }
    task.as_str().unwrap_or_default().to_string()
}

fn parse_pueue_time(raw: &str) -> Option<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(raw).ok().map(|time| time.with_timezone(&Utc))
}

#[async_trait]
pub trait SystemdRunner: Send + Sync {
    async fn run(&self, program: &str, args: &[String]) -> Result<String, RuntimeError>;
}

#[derive(Debug, Default, Clone, Copy)]
pub struct SystemdCliRunner;

#[async_trait]
impl SystemdRunner for SystemdCliRunner {
    async fn run(&self, program: &str, args: &[String]) -> Result<String, RuntimeError> {
        let output = Command::new(program)
            .args(args)
            .output()
            .await
            .map_err(|e| RuntimeError::InvalidTool(format!("failed to execute {program}: {e}")))?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
            let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
            let message = if stderr.is_empty() { stdout } else { stderr };
            return Err(RuntimeError::InvalidTool(format!("{program} {:?} failed: {message}", args)));
        }
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }
}

#[derive(Clone)]
pub struct SystemdProcessJobService<R = SystemdCliRunner> {
    runner: Arc<R>,
    enabled: bool,
    user_mode: bool,
}

impl Default for SystemdProcessJobService<SystemdCliRunner> {
    fn default() -> Self {
        Self::new(SystemdCliRunner)
    }
}

impl<R> SystemdProcessJobService<R> {
    pub fn new(runner: R) -> Self {
        Self {
            runner: Arc::new(runner),
            enabled: true,
            user_mode: true,
        }
    }

    #[cfg(test)]
    fn disabled(runner: R) -> Self {
        Self {
            runner: Arc::new(runner),
            enabled: false,
            user_mode: true,
        }
    }

    fn manager_args(&self) -> Vec<String> {
        if self.user_mode {
            vec!["--user".to_string()]
        } else {
            Vec::new()
        }
    }
}

impl<R: SystemdRunner> SystemdProcessJobService<R> {
    async fn ensure_available(
        &self,
        operation: ProcessJobOperation,
    ) -> Result<Option<ProcessJobReceipt>, RuntimeError> {
        if !self.enabled {
            return Ok(Some(systemd_backend_unavailable(operation, "systemd backend is disabled by configuration")));
        }
        match self.runner.run("systemctl", &["--version".to_string()]).await {
            Ok(_) => Ok(None),
            Err(error) => Ok(Some(systemd_backend_unavailable(operation, error.to_string()))),
        }
    }

    async fn systemd_unit(&self, id: &ProcessJobId) -> Result<SystemdUnitProjection, RuntimeError> {
        let unit = systemd_unit_name(id)?;
        let mut args = self.manager_args();
        args.extend(["show".to_string(), unit.clone()]);
        args.extend([
            "--property=Id".to_string(),
            "--property=Description".to_string(),
            "--property=ActiveState".to_string(),
            "--property=SubState".to_string(),
            "--property=Result".to_string(),
            "--property=ExecMainStatus".to_string(),
            "--property=ExecMainPID".to_string(),
        ]);
        let output = self.runner.run("systemctl", &args).await?;
        Ok(parse_systemd_show(&output, &unit))
    }
}

#[async_trait]
impl<R: SystemdRunner> ProcessJobService for SystemdProcessJobService<R> {
    async fn start(&self, request: StartProcessJobRequest) -> Result<ProcessJobReceipt, RuntimeError> {
        if request.backend != ProcessJobBackendKind::Systemd {
            return Ok(ProcessJobReceipt::unsupported(
                ProcessJobOperation::Start,
                None,
                request.backend,
                "start",
                "systemd process service only supports systemd backend requests",
            ));
        }
        if let Some(receipt) = self.ensure_available(ProcessJobOperation::Start).await? {
            return Ok(receipt);
        }
        let unit = systemd_unit_from_request(&request);
        let mut args = self.manager_args();
        args.extend(["--unit".to_string(), unit.clone(), "--collect".to_string()]);
        if request.metadata.get("systemd_scope").is_some_and(|value| value == "true") {
            args.push("--scope".to_string());
        }
        if let ProcessJobCwd::Explicit(cwd) = &request.cwd {
            args.push("--working-directory".to_string());
            args.push(cwd.display().to_string());
        }
        match (&request.shell_command, &request.program) {
            (Some(command), None) => args.extend(["sh".to_string(), "-lc".to_string(), command.clone()]),
            (None, Some(program)) => {
                args.push(program.clone());
                args.extend(request.args.clone());
            }
            (Some(_), Some(_)) => {
                return Err(RuntimeError::InvalidTool(
                    "systemd start requires either shell_command or program, not both".to_string(),
                ));
            }
            (None, None) => {
                return Err(RuntimeError::InvalidTool("systemd start requires shell_command or program".to_string()));
            }
        }
        self.runner.run("systemd-run", &args).await?;
        let id = systemd_process_id(&unit);
        Ok(ProcessJobReceipt {
            operation: ProcessJobOperation::Start,
            id: Some(id.clone()),
            backend: Some(ProcessJobBackendKind::Systemd),
            status: Some(ProcessJobStatus::Running),
            backend_ref: Some(BackendRef(format!("systemd:{unit}"))),
            log_refs: systemd_log_refs(&unit),
            summary: format!("Started systemd transient unit {unit} as {}", id.0),
            error: None,
        })
    }

    async fn list(&self, filter: ProcessJobFilter) -> Result<Vec<ProcessJobSummary>, RuntimeError> {
        if self.ensure_available(ProcessJobOperation::List).await?.is_some() {
            return Ok(Vec::new());
        }
        let mut args = self.manager_args();
        args.extend([
            "list-units".to_string(),
            "--type=service".to_string(),
            "--type=scope".to_string(),
            "--all".to_string(),
            "--no-legend".to_string(),
            "--plain".to_string(),
        ]);
        let output = self.runner.run("systemctl", &args).await?;
        Ok(parse_systemd_list_units(&output)
            .into_iter()
            .filter(|summary| filter.backend.is_none_or(|backend| backend == summary.backend))
            .filter(|summary| filter.include_terminal || !summary.status.is_terminal())
            .collect())
    }

    async fn poll(
        &self,
        id: ProcessJobId,
        _cursor: Option<ProcessJobLogCursor>,
    ) -> Result<ProcessJobReceipt, RuntimeError> {
        let unit = self.systemd_unit(&id).await?;
        Ok(unit.receipt(ProcessJobOperation::Poll, format!("{} status: {}", id.0, unit.status_label)))
    }

    async fn log(&self, id: ProcessJobId, range: ProcessJobLogRange) -> Result<ProcessJobLogChunk, RuntimeError> {
        let unit = systemd_unit_name(&id)?;
        let mut args = self.manager_args();
        args.extend([
            "-u".to_string(),
            unit.clone(),
            "--no-pager".to_string(),
            "--output=short-iso".to_string(),
            "-n".to_string(),
            range.limit_bytes.clamp(1, DEFAULT_LOG_LIMIT as u64).to_string(),
        ]);
        let text = self.runner.run("journalctl", &args).await?;
        let start = range.offset.unwrap_or(0);
        let len = u64::try_from(text.lines().count()).unwrap_or(u64::MAX);
        Ok(ProcessJobLogChunk {
            id,
            backend: ProcessJobBackendKind::Systemd,
            stream: range.stream,
            cursor: ProcessJobLogCursor {
                stream: range.stream,
                offset: start,
            },
            next_cursor: Some(ProcessJobLogCursor {
                stream: range.stream,
                offset: start.saturating_add(len),
            }),
            text,
            truncated: false,
        })
    }

    async fn wait(&self, id: ProcessJobId, timeout: Option<Duration>) -> Result<ProcessJobReceipt, RuntimeError> {
        let deadline = Instant::now() + timeout.unwrap_or(Duration::from_secs(30));
        loop {
            let unit = self.systemd_unit(&id).await?;
            if unit.status.is_terminal() {
                return Ok(unit.receipt(
                    ProcessJobOperation::Wait,
                    format!("{} finished with status: {}", id.0, unit.status_label),
                ));
            }
            if Instant::now() >= deadline {
                return Ok(unit.receipt(
                    ProcessJobOperation::Wait,
                    format!("{} still running as systemd unit {}", id.0, unit.unit),
                ));
            }
            tokio::time::sleep(Duration::from_millis(250)).await;
        }
    }

    async fn kill(&self, id: ProcessJobId) -> Result<ProcessJobReceipt, RuntimeError> {
        let unit = systemd_unit_name(&id)?;
        let mut args = self.manager_args();
        args.extend(["kill".to_string(), "--kill-whom=all".to_string(), unit.clone()]);
        self.runner.run("systemctl", &args).await?;
        Ok(ProcessJobReceipt {
            operation: ProcessJobOperation::Kill,
            id: Some(id),
            backend: Some(ProcessJobBackendKind::Systemd),
            status: Some(ProcessJobStatus::Killed),
            backend_ref: Some(BackendRef(format!("systemd:{unit}"))),
            log_refs: systemd_log_refs(&unit),
            summary: format!("Cgroup kill requested for systemd unit {unit}"),
            error: None,
        })
    }

    async fn restart(&self, id: ProcessJobId) -> Result<ProcessJobReceipt, RuntimeError> {
        let unit = systemd_unit_name(&id)?;
        let mut args = self.manager_args();
        args.extend(["restart".to_string(), unit.clone()]);
        self.runner.run("systemctl", &args).await?;
        Ok(ProcessJobReceipt {
            operation: ProcessJobOperation::Restart,
            id: Some(id),
            backend: Some(ProcessJobBackendKind::Systemd),
            status: Some(ProcessJobStatus::Running),
            backend_ref: Some(BackendRef(format!("systemd:{unit}"))),
            log_refs: systemd_log_refs(&unit),
            summary: format!("Restart requested for systemd unit {unit}"),
            error: None,
        })
    }

    async fn write_stdin(
        &self,
        id: ProcessJobId,
        _data: Vec<u8>,
        _newline: bool,
    ) -> Result<ProcessJobReceipt, RuntimeError> {
        Ok(ProcessJobReceipt::unsupported(
            ProcessJobOperation::WriteStdin,
            Some(id),
            ProcessJobBackendKind::Systemd,
            "write_stdin",
            "systemd backend stdin mutation is not supported by the process tool",
        ))
    }

    async fn close_stdin(&self, id: ProcessJobId) -> Result<ProcessJobReceipt, RuntimeError> {
        Ok(ProcessJobReceipt::unsupported(
            ProcessJobOperation::CloseStdin,
            Some(id),
            ProcessJobBackendKind::Systemd,
            "close_stdin",
            "systemd backend stdin close is not supported by the process tool",
        ))
    }

    async fn adopt(&self, request: AdoptProcessJobRequest) -> Result<ProcessJobReceipt, RuntimeError> {
        if request.backend != ProcessJobBackendKind::Systemd {
            return Ok(ProcessJobReceipt::unsupported(
                ProcessJobOperation::Adopt,
                None,
                request.backend,
                "adopt",
                "systemd process service only supports systemd adoption requests",
            ));
        }
        if !request.is_authorized() {
            return Ok(ProcessJobReceipt::permission_denied(
                ProcessJobOperation::Adopt,
                ProcessJobBackendKind::Systemd,
                "adopt",
                "systemd unit adoption denied by caller identity, capability scope, or backend-selection grant",
            ));
        }
        if let Some(receipt) = self.ensure_available(ProcessJobOperation::Adopt).await? {
            return Ok(receipt);
        }
        let unit_name = systemd_unit_name_from_backend_ref(&request.backend_ref)?;
        let unit = self.systemd_unit(&systemd_process_id(&unit_name)).await?;
        Ok(unit.receipt(
            ProcessJobOperation::Adopt,
            format!("Adopted systemd unit {} as {}", unit.unit, unit.process_id().0),
        ))
    }

    async fn garbage_collect(&self, _filter: ProcessJobFilter) -> Result<ProcessJobReceipt, RuntimeError> {
        Ok(ProcessJobReceipt::unsupported(
            ProcessJobOperation::GarbageCollect,
            None,
            ProcessJobBackendKind::Systemd,
            "garbage_collect",
            "systemd transient-unit retention is delegated to systemd --collect for now",
        ))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SystemdUnitProjection {
    unit: String,
    description: String,
    status: ProcessJobStatus,
    status_label: String,
    updated_at: DateTime<Utc>,
}

impl SystemdUnitProjection {
    fn process_id(&self) -> ProcessJobId {
        systemd_process_id(&self.unit)
    }

    fn backend_ref(&self) -> BackendRef {
        BackendRef(format!("systemd:{}", self.unit))
    }

    fn summary(&self) -> ProcessJobSummary {
        let id = self.process_id();
        ProcessJobSummary {
            id: id.clone(),
            backend: ProcessJobBackendKind::Systemd,
            backend_ref: Some(self.backend_ref()),
            owner: ProcessJobOwnerScope::DaemonGlobal,
            status: self.status.clone(),
            command_preview: self.description.chars().take(MAX_COMMAND_PREVIEW_LEN).collect(),
            cwd: ProcessJobCwd::Inherited,
            started_at: None,
            updated_at: self.updated_at,
            completed_at: self.status.is_terminal().then_some(self.updated_at),
            log_refs: systemd_log_refs(&self.unit),
        }
    }

    fn receipt(&self, operation: ProcessJobOperation, summary: String) -> ProcessJobReceipt {
        let id = self.process_id();
        ProcessJobReceipt {
            operation,
            id: Some(id.clone()),
            backend: Some(ProcessJobBackendKind::Systemd),
            status: Some(self.status.clone()),
            backend_ref: Some(self.backend_ref()),
            log_refs: systemd_log_refs(&self.unit),
            summary,
            error: None,
        }
    }
}

fn systemd_process_id(unit: &str) -> ProcessJobId {
    ProcessJobId(format!("systemd_{unit}"))
}

fn systemd_unit_name(id: &ProcessJobId) -> Result<String, RuntimeError> {
    id.0.strip_prefix("systemd_")
        .filter(|unit| unit.ends_with(".service") || unit.ends_with(".scope"))
        .map(str::to_string)
        .ok_or_else(|| RuntimeError::InvalidTool(format!("{} is not a systemd process id", id.0)))
}

fn systemd_unit_from_request(request: &StartProcessJobRequest) -> String {
    if let Some(unit) = request.metadata.get("systemd_unit").filter(|unit| !unit.is_empty()) {
        return normalize_systemd_unit_name(unit);
    }
    let suffix = if request.metadata.get("systemd_scope").is_some_and(|value| value == "true") {
        "scope"
    } else {
        "service"
    };
    let label = request.metadata.get("label").map(String::as_str).unwrap_or(request.command_preview.as_str());
    let safe = sanitize_systemd_unit_component(label);
    format!("clankers-{safe}-{}.{}", Utc::now().timestamp_millis(), suffix)
}

fn normalize_systemd_unit_name(unit: &str) -> String {
    if unit.ends_with(".service") || unit.ends_with(".scope") {
        unit.to_string()
    } else {
        format!("{unit}.service")
    }
}

fn sanitize_systemd_unit_component(raw: &str) -> String {
    let mut safe = raw
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() {
                ch.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect::<String>()
        .trim_matches('-')
        .to_string();
    safe.truncate(48);
    if safe.is_empty() { "job".to_string() } else { safe }
}

fn systemd_log_refs(unit: &str) -> Vec<ProcessJobLogRef> {
    vec![ProcessJobLogRef {
        stream: ProcessJobStream::Combined,
        reference: format!("journalctl:{unit}"),
        retained_until: None,
        max_bytes: None,
    }]
}

fn systemd_backend_unavailable(operation: ProcessJobOperation, reason: impl Into<String>) -> ProcessJobReceipt {
    let reason = reason.into();
    ProcessJobReceipt {
        operation,
        id: None,
        backend: Some(ProcessJobBackendKind::Systemd),
        status: Some(ProcessJobStatus::BackendUnavailable { reason: reason.clone() }),
        backend_ref: None,
        log_refs: Vec::new(),
        summary: reason.clone(),
        error: Some(ProcessJobError {
            code: ProcessJobErrorCode::BackendUnavailable,
            operation,
            id: None,
            backend: Some(ProcessJobBackendKind::Systemd),
            action: Some(format!("{:?}", operation).to_ascii_lowercase()),
            message: reason,
        }),
    }
}

fn parse_systemd_show(raw: &str, fallback_unit: &str) -> SystemdUnitProjection {
    let mut values = std::collections::BTreeMap::new();
    for line in raw.lines() {
        if let Some((key, value)) = line.split_once('=') {
            values.insert(key, value);
        }
    }
    let unit = values.get("Id").copied().filter(|value| !value.is_empty()).unwrap_or(fallback_unit).to_string();
    let description = values
        .get("Description")
        .copied()
        .filter(|value| !value.is_empty())
        .unwrap_or(unit.as_str())
        .to_string();
    let active = values.get("ActiveState").copied().unwrap_or("unknown");
    let sub = values.get("SubState").copied().unwrap_or("unknown");
    let result = values.get("Result").copied().unwrap_or("unknown");
    let exit_code = values.get("ExecMainStatus").and_then(|value| value.parse::<i32>().ok());
    let (status, status_label) = systemd_status_from_parts(active, sub, result, exit_code);
    SystemdUnitProjection {
        unit,
        description,
        status,
        status_label,
        updated_at: Utc::now(),
    }
}

fn parse_systemd_list_units(raw: &str) -> Vec<ProcessJobSummary> {
    raw.lines()
        .filter_map(|line| {
            let fields = line.split_whitespace().collect::<Vec<_>>();
            if fields.len() < 4 {
                return None;
            }
            let unit = fields[0];
            if !(unit.ends_with(".service") || unit.ends_with(".scope")) {
                return None;
            }
            if !unit.starts_with("clankers-") {
                return None;
            }
            let description = if fields.len() > 4 {
                fields[4..].join(" ")
            } else {
                unit.to_string()
            };
            let (status, status_label) = systemd_status_from_parts(fields[2], fields[3], "", None);
            Some(
                SystemdUnitProjection {
                    unit: unit.to_string(),
                    description,
                    status,
                    status_label,
                    updated_at: Utc::now(),
                }
                .summary(),
            )
        })
        .collect()
}

fn systemd_status_from_parts(
    active: &str,
    sub: &str,
    result: &str,
    exit_code: Option<i32>,
) -> (ProcessJobStatus, String) {
    let label = if result.is_empty() || result == "success" {
        format!("{active}/{sub}")
    } else {
        format!("{active}/{sub}/{result}")
    };
    let status = match active {
        "active" | "reloading" | "refreshing" => ProcessJobStatus::Running,
        "activating" => ProcessJobStatus::Pending,
        "failed" => ProcessJobStatus::Failed {
            exit_code,
            reason: if result.is_empty() {
                sub.to_string()
            } else {
                result.to_string()
            },
        },
        "inactive" | "deactivating" if matches!(result, "signal" | "core-dump") => ProcessJobStatus::Killed,
        "inactive" | "deactivating" if result == "success" || exit_code == Some(0) => ProcessJobStatus::Succeeded {
            exit_code: exit_code.or(Some(0)),
        },
        "inactive" | "deactivating" if !result.is_empty() && result != "success" => ProcessJobStatus::Failed {
            exit_code,
            reason: result.to_string(),
        },
        _ => ProcessJobStatus::Unknown { raw: label.clone() },
    };
    (status, label)
}

fn native_pid_from_backend_ref(backend_ref: &BackendRef) -> Result<u32, RuntimeError> {
    let raw = backend_ref.0.strip_prefix("pid:").unwrap_or(backend_ref.0.as_str());
    let pid = raw
        .parse::<u32>()
        .map_err(|_| RuntimeError::InvalidTool(format!("invalid native pid backend_ref: {}", backend_ref.0)))?;
    if pid == 0 {
        return Err(RuntimeError::InvalidTool("native pid adoption requires a non-zero pid".to_string()));
    }
    Ok(pid)
}

fn pueue_task_id_from_backend_ref(backend_ref: &BackendRef) -> Result<u64, RuntimeError> {
    let raw = backend_ref.0.strip_prefix("pueue:").unwrap_or(backend_ref.0.as_str());
    raw.parse::<u64>()
        .map_err(|_| RuntimeError::InvalidTool(format!("invalid pueue task backend_ref: {}", backend_ref.0)))
}

fn systemd_unit_name_from_backend_ref(backend_ref: &BackendRef) -> Result<String, RuntimeError> {
    let unit = backend_ref.0.strip_prefix("systemd:").unwrap_or(backend_ref.0.as_str()).trim();
    if unit.is_empty() || unit.contains('/') || unit.contains("..") {
        return Err(RuntimeError::InvalidTool(format!("invalid systemd unit backend_ref: {}", backend_ref.0)));
    }
    if !(unit.ends_with(".service") || unit.ends_with(".scope")) {
        return Err(RuntimeError::InvalidTool(format!(
            "systemd adoption requires a .service or .scope unit name: {}",
            backend_ref.0
        )));
    }
    Ok(unit.to_string())
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

fn stored_status_to_job_status(status: &StoredProcessJobStatus) -> ProcessJobStatus {
    match status {
        StoredProcessJobStatus::Pending => ProcessJobStatus::Pending,
        StoredProcessJobStatus::Running => ProcessJobStatus::Running,
        StoredProcessJobStatus::Waiting => ProcessJobStatus::Waiting,
        StoredProcessJobStatus::Succeeded { exit_code } => ProcessJobStatus::Succeeded { exit_code: *exit_code },
        StoredProcessJobStatus::Failed { exit_code, reason } => ProcessJobStatus::Failed {
            exit_code: *exit_code,
            reason: reason.clone(),
        },
        StoredProcessJobStatus::Killed => ProcessJobStatus::Killed,
        StoredProcessJobStatus::Cancelled => ProcessJobStatus::Cancelled,
        StoredProcessJobStatus::LostAfterRestart => ProcessJobStatus::LostAfterRestart,
        StoredProcessJobStatus::ReattachedLogIncomplete => ProcessJobStatus::ReattachedLogIncomplete,
        StoredProcessJobStatus::BackendUnavailable { reason } => {
            ProcessJobStatus::BackendUnavailable { reason: reason.clone() }
        }
        StoredProcessJobStatus::Unknown { raw } => ProcessJobStatus::Unknown { raw: raw.clone() },
    }
}

fn stored_backend_to_job_backend(backend: StoredProcessJobBackendKind) -> ProcessJobBackendKind {
    match backend {
        StoredProcessJobBackendKind::Native => ProcessJobBackendKind::Native,
        StoredProcessJobBackendKind::Pueue => ProcessJobBackendKind::Pueue,
        StoredProcessJobBackendKind::Systemd => ProcessJobBackendKind::Systemd,
        StoredProcessJobBackendKind::Unknown => ProcessJobBackendKind::Unknown,
    }
}

fn stored_cwd_to_job_cwd(cwd: &StoredProcessJobCwd) -> ProcessJobCwd {
    match cwd {
        StoredProcessJobCwd::Inherited => ProcessJobCwd::Inherited,
        StoredProcessJobCwd::Explicit(path) => ProcessJobCwd::Explicit(path.clone()),
    }
}

fn stored_log_ref_to_job_log_ref(log_ref: &StoredProcessJobLogRef) -> ProcessJobLogRef {
    ProcessJobLogRef {
        stream: match log_ref.stream {
            StoredProcessJobStream::Stdout => ProcessJobStream::Stdout,
            StoredProcessJobStream::Stderr => ProcessJobStream::Stderr,
            StoredProcessJobStream::Combined => ProcessJobStream::Combined,
        },
        reference: log_ref.reference.clone(),
        retained_until: log_ref.retained_until,
        max_bytes: log_ref.max_bytes,
    }
}

fn stored_record_summary(record: &StoredProcessJobRecord) -> ProcessJobSummary {
    ProcessJobSummary {
        id: ProcessJobId(record.id.clone()),
        backend: stored_backend_to_job_backend(record.backend),
        backend_ref: record.backend_ref.clone().map(BackendRef),
        owner: ProcessJobOwnerScope::DaemonGlobal,
        status: stored_status_to_job_status(&record.status),
        command_preview: record.command_preview.clone(),
        cwd: stored_cwd_to_job_cwd(&record.cwd),
        started_at: Some(record.started_at),
        updated_at: record.updated_at,
        completed_at: record.completed_at,
        log_refs: record.log_refs.iter().map(stored_log_ref_to_job_log_ref).collect(),
    }
}

fn format_process_job_projection(projection: &ProcessJobListProjection) -> String {
    if projection.total_active == 0 && projection.total_completed == 0 {
        return "No background processes.".to_string();
    }
    let mut lines = vec![format!(
        "{:<12} {:<8} {:<24} {:<10} {}",
        "SESSION", "BACKEND", "STATUS", "BUCKET", "COMMAND"
    )];
    lines.push("─".repeat(96));
    for item in projection.active.iter().chain(projection.completed.iter()) {
        let bucket = match item.lifecycle {
            clankers_runtime::process_jobs::ProcessJobLifecycleBucket::Active => "active",
            clankers_runtime::process_jobs::ProcessJobLifecycleBucket::Completed => "completed",
        };
        lines.push(format!(
            "{:<12} {:<8} {:<24} {:<10} {}",
            item.id.0, item.backend_label, item.status_label, bucket, item.command_preview
        ));
    }
    if projection.truncated_active || projection.truncated_completed {
        lines.push(format!(
            "… truncated: showing {}/{} active and {}/{} completed",
            projection.active.len(),
            projection.total_active,
            projection.completed.len(),
            projection.total_completed
        ));
    }
    lines.join(
        "
",
    )
}

fn native_reconciliation_status(status: &StoredProcessJobStatus) -> bool {
    matches!(
        status,
        StoredProcessJobStatus::Pending | StoredProcessJobStatus::Running | StoredProcessJobStatus::Waiting
    )
}

fn native_terminal_status(status: &StoredProcessJobStatus) -> bool {
    matches!(
        status,
        StoredProcessJobStatus::Succeeded { .. }
            | StoredProcessJobStatus::Failed { .. }
            | StoredProcessJobStatus::Killed
            | StoredProcessJobStatus::Cancelled
            | StoredProcessJobStatus::LostAfterRestart
            | StoredProcessJobStatus::BackendUnavailable { .. }
    )
}

#[cfg(unix)]
fn native_pid_is_alive(pid: u32) -> bool {
    let pid = match libc::pid_t::try_from(pid) {
        Ok(pid) if pid > 0 => pid,
        _ => return false,
    };
    // SAFETY: kill(pid, 0) does not send a signal; it only asks the kernel whether
    // the process exists and is signalable from this daemon's credentials.
    unsafe { libc::kill(pid, 0) == 0 || std::io::Error::last_os_error().raw_os_error() == Some(libc::EPERM) }
}

#[cfg(not(unix))]
fn native_pid_is_alive(_pid: u32) -> bool {
    false
}

fn reconciled_native_record(mut record: StoredProcessJobRecord) -> StoredProcessJobRecord {
    if record.backend != StoredProcessJobBackendKind::Native {
        return record;
    }

    let now = Utc::now();
    if native_terminal_status(&record.status) {
        record.safe_metadata.insert("reconciliation".to_string(), "exited".to_string());
        record.completed_at.get_or_insert(now);
        record.updated_at = now;
        return record;
    }

    if !native_reconciliation_status(&record.status) {
        return record;
    }

    match record.os_pid {
        Some(pid) if native_pid_is_alive(pid) => {
            if record.log_refs.is_empty() {
                record.status = StoredProcessJobStatus::ReattachedLogIncomplete;
                record.safe_metadata.insert("reconciliation".to_string(), "reattached-log-incomplete".to_string());
            } else {
                record.status = StoredProcessJobStatus::Running;
                record.safe_metadata.insert("reconciliation".to_string(), "reattached".to_string());
            }
            record.completed_at = None;
        }
        _ => {
            record.status = StoredProcessJobStatus::LostAfterRestart;
            record.completed_at = Some(now);
            record.safe_metadata.insert("reconciliation".to_string(), "lost-after-restart".to_string());
        }
    }
    record.updated_at = now;
    record
}

async fn reconcile_native_record(db: &clankers_db::Db, record: StoredProcessJobRecord) -> StoredProcessJobRecord {
    let reconciled = reconciled_native_record(record.clone());
    if reconciled != record {
        if let Err(error) = db.async_process_jobs().upsert(reconciled.clone()).await {
            tracing::warn!("failed to persist reconciled process job metadata: {error}");
            return record;
        }
    }
    reconciled
}

pub(crate) async fn reconcile_durable_native_process_jobs(db: &clankers_db::Db) -> Vec<StoredProcessJobRecord> {
    match db.async_process_jobs().list().await {
        Ok(records) => {
            let mut reconciled = Vec::with_capacity(records.len());
            for record in records {
                reconciled.push(reconcile_native_record(db, record).await);
            }
            reconciled
        }
        Err(error) => {
            tracing::warn!("failed to reconcile durable process job metadata: {error}");
            Vec::new()
        }
    }
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
        Ok(Some(record)) => Some(reconcile_native_record(db, record).await),
        Ok(None) => None,
        Err(error) => {
            tracing::warn!("failed to read durable process job metadata: {error}");
            None
        }
    }
}

fn env_u64(name: &str) -> Option<u64> {
    std::env::var(name).ok().and_then(|value| value.parse::<u64>().ok())
}

fn gc_param_u64(params: &Value, key: &str, env: &str) -> Option<u64> {
    params.get(key).and_then(Value::as_u64).or_else(|| env_u64(env))
}

fn process_job_retention_policy(params: &Value) -> ProcessJobRetentionPolicy {
    let max_age_days = gc_param_u64(params, "max_age_days", "CLANKERS_PROCESS_JOB_RETENTION_MAX_AGE_DAYS");
    let max_records = gc_param_u64(params, "max_records", "CLANKERS_PROCESS_JOB_RETENTION_MAX_RECORDS")
        .and_then(|value| usize::try_from(value).ok());
    let max_log_bytes = gc_param_u64(params, "max_log_bytes", "CLANKERS_PROCESS_JOB_RETENTION_MAX_LOG_BYTES");
    let defaults = ProcessJobRetentionPolicy::default();
    ProcessJobRetentionPolicy {
        max_age: max_age_days.map(|days| Duration::from_secs(days.saturating_mul(24 * 60 * 60))).or(defaults.max_age),
        max_records: max_records.or(defaults.max_records),
        max_log_bytes: max_log_bytes.or(defaults.max_log_bytes),
    }
}

fn completed_or_updated_at(record: &StoredProcessJobRecord) -> DateTime<Utc> {
    record.completed_at.unwrap_or(record.updated_at)
}

fn retained_log_bytes(record: &StoredProcessJobRecord) -> u64 {
    record.log_refs.iter().filter_map(|log_ref| log_ref.max_bytes).sum()
}

fn safe_native_log_path(log_dir: Option<&PathBuf>, reference: &str) -> Option<PathBuf> {
    let log_dir = log_dir?;
    let relative = reference.strip_prefix("native:")?;
    if relative.split('/').any(|part| part.is_empty() || part == "." || part == "..") {
        return None;
    }
    Some(log_dir.join(relative))
}

fn retention_log_dir(params: &Value) -> Option<PathBuf> {
    params
        .get("log_dir")
        .and_then(Value::as_str)
        .map(PathBuf::from)
        .or_else(|| std::env::var("CLANKERS_PROCESS_JOB_LOG_DIR").ok().map(PathBuf::from))
}

async fn apply_process_job_retention(
    db: Option<&clankers_db::Db>,
    policy: ProcessJobRetentionPolicy,
    log_dir: Option<PathBuf>,
) -> ProcessJobGarbageCollectionReceipt {
    let Some(db) = db else {
        let mut receipt = ProcessJobGarbageCollectionReceipt::empty();
        receipt.failures.push(ProcessJobGarbageCollectionFailure {
            id: None,
            reference: None,
            message: "process job GC requires a durable process-job database".to_string(),
        });
        receipt.refresh_summary();
        return receipt;
    };

    let live_ids = ProcessTool::all_entries()
        .iter()
        .map(|entry| entry.id.clone())
        .collect::<std::collections::BTreeSet<_>>();
    let records = match db.async_process_jobs().list().await {
        Ok(records) => records,
        Err(error) => {
            let mut receipt = ProcessJobGarbageCollectionReceipt::empty();
            receipt.failures.push(ProcessJobGarbageCollectionFailure {
                id: None,
                reference: None,
                message: format!("failed to list process job metadata for GC: {error}"),
            });
            receipt.refresh_summary();
            return receipt;
        }
    };

    let now = Utc::now();
    let age_cutoff = policy.max_age.and_then(|age| chrono::Duration::from_std(age).ok()).map(|age| now - age);
    let mut receipt = ProcessJobGarbageCollectionReceipt::empty();
    let mut terminal = Vec::new();
    let mut remove_ids = std::collections::BTreeSet::<String>::new();

    for record in &records {
        let status = stored_status_to_job_status(&record.status);
        if live_ids.contains(&record.id) || !status.is_terminal() {
            receipt.skipped_active_jobs.push(ProcessJobId(record.id.clone()));
            continue;
        }
        if age_cutoff.is_some_and(|cutoff| completed_or_updated_at(record) < cutoff) {
            remove_ids.insert(record.id.clone());
        }
        terminal.push(record.clone());
    }

    terminal.sort_by_key(completed_or_updated_at);
    terminal.reverse();
    if let Some(max_records) = policy.max_records {
        for record in terminal.iter().skip(max_records) {
            remove_ids.insert(record.id.clone());
        }
    }
    if let Some(max_log_bytes) = policy.max_log_bytes {
        let mut retained = 0_u64;
        for record in &terminal {
            let bytes = retained_log_bytes(record);
            if retained.saturating_add(bytes) > max_log_bytes {
                remove_ids.insert(record.id.clone());
            } else {
                retained = retained.saturating_add(bytes);
            }
        }
    }

    let mut remove_ids_vec = remove_ids.iter().cloned().collect::<Vec<_>>();
    remove_ids_vec.sort();
    for record in records.iter().filter(|record| remove_ids.contains(&record.id)) {
        for log_ref in &record.log_refs {
            let bytes = log_ref.max_bytes.unwrap_or(0);
            receipt.removed_log_bytes = receipt.removed_log_bytes.saturating_add(bytes);
            receipt.released_log_refs.push(ProcessJobReleasedLogRef {
                id: ProcessJobId(record.id.clone()),
                backend: stored_backend_to_job_backend(record.backend),
                reference: log_ref.reference.clone(),
                bytes,
            });
            if let Some(path) = safe_native_log_path(log_dir.as_ref(), &log_ref.reference) {
                match std::fs::remove_file(&path) {
                    Ok(()) => {}
                    Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                    Err(error) => receipt.failures.push(ProcessJobGarbageCollectionFailure {
                        id: Some(ProcessJobId(record.id.clone())),
                        reference: Some(log_ref.reference.clone()),
                        message: format!("failed to remove native log file {}: {error}", path.display()),
                    }),
                }
            }
        }
    }

    match db.async_process_jobs().delete_many(remove_ids_vec.clone()).await {
        Ok(_) => receipt.removed_records = remove_ids_vec.into_iter().map(ProcessJobId).collect(),
        Err(error) => receipt.failures.push(ProcessJobGarbageCollectionFailure {
            id: None,
            reference: None,
            message: format!("failed to remove process job metadata during GC: {error}"),
        }),
    }
    receipt.refresh_summary();
    receipt
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
                    "poll, log, wait, kill, restart, write, submit, close, adopt, gc. Start with either `command` ",
                    "(shell mode) or `program` + `args` (direct exec mode). Prefer this over shell-level &, ",
                    "nohup, disown, or foreground bash for long-lived processes."
                )
                .to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "action": {
                            "type": "string",
                            "enum": ["start", "list", "poll", "log", "wait", "kill", "restart", "write", "submit", "close", "adopt", "gc", "garbage_collect"],
                            "description": "Action to perform"
                        },
                        "backend": {
                            "type": "string",
                            "enum": ["native", "pueue", "systemd"],
                            "description": "Durable backend for start/list/poll/log/wait/kill/restart/adopt (default: native)"
                        },
                        "group": {
                            "type": "string",
                            "description": "Backend group/queue for pueue starts"
                        },
                        "label": {
                            "type": "string",
                            "description": "Backend label/unit label for durable backend starts"
                        },
                        "backend_ref": {
                            "type": "string",
                            "description": "Backend-owned reference for adopt/import, e.g. pid:1234, pueue:42, or systemd:unit.service"
                        },
                        "pid": {
                            "type": ["string", "number"],
                            "description": "Native PID to adopt/import"
                        },
                        "pueue_task_id": {
                            "type": ["string", "number"],
                            "description": "Pueue task id to adopt/import"
                        },
                        "systemd_unit": {
                            "type": "string",
                            "description": "Systemd .service or .scope unit to adopt/import"
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
                        "notify_on_complete": {
                            "type": "boolean",
                            "description": "When true, emit one completion notification when the process reaches a terminal state"
                        },
                        "watch_patterns": {
                            "type": "array",
                            "items": { "type": "string" },
                            "description": "Bounded rare readiness patterns; noisy repeated matches are rate-limited and suppressed"
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
                        "max_age_days": {
                            "type": "number",
                            "description": "GC retention age override in days for completed process/job records"
                        },
                        "max_records": {
                            "type": "number",
                            "description": "GC retention count override for completed process/job records"
                        },
                        "max_log_bytes": {
                            "type": "number",
                            "description": "GC retained-log byte budget override"
                        },
                        "log_dir": {
                            "type": "string",
                            "description": "GC native log directory override; defaults to CLANKERS_PROCESS_JOB_LOG_DIR"
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

    fn reserve_native_start() -> Result<NativeAdmissionReservation, NativeAdmissionDecision> {
        REGISTRY
            .lock()
            .expect("process registry lock poisoned")
            .reserve_start(MAX_NATIVE_ACTIVE_PROCESS_JOBS)
    }

    fn get(session_id: &str) -> Option<Arc<ProcessEntry>> {
        let registry = REGISTRY.lock().expect("process registry lock poisoned");
        registry.entries.get(session_id).cloned()
    }

    fn all_entries() -> Vec<Arc<ProcessEntry>> {
        let registry = REGISTRY.lock().expect("process registry lock poisoned");
        registry.entries.values().cloned().collect()
    }

    fn admission_denied_receipt(decision: NativeAdmissionDecision) -> ProcessJobReceipt {
        let summary = decision.summary();
        ProcessJobReceipt {
            operation: ProcessJobOperation::Start,
            id: None,
            backend: Some(ProcessJobBackendKind::Native),
            status: Some(ProcessJobStatus::Waiting),
            backend_ref: None,
            log_refs: Vec::new(),
            summary: summary.clone(),
            error: Some(ProcessJobError {
                code: ProcessJobErrorCode::ConcurrencyLimitExceeded,
                operation: ProcessJobOperation::Start,
                id: None,
                backend: Some(ProcessJobBackendKind::Native),
                action: Some("start".to_string()),
                message: summary,
            }),
        }
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

    fn notification_policy(params: &Value) -> Result<ProcessJobNotificationPolicy, ToolResult> {
        let notify_on_complete = params.get("notify_on_complete").and_then(Value::as_bool).unwrap_or(false);
        let watch_patterns = match params.get("watch_patterns") {
            Some(value) => {
                let Some(values) = value.as_array() else {
                    return Err(ToolResult::error("Parameter 'watch_patterns' must be an array of strings."));
                };
                let mut patterns = Vec::with_capacity(values.len());
                for value in values {
                    let Some(pattern) = value.as_str() else {
                        return Err(ToolResult::error("Parameter 'watch_patterns' must be an array of strings."));
                    };
                    patterns.push(pattern.to_string());
                }
                patterns
            }
            None => Vec::new(),
        };
        Ok(ProcessJobNotificationPolicy {
            notify_on_complete,
            watch_patterns,
        })
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

    fn requested_backend(params: &Value) -> Result<ProcessJobBackendKind, ToolResult> {
        match params.get("backend").and_then(Value::as_str).unwrap_or("native") {
            "native" => Ok(ProcessJobBackendKind::Native),
            "pueue" => Ok(ProcessJobBackendKind::Pueue),
            "systemd" => Ok(ProcessJobBackendKind::Systemd),
            other => Err(ToolResult::error(format!("Unsupported process backend: {other}"))),
        }
    }

    fn caller_scope_for_owner(
        owner: &ProcessJobOwnerScope,
        capabilities: clankers_runtime::process_jobs::ProcessJobCapabilitySet,
    ) -> clankers_runtime::process_jobs::ProcessJobCallerScope {
        let mut caller = clankers_runtime::process_jobs::ProcessJobCallerScope {
            capabilities,
            ..clankers_runtime::process_jobs::ProcessJobCallerScope::default()
        };
        match owner {
            ProcessJobOwnerScope::Session(session) => caller.session_id = Some(session.clone()),
            ProcessJobOwnerScope::Workspace(workspace) => caller.workspace_id = Some(workspace.clone()),
            ProcessJobOwnerScope::User(user) => caller.user_id = Some(user.clone()),
            ProcessJobOwnerScope::DaemonGlobal => caller.daemon_global = true,
        }
        caller
    }

    fn adopt_request(params: &Value, backend: ProcessJobBackendKind) -> Result<AdoptProcessJobRequest, ToolResult> {
        let backend_ref = params
            .get("backend_ref")
            .or_else(|| params.get("pid"))
            .or_else(|| params.get("pueue_task_id"))
            .or_else(|| params.get("systemd_unit"))
            .and_then(|value| value.as_str().map(str::to_string).or_else(|| value.as_u64().map(|id| id.to_string())))
            .filter(|value| !value.trim().is_empty())
            .ok_or_else(|| {
                ToolResult::error(
                    "Missing required parameter for adopt: backend_ref, pid, pueue_task_id, or systemd_unit",
                )
            })?;
        let owner = ProcessJobOwnerScope::DaemonGlobal;
        let caller = Self::caller_scope_for_owner(
            &owner,
            clankers_runtime::process_jobs::ProcessJobCapabilitySet::full_control(),
        );
        Ok(AdoptProcessJobRequest {
            backend,
            backend_ref: BackendRef(backend_ref),
            owner,
            caller,
        })
    }

    fn pueue_service() -> PueueProcessJobService {
        PueueProcessJobService::default()
    }

    fn systemd_service() -> SystemdProcessJobService {
        SystemdProcessJobService::default()
    }

    fn pueue_id(session_id: &str) -> Option<ProcessJobId> {
        session_id.starts_with("pueue_").then(|| ProcessJobId(session_id.to_string()))
    }

    fn systemd_id(session_id: &str) -> Option<ProcessJobId> {
        session_id.starts_with("systemd_").then(|| ProcessJobId(session_id.to_string()))
    }

    fn start_request(params: &Value, backend: ProcessJobBackendKind) -> Result<StartProcessJobRequest, ToolResult> {
        let command = params.get("command").and_then(Value::as_str).filter(|s| !s.trim().is_empty());
        let program = params.get("program").and_then(Value::as_str).filter(|s| !s.trim().is_empty());
        if command.is_some() && program.is_some() {
            return Err(ToolResult::error("Provide either 'command' or 'program', not both."));
        }
        let args = Self::parse_args(params)?;
        let command_preview = match (command, program) {
            (Some(command), None) => command.to_string(),
            (None, Some(program)) => format_direct_command(program, &args),
            (None, None) => return Err(ToolResult::error("Missing required parameter: command or program")),
            (Some(_), Some(_)) => unreachable!(),
        };
        let mut metadata = std::collections::BTreeMap::new();
        for key in ["group", "label", "systemd_unit", "systemd_scope"] {
            if let Some(value) = params.get(key).and_then(Value::as_str).filter(|value| !value.is_empty()) {
                metadata.insert(key.to_string(), value.to_string());
            }
        }
        Ok(StartProcessJobRequest {
            backend,
            command_preview,
            program: program.map(str::to_string),
            args,
            shell_command: command.map(str::to_string),
            cwd: ProcessJobCwd::Inherited,
            owner: ProcessJobOwnerScope::DaemonGlobal,
            resource_policy: clankers_runtime::process_jobs::ProcessJobResourcePolicy::default(),
            notification_policy: Self::notification_policy(params)?,
            metadata,
        })
    }

    async fn handle_pueue_start(params: &Value) -> ToolResult {
        let request = match Self::start_request(params, ProcessJobBackendKind::Pueue) {
            Ok(request) => request,
            Err(result) => return result,
        };
        match Self::pueue_service().start(request).await {
            Ok(receipt) if receipt.error.is_some() => {
                ToolResult::error(serde_json::to_string(&receipt).unwrap_or(receipt.summary))
            }
            Ok(receipt) => ToolResult::text(serde_json::to_string(&receipt).unwrap_or(receipt.summary)),
            Err(error) => ToolResult::error(error.to_string()),
        }
    }

    async fn handle_systemd_start(params: &Value) -> ToolResult {
        let request = match Self::start_request(params, ProcessJobBackendKind::Systemd) {
            Ok(request) => request,
            Err(result) => return result,
        };
        Self::systemd_receipt_result(Self::systemd_service().start(request).await).await
    }

    async fn pueue_receipt_result(result: Result<ProcessJobReceipt, RuntimeError>) -> ToolResult {
        match result {
            Ok(receipt) if receipt.error.is_some() => {
                ToolResult::error(serde_json::to_string(&receipt).unwrap_or(receipt.summary))
            }
            Ok(receipt) => ToolResult::text(serde_json::to_string(&receipt).unwrap_or(receipt.summary)),
            Err(error) => ToolResult::error(error.to_string()),
        }
    }

    async fn systemd_receipt_result(result: Result<ProcessJobReceipt, RuntimeError>) -> ToolResult {
        match result {
            Ok(receipt) if receipt.error.is_some() => {
                ToolResult::error(serde_json::to_string(&receipt).unwrap_or(receipt.summary))
            }
            Ok(receipt) => ToolResult::text(serde_json::to_string(&receipt).unwrap_or(receipt.summary)),
            Err(error) => ToolResult::error(error.to_string()),
        }
    }

    async fn handle_pueue_log(session_id: &str, params: &Value) -> Option<ToolResult> {
        let id = Self::pueue_id(session_id)?;
        let limit = params.get("limit").and_then(Value::as_u64).unwrap_or(DEFAULT_LOG_LIMIT as u64);
        let offset = params.get("offset").and_then(Value::as_u64);
        let range = ProcessJobLogRange {
            stream: ProcessJobStream::Combined,
            offset,
            limit_bytes: limit,
        };
        Some(match Self::pueue_service().log(id, range).await {
            Ok(chunk) => ToolResult::text(serde_json::to_string(&chunk).unwrap_or(chunk.text)),
            Err(error) => ToolResult::error(error.to_string()),
        })
    }

    async fn handle_systemd_log(session_id: &str, params: &Value) -> Option<ToolResult> {
        let id = Self::systemd_id(session_id)?;
        let limit = params.get("limit").and_then(Value::as_u64).unwrap_or(DEFAULT_LOG_LIMIT as u64);
        let offset = params.get("offset").and_then(Value::as_u64);
        let range = ProcessJobLogRange {
            stream: ProcessJobStream::Combined,
            offset,
            limit_bytes: limit,
        };
        Some(match Self::systemd_service().log(id, range).await {
            Ok(chunk) => ToolResult::text(serde_json::to_string(&chunk).unwrap_or(chunk.text)),
            Err(error) => ToolResult::error(error.to_string()),
        })
    }

    async fn handle_start(&self, ctx: &ToolContext, params: &Value) -> ToolResult {
        let backend = match Self::requested_backend(params) {
            Ok(backend) => backend,
            Err(result) => return result,
        };
        if backend == ProcessJobBackendKind::Pueue {
            return Self::handle_pueue_start(params).await;
        }
        if backend == ProcessJobBackendKind::Systemd {
            return Self::handle_systemd_start(params).await;
        }
        let admission = match Self::reserve_native_start() {
            Ok(admission) => admission,
            Err(decision) => {
                let receipt = Self::admission_denied_receipt(decision);
                let payload = serde_json::to_string(&receipt).unwrap_or_else(|_| receipt.summary.clone());
                return ToolResult::error(payload);
            }
        };

        let (display_command, mut child) = match Self::start_spec(params) {
            Ok(spec) => spec,
            Err(result) => return result,
        };
        let pid = child.id();
        let stdin = child.stdin.take();
        let stdout = match child.stdout.take() {
            Some(stdout) => stdout,
            None => {
                admission.release();
                return ToolResult::error("Failed to capture stdout from background process");
            }
        };
        let stderr = match child.stderr.take() {
            Some(stderr) => stderr,
            None => {
                admission.release();
                return ToolResult::error("Failed to capture stderr from background process");
            }
        };
        let notification_policy = match Self::notification_policy(params) {
            Ok(policy) => policy,
            Err(result) => return result,
        };
        let (kill_tx, kill_rx) = oneshot::channel();
        let id = Self::next_id();
        let entry =
            Arc::new(ProcessEntry::new(id.clone(), display_command.clone(), stdin, kill_tx, pid, notification_policy));
        Self::insert(entry.clone());
        admission.release();

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

    async fn handle_list(ctx: &ToolContext, params: &Value) -> ToolResult {
        let policy = process_job_retention_policy(&json!({}));
        let _ = apply_process_job_retention(ctx.db(), policy, retention_log_dir(&json!({}))).await;
        let backend_filter = match params.get("backend") {
            Some(_) => match Self::requested_backend(params) {
                Ok(backend) => Some(backend),
                Err(result) => return result,
            },
            None => None,
        };
        let mut summaries = Vec::new();
        if backend_filter.is_none_or(|backend| backend == ProcessJobBackendKind::Native) {
            let entries = Self::all_entries();
            let mut durable = Vec::new();
            if let Some(db) = ctx.db() {
                let live_ids = entries.iter().map(|entry| entry.id.as_str()).collect::<std::collections::BTreeSet<_>>();
                durable = reconcile_durable_native_process_jobs(db)
                    .await
                    .into_iter()
                    .filter(|record| !live_ids.contains(record.id.as_str()))
                    .collect();
            }
            summaries.extend(entries.into_iter().map(|entry| entry.summary()));
            summaries.extend(durable.iter().map(stored_record_summary));
        }
        if backend_filter.is_none_or(|backend| backend == ProcessJobBackendKind::Pueue) {
            match Self::pueue_service()
                .list(ProcessJobFilter {
                    backend: Some(ProcessJobBackendKind::Pueue),
                    include_terminal: true,
                    ..ProcessJobFilter::default()
                })
                .await
            {
                Ok(items) => summaries.extend(items),
                Err(error) if backend_filter == Some(ProcessJobBackendKind::Pueue) => {
                    return ToolResult::error(error.to_string());
                }
                Err(error) => tracing::debug!("pueue process projection unavailable: {error}"),
            }
        }
        if backend_filter.is_none_or(|backend| backend == ProcessJobBackendKind::Systemd) {
            match Self::systemd_service()
                .list(ProcessJobFilter {
                    backend: Some(ProcessJobBackendKind::Systemd),
                    include_terminal: true,
                    ..ProcessJobFilter::default()
                })
                .await
            {
                Ok(items) => summaries.extend(items),
                Err(error) if backend_filter == Some(ProcessJobBackendKind::Systemd) => {
                    return ToolResult::error(error.to_string());
                }
                Err(error) => tracing::debug!("systemd process projection unavailable: {error}"),
            }
        }
        let projection = project_process_job_list(summaries, ProcessJobProjectionBounds::default());
        ToolResult::text(format_process_job_projection(&projection))
    }

    async fn handle_gc(ctx: &ToolContext, params: &Value) -> ToolResult {
        let policy = process_job_retention_policy(params);
        let receipt = apply_process_job_retention(ctx.db(), policy, retention_log_dir(params)).await;
        ToolResult::text(serde_json::to_string(&receipt).unwrap_or(receipt.summary))
    }

    async fn handle_poll(ctx: &ToolContext, params: &Value) -> ToolResult {
        let session_id = match Self::required_session(params) {
            Ok(id) => id,
            Err(result) => return result,
        };
        if let Some(id) = Self::pueue_id(&session_id) {
            return Self::pueue_receipt_result(Self::pueue_service().poll(id, None).await).await;
        }
        if let Some(id) = Self::systemd_id(&session_id) {
            return Self::systemd_receipt_result(Self::systemd_service().poll(id, None).await).await;
        }
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
        let notifications = entry.drain_new_notifications();
        if !notifications.is_empty() {
            text.push_str("\nNotifications:");
            for notification in notifications {
                let kind = match &notification.kind {
                    ProcessJobNotificationKind::Completion => "completion".to_string(),
                    ProcessJobNotificationKind::WatchPattern { pattern_index, pattern } => {
                        format!("watch_pattern[{pattern_index}]={pattern}")
                    }
                };
                text.push_str(&format!("\n- {} {}: {}", notification.event_id.0, kind, notification.summary));
            }
        }
        ToolResult::text(text)
    }

    async fn handle_log(ctx: &ToolContext, params: &Value) -> ToolResult {
        let session_id = match Self::required_session(params) {
            Ok(id) => id,
            Err(result) => return result,
        };
        if let Some(result) = Self::handle_pueue_log(&session_id, params).await {
            return result;
        }
        if let Some(result) = Self::handle_systemd_log(&session_id, params).await {
            return result;
        }
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
        if let Some(id) = Self::pueue_id(&session_id) {
            let timeout = params.get("timeout").and_then(Value::as_u64).map(Duration::from_secs);
            return Self::pueue_receipt_result(Self::pueue_service().wait(id, timeout).await).await;
        }
        if let Some(id) = Self::systemd_id(&session_id) {
            let timeout = params.get("timeout").and_then(Value::as_u64).map(Duration::from_secs);
            return Self::systemd_receipt_result(Self::systemd_service().wait(id, timeout).await).await;
        }
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

    async fn handle_kill(params: &Value) -> ToolResult {
        let session_id = match Self::required_session(params) {
            Ok(id) => id,
            Err(result) => return result,
        };
        if let Some(id) = Self::pueue_id(&session_id) {
            return Self::pueue_receipt_result(Self::pueue_service().kill(id).await).await;
        }
        if let Some(id) = Self::systemd_id(&session_id) {
            return Self::systemd_receipt_result(Self::systemd_service().kill(id).await).await;
        }
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
        if let Some(id) = Self::pueue_id(&session_id) {
            return Self::pueue_receipt_result(
                Self::pueue_service().write_stdin(id, data.as_bytes().to_vec(), newline).await,
            )
            .await;
        }
        if let Some(id) = Self::systemd_id(&session_id) {
            return Self::systemd_receipt_result(
                Self::systemd_service().write_stdin(id, data.as_bytes().to_vec(), newline).await,
            )
            .await;
        }
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
        if let Some(id) = Self::pueue_id(&session_id) {
            return Self::pueue_receipt_result(Self::pueue_service().close_stdin(id).await).await;
        }
        if let Some(id) = Self::systemd_id(&session_id) {
            return Self::systemd_receipt_result(Self::systemd_service().close_stdin(id).await).await;
        }
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

    async fn handle_adopt(params: &Value) -> ToolResult {
        let backend = match Self::requested_backend(params) {
            Ok(backend) => backend,
            Err(result) => return result,
        };
        let request = match Self::adopt_request(params, backend) {
            Ok(request) => request,
            Err(result) => return result,
        };
        match backend {
            ProcessJobBackendKind::Native => match NativeProcessJobService.adopt(request).await {
                Ok(receipt) if receipt.error.is_some() => {
                    ToolResult::error(serde_json::to_string(&receipt).unwrap_or(receipt.summary))
                }
                Ok(receipt) => ToolResult::text(serde_json::to_string(&receipt).unwrap_or(receipt.summary)),
                Err(error) => ToolResult::error(error.to_string()),
            },
            ProcessJobBackendKind::Pueue => {
                Self::pueue_receipt_result(Self::pueue_service().adopt(request).await).await
            }
            ProcessJobBackendKind::Systemd => {
                Self::systemd_receipt_result(Self::systemd_service().adopt(request).await).await
            }
            ProcessJobBackendKind::Unknown => ToolResult::error("Unsupported process backend: unknown"),
        }
    }

    async fn handle_restart(params: &Value) -> ToolResult {
        let session_id = match Self::required_session(params) {
            Ok(id) => id,
            Err(result) => return result,
        };
        if let Some(id) = Self::pueue_id(&session_id) {
            return Self::pueue_receipt_result(Self::pueue_service().restart(id).await).await;
        }
        if let Some(id) = Self::systemd_id(&session_id) {
            return Self::systemd_receipt_result(Self::systemd_service().restart(id).await).await;
        }
        ToolResult::error("Native process restart is not supported; start a new native process instead.")
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
            "list" => Self::handle_list(ctx, &params).await,
            "poll" => Self::handle_poll(ctx, &params).await,
            "log" => Self::handle_log(ctx, &params).await,
            "wait" => Self::handle_wait(ctx, &params).await,
            "kill" => Self::handle_kill(&params).await,
            "restart" => Self::handle_restart(&params).await,
            "write" => Self::handle_write(&params, false).await,
            "submit" => Self::handle_write(&params, true).await,
            "close" => Self::handle_close(&params).await,
            "adopt" => Self::handle_adopt(&params).await,
            "gc" | "garbage_collect" => Self::handle_gc(ctx, &params).await,
            other => ToolResult::error(format!("Unknown process action: {other}")),
        }
    }
}

fn spawn_reader<R>(entry: Arc<ProcessEntry>, stream: &'static str, reader: R)
where R: tokio::io::AsyncRead + Unpin + Send + 'static {
    tokio::spawn(async move {
        let mut lines = BufReader::new(reader).lines();
        while let Ok(Some(line)) = lines.next_line().await {
            let clean_line = entry.push_output(stream, &line);
            entry.evaluate_output_notification(clean_line).await;
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
                let outcome = terminate_process_group(pid, &mut child).await;
                entry.set_status(ProcessStatus::Killed {
                    elapsed: started_at.elapsed(),
                    outcome,
                });
            }
        }
        entry.evaluate_completion_notification().await;
        persist_entry(db.as_ref(), &entry).await;
    });
}

async fn terminate_process_group(pid: Option<u32>, child: &mut tokio::process::Child) -> NativeTerminationOutcome {
    #[cfg(unix)]
    if let Some(pid) = pid.and_then(|pid| i32::try_from(pid).ok()) {
        // Negative PID targets the process group whose ID is `pid`.
        unsafe {
            libc::kill(-pid, libc::SIGTERM);
        }
        if tokio::time::timeout(NATIVE_KILL_GRACE, child.wait()).await.is_ok() {
            return NativeTerminationOutcome::GracefulTerm;
        }
        unsafe {
            libc::kill(-pid, libc::SIGKILL);
        }
        let _ = child.wait().await;
        return NativeTerminationOutcome::EscalatedKill;
    }

    child.start_kill().ok();
    let _ = child.wait().await;
    NativeTerminationOutcome::DirectKill
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
            notification_policy: ProcessJobNotificationPolicy::default(),
            metadata: Default::default(),
        }
    }

    fn adopt_request_for(backend: ProcessJobBackendKind, backend_ref: &str) -> AdoptProcessJobRequest {
        let owner = ProcessJobOwnerScope::DaemonGlobal;
        let caller = ProcessTool::caller_scope_for_owner(
            &owner,
            clankers_runtime::process_jobs::ProcessJobCapabilitySet::full_control(),
        );
        AdoptProcessJobRequest {
            backend,
            backend_ref: BackendRef(backend_ref.to_string()),
            owner,
            caller,
        }
    }

    fn denied_adopt_request_for(backend: ProcessJobBackendKind, backend_ref: &str) -> AdoptProcessJobRequest {
        AdoptProcessJobRequest {
            backend,
            backend_ref: BackendRef(backend_ref.to_string()),
            owner: ProcessJobOwnerScope::DaemonGlobal,
            caller: clankers_runtime::process_jobs::ProcessJobCallerScope::default(),
        }
    }

    #[derive(Clone)]
    struct FakePueueRunner {
        calls: Arc<std::sync::Mutex<Vec<Vec<String>>>>,
        status_json: String,
        log_json: String,
        add_output: String,
        fail_version: bool,
    }

    impl FakePueueRunner {
        fn new(status_json: String, log_json: String) -> Self {
            Self {
                calls: Arc::new(std::sync::Mutex::new(Vec::new())),
                status_json,
                log_json,
                add_output: "42\n".to_string(),
                fail_version: false,
            }
        }

        fn unavailable() -> Self {
            Self {
                calls: Arc::new(std::sync::Mutex::new(Vec::new())),
                status_json: "{}".to_string(),
                log_json: "{}".to_string(),
                add_output: String::new(),
                fail_version: true,
            }
        }

        fn calls(&self) -> Vec<Vec<String>> {
            self.calls.lock().expect("calls lock poisoned").clone()
        }
    }

    #[async_trait]
    impl PueueRunner for FakePueueRunner {
        async fn run(&self, args: &[String]) -> Result<String, RuntimeError> {
            self.calls.lock().expect("calls lock poisoned").push(args.to_vec());
            match args.first().map(String::as_str) {
                Some("--version") if self.fail_version => {
                    Err(RuntimeError::InvalidTool("pueue unavailable".to_string()))
                }
                Some("--version") => Ok("pueue 4.0.4".to_string()),
                Some("status") => Ok(self.status_json.clone()),
                Some("log") => Ok(self.log_json.clone()),
                Some("add") => Ok(self.add_output.clone()),
                Some("kill") | Some("restart") => Ok(String::new()),
                other => Err(RuntimeError::InvalidTool(format!("unexpected pueue call: {other:?}"))),
            }
        }
    }

    fn pueue_status_fixture() -> String {
        json!({
            "tasks": {
                "42": {
                    "id": 42,
                    "created_at": "2026-05-17T16:00:00Z",
                    "original_command": "cargo check",
                    "command": "cargo check",
                    "path": "/tmp/work",
                    "group": "builds",
                    "status": { "Running": { "start": "2026-05-17T16:00:01Z" } }
                },
                "43": {
                    "id": 43,
                    "created_at": "2026-05-17T16:00:00Z",
                    "original_command": "false",
                    "command": "false",
                    "path": "/tmp/work",
                    "group": "builds",
                    "status": { "Done": { "finished_at": "2026-05-17T16:00:03Z", "exit_code": 0 } }
                }
            },
            "groups": { "builds": { "parallel_tasks": 1 } }
        })
        .to_string()
    }

    #[tokio::test]
    async fn pueue_backend_projects_status_logs_and_mutations_without_hard_seam() {
        let runner = FakePueueRunner::new(
            pueue_status_fixture(),
            json!({ "tasks": { "42": { "output": "line one\nline two" } } }).to_string(),
        );
        let service = PueueProcessJobService::new(runner.clone());
        let mut request = native_start_request("cargo check");
        request.backend = ProcessJobBackendKind::Pueue;
        request.metadata.insert("group".to_string(), "builds".to_string());
        request.metadata.insert("label".to_string(), "clankers".to_string());

        let started = service.start(request).await.expect("pueue start succeeds");
        assert_eq!(started.id, Some(ProcessJobId("pueue_42".to_string())));
        assert_eq!(started.backend_ref, Some(BackendRef("pueue:42".to_string())));

        let summaries = service
            .list(ProcessJobFilter {
                backend: Some(ProcessJobBackendKind::Pueue),
                include_terminal: true,
                ..ProcessJobFilter::default()
            })
            .await
            .expect("list succeeds");
        assert_eq!(summaries.len(), 2);
        assert_eq!(summaries[0].backend, ProcessJobBackendKind::Pueue);
        assert_eq!(summaries[0].backend_ref, Some(BackendRef("pueue:42".to_string())));
        assert!(summaries[0].command_preview.contains("cargo check"));
        assert_eq!(summaries[0].log_refs[0].reference, "pueue:42:log");

        let poll = service.poll(ProcessJobId("pueue_42".to_string()), None).await.expect("poll succeeds");
        assert_eq!(poll.status, Some(ProcessJobStatus::Running));

        let log = service
            .log(ProcessJobId("pueue_42".to_string()), ProcessJobLogRange {
                stream: ProcessJobStream::Combined,
                offset: None,
                limit_bytes: 10,
            })
            .await
            .expect("log succeeds");
        assert_eq!(log.text, "line one\nline two");

        let killed = service.kill(ProcessJobId("pueue_42".to_string())).await.expect("kill succeeds");
        assert_eq!(killed.status, Some(ProcessJobStatus::Killed));
        let restarted = service.restart(ProcessJobId("pueue_42".to_string())).await.expect("restart succeeds");
        assert_eq!(restarted.status, Some(ProcessJobStatus::Pending));
        let stdin = service
            .write_stdin(ProcessJobId("pueue_42".to_string()), b"input".to_vec(), true)
            .await
            .expect("stdin receipt succeeds");
        assert_eq!(stdin.error.expect("unsupported receipt").code, ProcessJobErrorCode::UnsupportedActionForBackend);

        let calls = runner.calls();
        assert!(calls.iter().any(|call| call == &["--version"]));
        assert!(calls.iter().any(|call| call
            == &[
                "add",
                "--print-task-id",
                "--group",
                "builds",
                "--label",
                "clankers",
                "cargo check"
            ]));
        assert!(calls.iter().any(|call| call == &["status", "--json"]));
        assert!(calls.iter().any(|call| call == &["log", "--json", "--lines", "10", "42"]));
        assert!(calls.iter().any(|call| call == &["kill", "42"]));
        assert!(calls.iter().any(|call| call == &["restart", "--in-place", "42"]));
    }

    #[tokio::test]
    async fn pueue_adoption_imports_task_through_runner_seam_and_fails_closed() {
        let runner = FakePueueRunner::new(pueue_status_fixture(), "{}".to_string());
        let service = PueueProcessJobService::new(runner.clone());

        let denied = service
            .adopt(denied_adopt_request_for(ProcessJobBackendKind::Pueue, "pueue:42"))
            .await
            .expect("denied receipt");
        assert_eq!(denied.error.expect("permission error").code, ProcessJobErrorCode::PermissionDenied);
        assert!(runner.calls().is_empty(), "authorization must fail before pueue CLI seam");

        let adopted = service
            .adopt(adopt_request_for(ProcessJobBackendKind::Pueue, "pueue:42"))
            .await
            .expect("adopt succeeds");
        assert_eq!(adopted.operation, ProcessJobOperation::Adopt);
        assert_eq!(adopted.id, Some(ProcessJobId("pueue_42".to_string())));
        assert_eq!(adopted.backend_ref, Some(BackendRef("pueue:42".to_string())));
        assert!(adopted.summary.contains("Adopted pueue task 42"));
        let calls = runner.calls();
        assert!(calls.iter().any(|call| call == &["--version"]));
        assert!(calls.iter().any(|call| call == &["status", "--json"]));
    }

    #[tokio::test]
    async fn pueue_backend_unavailable_returns_typed_receipt_before_mutation() {
        let runner = FakePueueRunner::unavailable();
        let service = PueueProcessJobService::new(runner.clone());
        let mut request = native_start_request("cargo check");
        request.backend = ProcessJobBackendKind::Pueue;

        let receipt = service.start(request).await.expect("unavailable is typed receipt");
        let error = receipt.error.expect("error receipt");
        assert_eq!(error.code, ProcessJobErrorCode::BackendUnavailable);
        assert_eq!(
            receipt.status,
            Some(ProcessJobStatus::BackendUnavailable {
                reason: "invalid tool: pueue unavailable".to_string()
            })
        );
        assert_eq!(runner.calls(), vec![vec!["--version".to_string()]]);
    }

    #[tokio::test]
    async fn pueue_backend_disabled_is_config_guard_not_cli_failure() {
        let runner = FakePueueRunner::new("{}".to_string(), "{}".to_string());
        let service = PueueProcessJobService::disabled(runner.clone());
        let mut request = native_start_request("cargo check");
        request.backend = ProcessJobBackendKind::Pueue;

        let receipt = service.start(request).await.expect("disabled is typed receipt");
        assert_eq!(receipt.error.expect("error receipt").code, ProcessJobErrorCode::BackendUnavailable);
        assert!(runner.calls().is_empty(), "disabled config must avoid pueue CLI mutation seam");
    }

    #[derive(Clone)]
    struct FakeSystemdRunner {
        calls: Arc<std::sync::Mutex<Vec<(String, Vec<String>)>>>,
        show_output: String,
        list_output: String,
        journal_output: String,
        fail_version: bool,
    }

    impl FakeSystemdRunner {
        fn new(show_output: String, list_output: String, journal_output: String) -> Self {
            Self {
                calls: Arc::new(std::sync::Mutex::new(Vec::new())),
                show_output,
                list_output,
                journal_output,
                fail_version: false,
            }
        }

        fn unavailable() -> Self {
            Self {
                calls: Arc::new(std::sync::Mutex::new(Vec::new())),
                show_output: String::new(),
                list_output: String::new(),
                journal_output: String::new(),
                fail_version: true,
            }
        }

        fn calls(&self) -> Vec<(String, Vec<String>)> {
            self.calls.lock().expect("calls lock poisoned").clone()
        }
    }

    #[async_trait]
    impl SystemdRunner for FakeSystemdRunner {
        async fn run(&self, program: &str, args: &[String]) -> Result<String, RuntimeError> {
            self.calls.lock().expect("calls lock poisoned").push((program.to_string(), args.to_vec()));
            match program {
                "systemctl" if args.first().map(String::as_str) == Some("--version") && self.fail_version => {
                    Err(RuntimeError::InvalidTool("systemd unavailable".to_string()))
                }
                "systemctl" if args.first().map(String::as_str) == Some("--version") => Ok("systemd 255".to_string()),
                "systemd-run" => Ok("Running as unit clankers-build.service".to_string()),
                "systemctl" if args.iter().any(|arg| arg == "list-units") => Ok(self.list_output.clone()),
                "systemctl" if args.iter().any(|arg| arg == "show") => Ok(self.show_output.clone()),
                "systemctl" if args.iter().any(|arg| arg == "kill") || args.iter().any(|arg| arg == "restart") => {
                    Ok(String::new())
                }
                "journalctl" => Ok(self.journal_output.clone()),
                other => Err(RuntimeError::InvalidTool(format!("unexpected systemd call: {other} {args:?}"))),
            }
        }
    }

    fn systemd_show_fixture() -> String {
        [
            "Id=clankers-build.service",
            "Description=cargo check",
            "ActiveState=active",
            "SubState=running",
            "Result=success",
            "ExecMainStatus=0",
            "ExecMainPID=4242",
        ]
        .join("\n")
    }

    fn systemd_list_fixture() -> String {
        "clankers-build.service loaded active running cargo check\nclankers-shell.scope loaded inactive dead shell scope".to_string()
    }

    #[tokio::test]
    async fn systemd_backend_projects_transient_units_logs_and_mutations_without_hard_seam() {
        let runner = FakeSystemdRunner::new(
            systemd_show_fixture(),
            systemd_list_fixture(),
            "2026-05-17T17:00:00Z host cargo[1]: ok".to_string(),
        );
        let service = SystemdProcessJobService::new(runner.clone());
        let mut request = native_start_request("cargo check");
        request.backend = ProcessJobBackendKind::Systemd;
        request.metadata.insert("systemd_unit".to_string(), "clankers-build.service".to_string());
        request.metadata.insert("systemd_scope".to_string(), "true".to_string());

        let started = service.start(request).await.expect("systemd start succeeds");
        assert_eq!(started.id, Some(ProcessJobId("systemd_clankers-build.service".to_string())));
        assert_eq!(started.backend_ref, Some(BackendRef("systemd:clankers-build.service".to_string())));
        assert_eq!(started.log_refs[0].reference, "journalctl:clankers-build.service");

        let summaries = service
            .list(ProcessJobFilter {
                backend: Some(ProcessJobBackendKind::Systemd),
                include_terminal: true,
                ..ProcessJobFilter::default()
            })
            .await
            .expect("list succeeds");
        assert_eq!(summaries.len(), 2);
        assert_eq!(summaries[0].backend, ProcessJobBackendKind::Systemd);
        assert_eq!(summaries[0].backend_ref, Some(BackendRef("systemd:clankers-build.service".to_string())));

        let poll = service
            .poll(ProcessJobId("systemd_clankers-build.service".to_string()), None)
            .await
            .expect("poll succeeds");
        assert_eq!(poll.status, Some(ProcessJobStatus::Running));

        let log = service
            .log(ProcessJobId("systemd_clankers-build.service".to_string()), ProcessJobLogRange {
                stream: ProcessJobStream::Combined,
                offset: None,
                limit_bytes: 10,
            })
            .await
            .expect("log succeeds");
        assert!(log.text.contains("cargo[1]: ok"));

        let killed = service
            .kill(ProcessJobId("systemd_clankers-build.service".to_string()))
            .await
            .expect("kill succeeds");
        assert_eq!(killed.status, Some(ProcessJobStatus::Killed));
        let restarted = service
            .restart(ProcessJobId("systemd_clankers-build.service".to_string()))
            .await
            .expect("restart succeeds");
        assert_eq!(restarted.status, Some(ProcessJobStatus::Running));
        let stdin = service
            .write_stdin(ProcessJobId("systemd_clankers-build.service".to_string()), b"input".to_vec(), true)
            .await
            .expect("stdin receipt succeeds");
        assert_eq!(stdin.error.expect("unsupported receipt").code, ProcessJobErrorCode::UnsupportedActionForBackend);

        let calls = runner.calls();
        assert!(calls.iter().any(|(program, args)| program == "systemd-run"
            && args
                == &[
                    "--user",
                    "--unit",
                    "clankers-build.service",
                    "--collect",
                    "--scope",
                    "sh",
                    "-lc",
                    "cargo check"
                ]));
        assert!(
            calls
                .iter()
                .any(|(program, args)| program == "systemctl" && args.contains(&"list-units".to_string()))
        );
        assert!(calls.iter().any(|(program, args)| program == "journalctl" && args.contains(&"-u".to_string())));
        assert!(calls.iter().any(|(program, args)| program == "systemctl"
            && args.contains(&"kill".to_string())
            && args.contains(&"--kill-whom=all".to_string())));
        assert!(calls.iter().any(|(program, args)| program == "systemctl" && args.contains(&"restart".to_string())));
    }

    #[tokio::test]
    async fn systemd_adoption_imports_unit_through_runner_seam_and_fails_closed() {
        let runner = FakeSystemdRunner::new(systemd_show_fixture(), systemd_list_fixture(), String::new());
        let service = SystemdProcessJobService::new(runner.clone());

        let denied = service
            .adopt(denied_adopt_request_for(ProcessJobBackendKind::Systemd, "systemd:clankers-build.service"))
            .await
            .expect("denied receipt");
        assert_eq!(denied.error.expect("permission error").code, ProcessJobErrorCode::PermissionDenied);
        assert!(runner.calls().is_empty(), "authorization must fail before systemd CLI seam");

        let adopted = service
            .adopt(adopt_request_for(ProcessJobBackendKind::Systemd, "systemd:clankers-build.service"))
            .await
            .expect("adopt succeeds");
        assert_eq!(adopted.operation, ProcessJobOperation::Adopt);
        assert_eq!(adopted.id, Some(ProcessJobId("systemd_clankers-build.service".to_string())));
        assert_eq!(adopted.backend_ref, Some(BackendRef("systemd:clankers-build.service".to_string())));
        assert!(adopted.summary.contains("Adopted systemd unit clankers-build.service"));
        let calls = runner.calls();
        assert!(calls.iter().any(|(program, args)| program == "systemctl" && args == &["--version"]));
        assert!(calls.iter().any(|(program, args)| program == "systemctl" && args.contains(&"show".to_string())));
    }

    #[tokio::test]
    async fn systemd_backend_unavailable_returns_typed_receipt_before_mutation() {
        let runner = FakeSystemdRunner::unavailable();
        let service = SystemdProcessJobService::new(runner.clone());
        let mut request = native_start_request("cargo check");
        request.backend = ProcessJobBackendKind::Systemd;

        let receipt = service.start(request).await.expect("unavailable is typed receipt");
        let error = receipt.error.expect("error receipt");
        assert_eq!(error.code, ProcessJobErrorCode::BackendUnavailable);
        assert_eq!(
            receipt.status,
            Some(ProcessJobStatus::BackendUnavailable {
                reason: "invalid tool: systemd unavailable".to_string()
            })
        );
        assert_eq!(runner.calls(), vec![("systemctl".to_string(), vec!["--version".to_string()])]);
    }

    #[tokio::test]
    async fn systemd_backend_disabled_is_config_guard_not_cli_failure() {
        let runner = FakeSystemdRunner::new(String::new(), String::new(), String::new());
        let service = SystemdProcessJobService::disabled(runner.clone());
        let mut request = native_start_request("cargo check");
        request.backend = ProcessJobBackendKind::Systemd;

        let receipt = service.start(request).await.expect("disabled is typed receipt");
        assert_eq!(receipt.error.expect("error receipt").code, ProcessJobErrorCode::BackendUnavailable);
        assert!(runner.calls().is_empty(), "disabled config must avoid systemd CLI mutation seam");
    }

    #[tokio::test]
    async fn native_pid_adoption_uses_metadata_only_receipt_and_fails_closed() {
        let service = NativeProcessJobService;
        let current_pid = std::process::id();

        let denied = service
            .adopt(denied_adopt_request_for(ProcessJobBackendKind::Native, &format!("pid:{current_pid}")))
            .await
            .expect("denied receipt");
        assert_eq!(denied.error.expect("permission error").code, ProcessJobErrorCode::PermissionDenied);

        let adopted = service
            .adopt(adopt_request_for(ProcessJobBackendKind::Native, &format!("pid:{current_pid}")))
            .await
            .expect("adopt succeeds");
        assert_eq!(adopted.operation, ProcessJobOperation::Adopt);
        assert_eq!(adopted.id, Some(ProcessJobId(format!("native_pid_{current_pid}"))));
        assert_eq!(adopted.backend_ref, Some(BackendRef(format!("pid:{current_pid}"))));
        assert_eq!(adopted.status, Some(ProcessJobStatus::ReattachedLogIncomplete));
        assert!(adopted.summary.contains("metadata-only"));
    }

    #[tokio::test]
    async fn process_tool_adopt_routes_to_backend_service_seams() {
        let tool = ProcessTool::new();
        let adopted = tool
            .execute(&make_ctx(), json!({"action": "adopt", "backend": "native", "pid": std::process::id()}))
            .await;
        assert!(!adopted.is_error, "{adopted:?}");
        assert!(text(&adopted).contains("native_pid_"), "{}", text(&adopted));
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

    #[test]
    fn native_admission_limit_rejects_at_capacity_with_typed_receipt() {
        let accepted = native_admission_decision(MAX_NATIVE_ACTIVE_PROCESS_JOBS - 1, MAX_NATIVE_ACTIVE_PROCESS_JOBS);
        assert!(accepted.accepted);

        let rejected = native_admission_decision(MAX_NATIVE_ACTIVE_PROCESS_JOBS, MAX_NATIVE_ACTIVE_PROCESS_JOBS);
        assert!(!rejected.accepted);
        let receipt = ProcessTool::admission_denied_receipt(rejected);
        assert_eq!(receipt.operation, ProcessJobOperation::Start);
        assert_eq!(receipt.backend, Some(ProcessJobBackendKind::Native));
        assert_eq!(receipt.status, Some(ProcessJobStatus::Waiting));
        let error = receipt.error.expect("typed admission error");
        assert_eq!(error.code, ProcessJobErrorCode::ConcurrencyLimitExceeded);
        assert!(error.message.contains("active process limit reached"));
        let payload = serde_json::to_value(&error).expect("typed error serializes");
        assert_eq!(payload["code"], "concurrency_limit_exceeded");
    }

    #[test]
    fn native_admission_reservations_count_against_capacity_before_spawn() {
        let mut registry = ProcessRegistry::default();
        assert!(registry.admission_decision(1).accepted);

        registry.reserved_starts = 1;
        let rejected = registry.admission_decision(1);
        assert!(!rejected.accepted);
        assert_eq!(rejected.active, 1);
        assert_eq!(rejected.limit, 1);

        registry.release_start_reservation();
        assert!(registry.admission_decision(1).accepted);
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
    async fn process_gc_removes_expired_completed_records_logs_and_skips_active_jobs() {
        let db = clankers_db::Db::in_memory().expect("db opens");
        let ctx = make_ctx_with_db(db.clone());
        let tool = ProcessTool::new();
        let temp = tempfile::tempdir().expect("tempdir");
        let old = Utc::now() - chrono::Duration::days(30);

        let mut expired = StoredProcessJobRecord::new_native(
            "proc_gc_expired",
            "printf expired",
            StoredProcessJobOwnerScope::DaemonGlobal,
        );
        expired.status = StoredProcessJobStatus::Succeeded { exit_code: Some(0) };
        expired.started_at = old;
        expired.updated_at = old;
        expired.completed_at = Some(old);
        expired.log_refs = vec![StoredProcessJobLogRef {
            stream: StoredProcessJobStream::Combined,
            reference: "native:proc_gc_expired/combined.log".to_string(),
            retained_until: None,
            max_bytes: Some(12),
        }];
        let log_path = temp.path().join("proc_gc_expired").join("combined.log");
        std::fs::create_dir_all(log_path.parent().expect("log parent")).expect("log dir");
        std::fs::write(&log_path, b"expired-log!").expect("log write");

        let mut active = StoredProcessJobRecord::new_native(
            "proc_gc_active",
            "sleep active",
            StoredProcessJobOwnerScope::DaemonGlobal,
        );
        active.status = StoredProcessJobStatus::Running;
        active.updated_at = old;
        active.log_refs = vec![StoredProcessJobLogRef {
            stream: StoredProcessJobStream::Combined,
            reference: "native:proc_gc_active/combined.log".to_string(),
            retained_until: None,
            max_bytes: Some(99),
        }];

        db.async_process_jobs().upsert(expired).await.expect("insert expired");
        db.async_process_jobs().upsert(active).await.expect("insert active");

        let result = tool
            .execute(
                &ctx,
                json!({
                    "action": "gc",
                    "max_age_days": 1,
                    "max_records": 100,
                    "max_log_bytes": 1_000_000,
                    "log_dir": temp.path().to_string_lossy()
                }),
            )
            .await;
        assert!(!result.is_error, "{result:?}");
        let payload: serde_json::Value = serde_json::from_str(&text(&result)).expect("gc json");
        assert_eq!(payload["removed_records"][0], "proc_gc_expired");
        assert_eq!(payload["removed_log_bytes"], 12);
        assert_eq!(payload["skipped_active_jobs"][0], "proc_gc_active");
        assert!(payload["failures"].as_array().expect("failures").is_empty(), "{payload}");
        assert!(db.async_process_jobs().get("proc_gc_expired").await.expect("db read").is_none());
        assert!(db.async_process_jobs().get("proc_gc_active").await.expect("db read").is_some());
        assert!(!log_path.exists());
    }

    #[tokio::test]
    async fn process_list_applies_automatic_completed_retention_policy() {
        let db = clankers_db::Db::in_memory().expect("db opens");
        let ctx = make_ctx_with_db(db.clone());
        let tool = ProcessTool::new();
        let old = Utc::now() - chrono::Duration::days(30);
        let mut expired = StoredProcessJobRecord::new_native(
            "proc_list_gc_expired",
            "printf expired",
            StoredProcessJobOwnerScope::DaemonGlobal,
        );
        expired.status = StoredProcessJobStatus::Succeeded { exit_code: Some(0) };
        expired.started_at = old;
        expired.updated_at = old;
        expired.completed_at = Some(old);
        expired.log_refs = vec![StoredProcessJobLogRef {
            stream: StoredProcessJobStream::Combined,
            reference: "native:proc_list_gc_expired/combined.log".to_string(),
            retained_until: None,
            max_bytes: Some(1),
        }];
        db.async_process_jobs().upsert(expired).await.expect("insert expired");

        let listed = tool.execute(&ctx, json!({"action": "list", "backend": "native"})).await;
        assert!(!listed.is_error, "{listed:?}");
        assert!(db.async_process_jobs().get("proc_list_gc_expired").await.expect("db read").is_none());
        assert!(!text(&listed).contains("proc_list_gc_expired"), "{}", text(&listed));
    }

    #[tokio::test]
    async fn process_list_reconciles_durable_native_restart_states() {
        let db = clankers_db::Db::in_memory().expect("db opens");
        let ctx = make_ctx_with_db(db.clone());
        let tool = ProcessTool::new();
        let current_pid = std::process::id();

        let mut reattached = StoredProcessJobRecord::new_native(
            "proc_reattached",
            "sleep still-running",
            StoredProcessJobOwnerScope::DaemonGlobal,
        );
        reattached.status = StoredProcessJobStatus::Running;
        reattached.os_pid = Some(current_pid);
        reattached.log_refs = vec![StoredProcessJobLogRef {
            stream: StoredProcessJobStream::Combined,
            reference: "native:proc_reattached/combined.log".to_string(),
            retained_until: None,
            max_bytes: Some(1),
        }];

        let mut log_incomplete = StoredProcessJobRecord::new_native(
            "proc_log_incomplete",
            "sleep no-log",
            StoredProcessJobOwnerScope::DaemonGlobal,
        );
        log_incomplete.status = StoredProcessJobStatus::Running;
        log_incomplete.os_pid = Some(current_pid);

        let mut lost =
            StoredProcessJobRecord::new_native("proc_lost", "sleep gone", StoredProcessJobOwnerScope::DaemonGlobal);
        lost.status = StoredProcessJobStatus::Running;
        lost.os_pid = None;

        let mut exited =
            StoredProcessJobRecord::new_native("proc_exited", "printf done", StoredProcessJobOwnerScope::DaemonGlobal);
        exited.status = StoredProcessJobStatus::Succeeded { exit_code: Some(0) };

        for record in [reattached, log_incomplete, lost, exited] {
            db.async_process_jobs().upsert(record).await.expect("insert restart record");
        }

        let listed = tool.execute(&ctx, json!({"action": "list"})).await;
        assert!(!listed.is_error, "{listed:?}");
        let listed_text = text(&listed);
        assert!(listed_text.contains("proc_reattached"), "{listed_text}");
        assert!(listed_text.contains("reattached-log-incomplete"), "{listed_text}");
        assert!(listed_text.contains("lost-after-restart"), "{listed_text}");

        let reattached = db
            .async_process_jobs()
            .get("proc_reattached".to_string())
            .await
            .expect("db read")
            .expect("reattached record");
        assert_eq!(reattached.status, StoredProcessJobStatus::Running);
        assert_eq!(reattached.safe_metadata.get("reconciliation").map(String::as_str), Some("reattached"));

        let log_incomplete = db
            .async_process_jobs()
            .get("proc_log_incomplete".to_string())
            .await
            .expect("db read")
            .expect("log incomplete record");
        assert_eq!(log_incomplete.status, StoredProcessJobStatus::ReattachedLogIncomplete);

        let lost = db.async_process_jobs().get("proc_lost".to_string()).await.expect("db read").expect("lost record");
        assert_eq!(lost.status, StoredProcessJobStatus::LostAfterRestart);
        assert!(lost.completed_at.is_some());

        let exited = db
            .async_process_jobs()
            .get("proc_exited".to_string())
            .await
            .expect("db read")
            .expect("exited record");
        assert_eq!(exited.safe_metadata.get("reconciliation").map(String::as_str), Some("exited"));
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
    async fn notify_on_complete_and_watch_patterns_emit_through_policy_seam() {
        let tool = ProcessTool::new();
        let started = tool
            .execute(
                &make_ctx(),
                json!({
                    "action": "start",
                    "command": "printf 'READY\\nREADY\\nREADY\\nREADY\\ndone\\n'",
                    "notify_on_complete": true,
                    "watch_patterns": ["READY"]
                }),
            )
            .await;
        assert!(!started.is_error, "{started:?}");
        let id = extract_process_id(&started);
        let waited = tool.execute(&make_ctx(), json!({"action": "wait", "session_id": id, "timeout": 2})).await;
        assert!(!waited.is_error, "{waited:?}");

        let polled = tool.execute(&make_ctx(), json!({"action": "poll", "session_id": id})).await;
        let polled_text = text(&polled);
        assert!(polled_text.contains("watch_pattern[0]=READY"), "{polled_text}");
        assert!(polled_text.contains("completion"), "{polled_text}");
        assert_eq!(polled_text.matches("watch_pattern[0]=READY").count(), 1, "{polled_text}");
    }

    #[tokio::test]
    async fn cancellation_detach_and_log_poll_do_not_kill_process() {
        let tool = ProcessTool::new();
        let cancel = CancellationToken::new();
        let ctx = ToolContext::new("process-detach-start".to_string(), cancel.clone(), None);
        let started = tool.execute(&ctx, json!({"action": "start", "command": "sleep 1; printf after-detach"})).await;
        assert!(!started.is_error, "{started:?}");
        let id = extract_process_id(&started);

        let wait_id = id.clone();
        let wait_cancel = cancel.clone();
        let waiter = tokio::spawn(async move {
            let tool = ProcessTool::new();
            let ctx = ToolContext::new("process-detach-wait".to_string(), wait_cancel, None);
            tool.execute(&ctx, json!({"action": "wait", "session_id": wait_id, "timeout": 5})).await
        });
        tokio::time::sleep(Duration::from_millis(100)).await;
        cancel.cancel();
        waiter.abort();

        let poll = tool.execute(&make_ctx(), json!({"action": "poll", "session_id": id})).await;
        assert!(!poll.is_error, "{poll:?}");
        assert!(text(&poll).contains("running"), "{}", text(&poll));

        let log = tool.execute(&make_ctx(), json!({"action": "log", "session_id": id, "limit": 1})).await;
        assert!(!log.is_error, "{log:?}");

        let waited = tool.execute(&make_ctx(), json!({"action": "wait", "session_id": id, "timeout": 3})).await;
        assert!(!waited.is_error, "{waited:?}");
        assert!(text(&waited).contains("after-detach"), "{}", text(&waited));
    }

    #[tokio::test]
    async fn kill_stops_running_process() {
        let tool = ProcessTool::new();
        let started = tool.execute(&make_ctx(), json!({"action": "start", "command": "sleep 10"})).await;
        let id = extract_process_id(&started);
        let killed = tool.execute(&make_ctx(), json!({"action": "kill", "session_id": id})).await;
        assert!(!killed.is_error, "{killed:?}");
        let waited = tool.execute(&make_ctx(), json!({"action": "wait", "session_id": id, "timeout": 2})).await;
        assert!(text(&waited).contains("killed(graceful-term)"), "{}", text(&waited));
    }

    #[tokio::test]
    async fn kill_escalates_process_group_after_grace_period() {
        let tool = ProcessTool::new();
        let started = tool
            .execute(
                &make_ctx(),
                json!({
                    "action": "start",
                    "program": "python3",
                    "args": ["-c", "import signal,time; signal.signal(signal.SIGTERM, signal.SIG_IGN); time.sleep(10)"],
                }),
            )
            .await;
        let id = extract_process_id(&started);
        tokio::time::sleep(Duration::from_millis(200)).await;
        let killed = tool.execute(&make_ctx(), json!({"action": "kill", "session_id": id})).await;
        assert!(!killed.is_error, "{killed:?}");
        let waited = tool.execute(&make_ctx(), json!({"action": "wait", "session_id": id, "timeout": 4})).await;
        assert!(text(&waited).contains("killed(escalated-sigkill)"), "{}", text(&waited));
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
