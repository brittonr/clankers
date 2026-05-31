//! Agent-visible background process management.
//!
//! This complements the foreground `bash` tool by keeping long-running child
//! processes alive behind stable session IDs. Agents can poll incremental
//! output, inspect logs, wait, send stdin, and terminate processes.

use std::collections::HashMap;
use std::fmt::Write as _;
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
use clankers_runtime::process_jobs::ProcessJobBackendCapabilities;
use clankers_runtime::process_jobs::ProcessJobBackendKind;
use clankers_runtime::process_jobs::ProcessJobCwd;
use clankers_runtime::process_jobs::ProcessJobError;
use clankers_runtime::process_jobs::ProcessJobErrorCode;
use clankers_runtime::process_jobs::ProcessJobEventId;
use clankers_runtime::process_jobs::ProcessJobFilter;
use clankers_runtime::process_jobs::ProcessJobGarbageCollectionFailure;
use clankers_runtime::process_jobs::ProcessJobGarbageCollectionReceipt;
use clankers_runtime::process_jobs::ProcessJobId;
use clankers_runtime::process_jobs::ProcessJobIdentityEnvelope;
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
use clankers_runtime::process_jobs::ProcessJobProfileReceiptMetadata;
use clankers_runtime::process_jobs::ProcessJobReceipt;
use clankers_runtime::process_jobs::ProcessJobRedactionPolicy;
use clankers_runtime::process_jobs::ProcessJobReleasedLogRef;
use clankers_runtime::process_jobs::ProcessJobRetentionPolicy;
use clankers_runtime::process_jobs::ProcessJobService;
use clankers_runtime::process_jobs::ProcessJobStatus;
use clankers_runtime::process_jobs::ProcessJobStream;
use clankers_runtime::process_jobs::ProcessJobSummary;
use clankers_runtime::process_jobs::ProcessJobToolReceipt;
use clankers_runtime::process_jobs::ProcessJobToolRequest;
use clankers_runtime::process_jobs::ProcessJobToolResult;
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
use clankers_util::ansi::strip_ansi;

mod adapter;
use adapter::ProcessToolJsonAdapter;

const DEFAULT_LOG_LIMIT: usize = 200;
const MAX_COMMAND_PREVIEW_LEN: usize = 200;
const MAX_NATIVE_ACTIVE_PROCESS_JOBS: usize = 32;
const NATIVE_KILL_GRACE: Duration = Duration::from_secs(2);
const NATIVE_RESTART_TERMINATION_TIMEOUT: Duration = Duration::from_secs(5);
const NATIVE_RESTART_TERMINATION_POLL: Duration = Duration::from_millis(50);
const ADOPTED_NATIVE_ID_PREFIX: &str = "native_pid_";

fn unsupported_backend_receipt(
    operation: ProcessJobOperation,
    id: Option<ProcessJobId>,
    backend: ProcessJobBackendKind,
    message: impl Into<String>,
) -> ProcessJobReceipt {
    ProcessJobBackendCapabilities::for_backend(backend).unsupported_receipt(operation, id, message)
}

fn unsupported_gc_receipt(
    backend: ProcessJobBackendKind,
    message: impl Into<String>,
) -> ProcessJobGarbageCollectionReceipt {
    let message = message.into();
    let mut receipt = ProcessJobGarbageCollectionReceipt::empty();
    receipt.failures.push(ProcessJobGarbageCollectionFailure {
        id: None,
        reference: Some(backend.label().to_string()),
        message,
    });
    receipt.refresh_summary();
    receipt
}

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
    restart_request: StartProcessJobRequest,
    started_at: Instant,
    started_at_wall: DateTime<Utc>,
    backend_ref: Option<BackendRef>,
    profile: Option<ProcessJobProfileReceiptMetadata>,
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
        restart_request: StartProcessJobRequest,
        stdin: Option<ChildStdin>,
        kill_tx: oneshot::Sender<()>,
        pid: Option<u32>,
        notification_policy: ProcessJobNotificationPolicy,
        profile: Option<ProcessJobProfileReceiptMetadata>,
    ) -> Self {
        Self {
            id,
            command,
            restart_request,
            started_at: Instant::now(),
            started_at_wall: Utc::now(),
            backend_ref: pid.map(|pid| BackendRef(format!("pid:{pid}"))),
            profile,
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
            command_preview: ProcessJobRedactionPolicy::default().safe_command_preview(&self.command),
            cwd: clankers_runtime::process_jobs::ProcessJobCwd::Inherited,
            started_at: Some(self.started_at_wall),
            updated_at: Utc::now(),
            completed_at: self.status().is_done().then(Utc::now),
            log_refs: Vec::new(),
            profile: self.profile.clone(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct NativeProcessJobService {
    db: Option<clankers_db::Db>,
    retention_policy: ProcessJobRetentionPolicy,
    log_dir: Option<PathBuf>,
}

impl NativeProcessJobService {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn with_retention(
        db: clankers_db::Db,
        retention_policy: ProcessJobRetentionPolicy,
        log_dir: Option<PathBuf>,
    ) -> Self {
        Self {
            db: Some(db),
            retention_policy,
            log_dir,
        }
    }
}

impl Default for NativeProcessJobService {
    fn default() -> Self {
        Self {
            db: None,
            retention_policy: ProcessJobRetentionPolicy::default(),
            log_dir: None,
        }
    }
}

#[async_trait]
impl ProcessJobService for NativeProcessJobService {
    async fn start(&self, request: StartProcessJobRequest) -> Result<ProcessJobReceipt, RuntimeError> {
        if request.backend != ProcessJobBackendKind::Native {
            return Ok(unsupported_backend_receipt(
                ProcessJobOperation::Start,
                None,
                request.backend,
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
        let id = ProcessTool::next_native_job_id(&request);
        let id_string = id.0.clone();
        let entry = Arc::new(ProcessEntry::new(
            id_string.clone(),
            display_command,
            request.clone(),
            stdin,
            kill_tx,
            pid,
            request.notification_policy.clone(),
            ProcessJobProfileReceiptMetadata::from_metadata(&request.metadata),
        ));
        let backend_ref = entry.backend_ref.clone();
        ProcessTool::insert(entry.clone());
        admission.release();
        spawn_reader(entry.clone(), "stdout", stdout);
        spawn_reader(entry, "stderr", stderr);
        spawn_waiter(ProcessTool::get(&id_string).expect("inserted native process entry"), child, pid, kill_rx, None);

        Ok(ProcessJobReceipt {
            operation: ProcessJobOperation::Start,
            id: Some(id.clone()),
            backend: Some(ProcessJobBackendKind::Native),
            status: Some(ProcessJobStatus::Running),
            backend_ref,
            log_refs: Vec::new(),
            profile: ProcessJobProfileReceiptMetadata::from_metadata(&request.metadata),
            summary: format!(
                "Started background process {} (pid: {})",
                id.0,
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
            profile: entry.profile.clone(),
            summary: if output.is_empty() {
                "No new output.".to_string()
            } else {
                ProcessJobRedactionPolicy::default().safe_log_excerpt(&output.join("\n"))
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
            summary.push_str(&ProcessJobRedactionPolicy::default().safe_log_excerpt(&output.join("\n")));
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
        restart_native_process_job(id, None, None, None).await
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
            return Ok(unsupported_backend_receipt(
                ProcessJobOperation::Adopt,
                None,
                request.backend,
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
                profile: None,
                summary: format!("native pid {pid} is not signalable; refusing adoption"),
                error: Some(ProcessJobError {
                    code: ProcessJobErrorCode::NotFound,
                    operation: ProcessJobOperation::Adopt,
                    id: None,
                    backend: Some(ProcessJobBackendKind::Native),
                    action: Some("adopt".to_string()),
                    capability_detail: None,
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
            profile: None,
            summary: format!(
                "Adopted native pid {pid} as metadata-only process job; live stdout/stderr streams are unavailable"
            ),
            error: None,
        })
    }

    async fn garbage_collect(
        &self,
        mut filter: ProcessJobFilter,
    ) -> Result<ProcessJobGarbageCollectionReceipt, RuntimeError> {
        if let Some(backend) = filter.backend
            && backend != ProcessJobBackendKind::Native
        {
            return Ok(unsupported_gc_receipt(
                backend,
                "native process service only garbage-collects native process records",
            ));
        }
        filter.backend = Some(ProcessJobBackendKind::Native);
        Ok(
            apply_process_job_retention(self.db.as_ref(), self.retention_policy.clone(), self.log_dir.clone(), filter)
                .await,
        )
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
            return Ok(unsupported_backend_receipt(
                ProcessJobOperation::Start,
                None,
                request.backend,
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
            profile: ProcessJobProfileReceiptMetadata::from_metadata(&request.metadata),
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
            profile: None,
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
            profile: None,
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
        Ok(unsupported_backend_receipt(
            ProcessJobOperation::WriteStdin,
            Some(id),
            ProcessJobBackendKind::Pueue,
            "pueue backend stdin mutation is not supported by the process tool",
        ))
    }

    async fn close_stdin(&self, id: ProcessJobId) -> Result<ProcessJobReceipt, RuntimeError> {
        Ok(unsupported_backend_receipt(
            ProcessJobOperation::CloseStdin,
            Some(id),
            ProcessJobBackendKind::Pueue,
            "pueue backend stdin close is not supported by the process tool",
        ))
    }

    async fn adopt(&self, request: AdoptProcessJobRequest) -> Result<ProcessJobReceipt, RuntimeError> {
        if request.backend != ProcessJobBackendKind::Pueue {
            return Ok(unsupported_backend_receipt(
                ProcessJobOperation::Adopt,
                None,
                request.backend,
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

    async fn garbage_collect(
        &self,
        _filter: ProcessJobFilter,
    ) -> Result<ProcessJobGarbageCollectionReceipt, RuntimeError> {
        Ok(unsupported_gc_receipt(
            ProcessJobBackendKind::Pueue,
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
            profile: None,
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
            profile: None,
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
    ProcessJobReceipt::backend_unavailable(operation, ProcessJobBackendKind::Pueue, reason)
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
        .or_else(|| value.get(task_id.to_string()))
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
            return Ok(unsupported_backend_receipt(
                ProcessJobOperation::Start,
                None,
                request.backend,
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
            profile: ProcessJobProfileReceiptMetadata::from_metadata(&request.metadata),
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
            profile: None,
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
            profile: None,
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
        Ok(unsupported_backend_receipt(
            ProcessJobOperation::WriteStdin,
            Some(id),
            ProcessJobBackendKind::Systemd,
            "systemd backend stdin mutation is not supported by the process tool",
        ))
    }

    async fn close_stdin(&self, id: ProcessJobId) -> Result<ProcessJobReceipt, RuntimeError> {
        Ok(unsupported_backend_receipt(
            ProcessJobOperation::CloseStdin,
            Some(id),
            ProcessJobBackendKind::Systemd,
            "systemd backend stdin close is not supported by the process tool",
        ))
    }

    async fn adopt(&self, request: AdoptProcessJobRequest) -> Result<ProcessJobReceipt, RuntimeError> {
        if request.backend != ProcessJobBackendKind::Systemd {
            return Ok(unsupported_backend_receipt(
                ProcessJobOperation::Adopt,
                None,
                request.backend,
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

    async fn garbage_collect(
        &self,
        _filter: ProcessJobFilter,
    ) -> Result<ProcessJobGarbageCollectionReceipt, RuntimeError> {
        Ok(unsupported_gc_receipt(
            ProcessJobBackendKind::Systemd,
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
            profile: None,
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
            profile: None,
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
    ProcessJobReceipt::backend_unavailable(operation, ProcessJobBackendKind::Systemd, reason)
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
        profile: entry.profile.clone(),
        summary: summary.into(),
        error: None,
    }
}

fn native_restart_failed_receipt(id: ProcessJobId, message: impl Into<String>) -> ProcessJobReceipt {
    let message = message.into();
    ProcessJobReceipt {
        operation: ProcessJobOperation::Restart,
        id: Some(id.clone()),
        backend: Some(ProcessJobBackendKind::Native),
        status: None,
        backend_ref: None,
        log_refs: Vec::new(),
        profile: None,
        summary: message.clone(),
        error: Some(ProcessJobError {
            code: ProcessJobErrorCode::BackendFailed,
            operation: ProcessJobOperation::Restart,
            id: Some(id),
            backend: Some(ProcessJobBackendKind::Native),
            action: Some(ProcessJobOperation::Restart.action_name().to_string()),
            capability_detail: None,
            message,
        }),
    }
}

async fn stop_native_entry_for_restart(entry: &ProcessEntry) -> bool {
    if entry.status().is_done() {
        return true;
    }

    let tx = entry.kill_tx.lock().expect("process kill lock poisoned").take();
    if let Some(tx) = tx {
        tx.send(()).ok();
    }

    let deadline = Instant::now() + NATIVE_RESTART_TERMINATION_TIMEOUT;
    while !entry.status().is_done() {
        if Instant::now() >= deadline {
            return false;
        }
        tokio::time::sleep(NATIVE_RESTART_TERMINATION_POLL).await;
    }
    true
}

async fn restart_native_process_job(
    id: ProcessJobId,
    db: Option<clankers_db::Db>,
    process_monitor: Option<&clankers_procmon::ProcessMonitorHandle>,
    call_id: Option<&str>,
) -> Result<ProcessJobReceipt, RuntimeError> {
    let old_entry = native_entry(&id)?;
    let restart_request = old_entry.restart_request.clone();
    let previous_status = old_entry.status();
    if !stop_native_entry_for_restart(&old_entry).await {
        return Ok(native_restart_failed_receipt(
            id,
            format!("native process restart could not stop previous process before relaunch: {}", old_entry.id),
        ));
    }

    let admission = match ProcessTool::reserve_native_start() {
        Ok(admission) => admission,
        Err(decision) => return Ok(ProcessTool::admission_denied_receipt_for(ProcessJobOperation::Restart, decision)),
    };
    let (display_command, mut child) = spawn_from_start_request(&restart_request)?;
    let pid = child.id();
    let stdin = child.stdin.take();
    let stdout = child.stdout.take().ok_or_else(|| {
        RuntimeError::InvalidTool("failed to capture stdout from restarted native background process".to_string())
    })?;
    let stderr = child.stderr.take().ok_or_else(|| {
        RuntimeError::InvalidTool("failed to capture stderr from restarted native background process".to_string())
    })?;
    let (kill_tx, kill_rx) = oneshot::channel();
    let new_entry = Arc::new(ProcessEntry::new(
        id.0.clone(),
        display_command,
        restart_request,
        stdin,
        kill_tx,
        pid,
        old_entry.notification_policy.clone(),
        old_entry.profile.clone(),
    ));
    let backend_ref = new_entry.backend_ref.clone();
    ProcessTool::insert(new_entry.clone());
    admission.release();
    if let Some(monitor) = process_monitor
        && let Some(pid) = pid
    {
        monitor.register(pid, clankers_procmon::ProcessMeta {
            tool_name: "process".to_string(),
            command: ProcessJobRedactionPolicy::default().safe_command_preview(&new_entry.command),
            call_id: call_id.unwrap_or("process-restart").to_string(),
        });
    }
    persist_entry(db.as_ref(), &new_entry).await;
    spawn_reader(new_entry.clone(), "stdout", stdout);
    spawn_reader(new_entry.clone(), "stderr", stderr);
    spawn_waiter(new_entry, child, pid, kill_rx, db);

    Ok(ProcessJobReceipt {
        operation: ProcessJobOperation::Restart,
        id: Some(id.clone()),
        backend: Some(ProcessJobBackendKind::Native),
        status: Some(ProcessJobStatus::Running),
        backend_ref,
        log_refs: Vec::new(),
        profile: old_entry.profile.clone(),
        summary: format!("Restarted native process {} (previous status: {})", id.0, previous_status.label()),
        error: None,
    })
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
        command_preview: ProcessJobRedactionPolicy::default().safe_command_preview(&entry.command),
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
            can_restart: true,
            can_write_stdin: true,
            can_select_backend: false,
        },
        safe_metadata: std::collections::BTreeMap::default(),
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

fn durable_reconciliation_note(record: &StoredProcessJobRecord) -> String {
    let status = stored_status_label(&record.status);
    let reconciliation = record.safe_metadata.get("reconciliation").map_or("unknown", String::as_str);
    match &record.status {
        StoredProcessJobStatus::ReattachedLogIncomplete => {
            format!("degraded reconciliation: status={status}, state={reconciliation}; live stdio was not reattached")
        }
        StoredProcessJobStatus::LostAfterRestart => format!(
            "degraded reconciliation: status={status}, state={reconciliation}; process identity was not safely recoverable"
        ),
        StoredProcessJobStatus::BackendUnavailable { .. } => {
            format!("degraded reconciliation: status={status}, state={reconciliation}; backend was unavailable")
        }
        _ => format!("durable reconciliation: status={status}, state={reconciliation}"),
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
        profile: None,
    }
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
    if reconciled != record
        && let Err(error) = db.async_process_jobs().upsert(reconciled.clone()).await
    {
        tracing::warn!("failed to persist reconciled process job metadata: {error}");
        return record;
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

fn record_matches_retention_filter(record: &StoredProcessJobRecord, filter: &ProcessJobFilter) -> bool {
    filter.backend.is_none_or(|backend| backend == stored_backend_to_job_backend(record.backend))
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

fn log_reference_degradation_detail(record: &StoredProcessJobRecord, log_dir: Option<&PathBuf>) -> Option<String> {
    let mut unavailable = Vec::new();
    if record.log_refs.is_empty() {
        unavailable.push("log_unavailable:no_log_refs".to_string());
    }
    for log_ref in &record.log_refs {
        if log_ref.reference.starts_with("native:") {
            match safe_native_log_path(log_dir, &log_ref.reference) {
                Some(path) if path.exists() => {}
                Some(path) => unavailable.push(format!(
                    "log_unavailable:native_missing:{} ({})",
                    log_ref.reference,
                    path.display()
                )),
                None => unavailable.push(format!("log_unavailable:native_unresolved:{}", log_ref.reference)),
            }
        } else {
            unavailable.push(format!("log_unavailable:backend_ref_unresolved:{}", log_ref.reference));
        }
    }
    (!unavailable.is_empty()).then(|| unavailable.join("; "))
}

fn append_log_degradation(summary: &mut ProcessJobSummary, record: &StoredProcessJobRecord, log_dir: Option<&PathBuf>) {
    if let Some(detail) = log_reference_degradation_detail(record, log_dir) {
        summary.command_preview = format!("{} [{detail}]", summary.command_preview);
    }
}

fn durable_degraded_log_message(record: &StoredProcessJobRecord, log_dir: Option<&PathBuf>) -> String {
    let detail = log_reference_degradation_detail(record, log_dir)
        .unwrap_or_else(|| "log_unavailable:live_output_stream_detached".to_string());
    format!(
        "process job {}; {}; {detail}; durable log refs: {}",
        record.id,
        durable_reconciliation_note(record),
        format_log_refs(&record.log_refs)
    )
}

async fn apply_process_job_retention(
    db: Option<&clankers_db::Db>,
    policy: ProcessJobRetentionPolicy,
    log_dir: Option<PathBuf>,
    filter: ProcessJobFilter,
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
        if !record_matches_retention_filter(record, &filter) {
            continue;
        }
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
                    Ok(()) => {
                        receipt.deleted_native_log_files = receipt.deleted_native_log_files.saturating_add(1);
                    }
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
    process_monitor: Option<clankers_procmon::ProcessMonitorHandle>,
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
                            "description": "Native log directory override for gc/log/poll/list degradation checks; defaults to CLANKERS_PROCESS_JOB_LOG_DIR"
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

    pub fn with_process_monitor(mut self, monitor: clankers_procmon::ProcessMonitorHandle) -> Self {
        self.process_monitor = Some(monitor);
        self
    }

    fn next_native_job_id(request: &StartProcessJobRequest) -> ProcessJobId {
        let mut registry = REGISTRY.lock().expect("process registry lock poisoned");
        registry.next_id += 1;
        let request_nonce = format!("native:{}", registry.next_id);
        ProcessJobIdentityEnvelope::for_start_request(request, request_nonce).derive_id()
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

    fn is_current_entry(entry: &Arc<ProcessEntry>) -> bool {
        Self::get(&entry.id).is_some_and(|current| Arc::ptr_eq(&current, entry))
    }

    fn admission_denied_receipt(decision: NativeAdmissionDecision) -> ProcessJobReceipt {
        Self::admission_denied_receipt_for(ProcessJobOperation::Start, decision)
    }

    fn admission_denied_receipt_for(
        operation: ProcessJobOperation,
        decision: NativeAdmissionDecision,
    ) -> ProcessJobReceipt {
        let summary = decision.summary();
        ProcessJobReceipt {
            operation,
            id: None,
            backend: Some(ProcessJobBackendKind::Native),
            status: Some(ProcessJobStatus::Waiting),
            backend_ref: None,
            log_refs: Vec::new(),
            profile: None,
            summary: summary.clone(),
            error: Some(ProcessJobError {
                code: ProcessJobErrorCode::ConcurrencyLimitExceeded,
                operation,
                id: None,
                backend: Some(ProcessJobBackendKind::Native),
                action: Some(operation.action_name().to_string()),
                capability_detail: None,
                message: summary,
            }),
        }
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

    fn process_job_tool_request(params: &Value) -> Result<ProcessJobToolRequest, ToolResult> {
        ProcessToolJsonAdapter::process_job_tool_request(params)
    }

    fn receipt_result(receipt: ProcessJobReceipt) -> ToolResult {
        let result = match receipt.operation {
            ProcessJobOperation::Start => ProcessJobToolResult::Start(receipt),
            ProcessJobOperation::Poll => ProcessJobToolResult::Poll(receipt),
            ProcessJobOperation::Wait => ProcessJobToolResult::Wait(receipt),
            ProcessJobOperation::Kill => ProcessJobToolResult::Kill(receipt),
            ProcessJobOperation::Restart => ProcessJobToolResult::Restart(receipt),
            ProcessJobOperation::WriteStdin => ProcessJobToolResult::WriteStdin(receipt),
            ProcessJobOperation::CloseStdin => ProcessJobToolResult::CloseStdin(receipt),
            ProcessJobOperation::Adopt => ProcessJobToolResult::Adopt(receipt),
            ProcessJobOperation::List | ProcessJobOperation::Log | ProcessJobOperation::GarbageCollect => {
                ProcessJobToolResult::Poll(receipt)
            }
        };
        Self::tool_receipt_result(result)
    }

    fn tool_receipt_result(result: ProcessJobToolResult) -> ToolResult {
        let receipt: ProcessJobToolReceipt = result.into_receipt();
        let payload = serde_json::to_string(&receipt).unwrap_or(receipt.common.summary.clone());
        if receipt.common.error.is_some() {
            ToolResult::error(payload)
        } else {
            ToolResult::text(payload)
        }
    }

    #[allow(
        clippy::unused_async,
        reason = "process backends share async result helpers with the tool dispatch shell"
    )]
    async fn pueue_receipt_result(result: Result<ProcessJobReceipt, RuntimeError>) -> ToolResult {
        match result {
            Ok(receipt) => Self::receipt_result(receipt),
            Err(error) => ToolResult::error(error.to_string()),
        }
    }

    #[allow(
        clippy::unused_async,
        reason = "process backends share async result helpers with the tool dispatch shell"
    )]
    async fn systemd_receipt_result(result: Result<ProcessJobReceipt, RuntimeError>) -> ToolResult {
        match result {
            Ok(receipt) => Self::receipt_result(receipt),
            Err(error) => ToolResult::error(error.to_string()),
        }
    }

    async fn handle_start(&self, ctx: &ToolContext, params: &Value) -> ToolResult {
        let request = match Self::process_job_tool_request(params) {
            Ok(ProcessJobToolRequest::Start(request)) => request,
            Ok(_) => return ToolResult::error("Parsed unexpected process job request for start action"),
            Err(result) => return result,
        };
        let backend = request.backend;
        if backend == ProcessJobBackendKind::Pueue {
            return match Self::pueue_service().start(request).await {
                Ok(receipt) => Self::tool_receipt_result(ProcessJobToolResult::Start(receipt)),
                Err(error) => ToolResult::error(error.to_string()),
            };
        }
        if backend == ProcessJobBackendKind::Systemd {
            return match Self::systemd_service().start(request).await {
                Ok(receipt) => Self::tool_receipt_result(ProcessJobToolResult::Start(receipt)),
                Err(error) => ToolResult::error(error.to_string()),
            };
        }
        let admission = match Self::reserve_native_start() {
            Ok(admission) => admission,
            Err(decision) => {
                let receipt = Self::admission_denied_receipt(decision);
                let payload = serde_json::to_string(&receipt).unwrap_or_else(|_| receipt.summary.clone());
                return ToolResult::error(payload);
            }
        };

        let (display_command, mut child) = match spawn_from_start_request(&request) {
            Ok(spec) => spec,
            Err(error) => return ToolResult::error(error.to_string()),
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
        let notification_policy = request.notification_policy.clone();
        let (kill_tx, kill_rx) = oneshot::channel();
        let id = Self::next_native_job_id(&request).0;
        let entry = Arc::new(ProcessEntry::new(
            id.clone(),
            display_command.clone(),
            request.clone(),
            stdin,
            kill_tx,
            pid,
            notification_policy,
            ProcessJobProfileReceiptMetadata::from_metadata(&request.metadata),
        ));
        Self::insert(entry.clone());
        admission.release();

        if let Some(ref monitor) = self.process_monitor
            && let Some(pid) = pid
        {
            let command_preview: String = display_command.chars().take(MAX_COMMAND_PREVIEW_LEN).collect();
            monitor.register(pid, clankers_procmon::ProcessMeta {
                tool_name: "process".to_string(),
                command: command_preview,
                call_id: ctx.call_id.clone(),
            });
        }

        persist_entry(ctx.db(), &entry).await;
        spawn_reader(entry.clone(), "stdout", stdout);
        spawn_reader(entry.clone(), "stderr", stderr);
        spawn_waiter(entry.clone(), child, pid, kill_rx, ctx.db().cloned());

        let receipt = ProcessJobReceipt {
            operation: ProcessJobOperation::Start,
            id: Some(ProcessJobId(id.clone())),
            backend: Some(ProcessJobBackendKind::Native),
            status: Some(ProcessJobStatus::Running),
            backend_ref: pid.map(|pid| BackendRef(format!("pid:{pid}"))),
            log_refs: Vec::new(),
            profile: ProcessJobProfileReceiptMetadata::from_metadata(&request.metadata),
            summary: format!(
                "Started background process {id} (pid: {})",
                pid.map(|p| p.to_string()).unwrap_or_else(|| "unknown".to_string())
            ),
            error: None,
        };
        Self::receipt_result(receipt)
    }

    async fn handle_list(ctx: &ToolContext, params: &Value) -> ToolResult {
        let request = match Self::process_job_tool_request(params) {
            Ok(ProcessJobToolRequest::List(request)) => request,
            Ok(_) => return ToolResult::error("Parsed unexpected process job request for list action"),
            Err(result) => return result,
        };
        let policy = process_job_retention_policy(&json!({}));
        let log_dir = retention_log_dir(params);
        let _ = apply_process_job_retention(ctx.db(), policy, log_dir.clone(), ProcessJobFilter::default()).await;
        let backend_filter = request.filter.backend;
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
            summaries.extend(
                entries
                    .into_iter()
                    .map(|entry| entry.summary())
                    .filter(|summary| request.filter.include_terminal || !summary.status.is_terminal()),
            );
            summaries.extend(
                durable
                    .iter()
                    .map(|record| {
                        let mut summary = stored_record_summary(record);
                        append_log_degradation(&mut summary, record, log_dir.as_ref());
                        summary
                    })
                    .filter(|summary| request.filter.include_terminal || !summary.status.is_terminal()),
            );
        }
        if backend_filter.is_none_or(|backend| backend == ProcessJobBackendKind::Pueue) {
            match Self::pueue_service()
                .list(ProcessJobFilter {
                    backend: Some(ProcessJobBackendKind::Pueue),
                    include_terminal: request.filter.include_terminal,
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
                    include_terminal: request.filter.include_terminal,
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
        Self::tool_receipt_result(ProcessJobToolResult::List(summaries))
    }

    async fn handle_gc(ctx: &ToolContext, params: &Value) -> ToolResult {
        let request = match Self::process_job_tool_request(params) {
            Ok(ProcessJobToolRequest::GarbageCollect(request)) => request,
            Ok(_) => return ToolResult::error("Parsed unexpected process job request for garbage_collect action"),
            Err(result) => return result,
        };
        let policy = process_job_retention_policy(params);
        let receipt = apply_process_job_retention(ctx.db(), policy, retention_log_dir(params), request.filter).await;
        Self::tool_receipt_result(ProcessJobToolResult::GarbageCollect(receipt))
    }

    async fn handle_poll(ctx: &ToolContext, params: &Value) -> ToolResult {
        let request = match Self::process_job_tool_request(params) {
            Ok(ProcessJobToolRequest::Poll(request)) => request,
            Ok(_) => return ToolResult::error("Parsed unexpected process job request for poll action"),
            Err(result) => return result,
        };
        let session_id = request.id.0.clone();
        if let Some(id) = Self::pueue_id(&session_id) {
            return match Self::pueue_service().poll(id, request.cursor).await {
                Ok(receipt) => Self::pueue_receipt_result(Ok(receipt)).await,
                Err(error) => match durable_record(ctx.db(), &session_id).await {
                    Some(record) => ToolResult::text(format!(
                        "{}; backend poll unavailable: {error}",
                        durable_degraded_log_message(&record, retention_log_dir(params).as_ref())
                    )),
                    None => ToolResult::error(error.to_string()),
                },
            };
        }
        if let Some(id) = Self::systemd_id(&session_id) {
            return match Self::systemd_service().poll(id, request.cursor).await {
                Ok(receipt) => Self::systemd_receipt_result(Ok(receipt)).await,
                Err(error) => match durable_record(ctx.db(), &session_id).await {
                    Some(record) => ToolResult::text(format!(
                        "{}; backend poll unavailable: {error}",
                        durable_degraded_log_message(&record, retention_log_dir(params).as_ref())
                    )),
                    None => ToolResult::error(error.to_string()),
                },
            };
        }
        let entry = match Self::get(&session_id) {
            Some(entry) => entry,
            None => {
                if let Some(record) = durable_record(ctx.db(), &session_id).await {
                    return ToolResult::text(durable_degraded_log_message(&record, retention_log_dir(params).as_ref()));
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
                let _ = write!(text, "\n- {} {}: {}", notification.event_id.0, kind, notification.summary);
            }
        }
        ToolResult::text(text)
    }

    async fn handle_log(ctx: &ToolContext, params: &Value) -> ToolResult {
        let request = match Self::process_job_tool_request(params) {
            Ok(ProcessJobToolRequest::Log(request)) => request,
            Ok(_) => return ToolResult::error("Parsed unexpected process job request for log action"),
            Err(result) => return result,
        };
        let session_id = request.id.0.clone();
        if let Some(id) = Self::pueue_id(&session_id) {
            return match Self::pueue_service().log(id, request.range.clone()).await {
                Ok(chunk) if chunk.text.is_empty() => match durable_record(ctx.db(), &session_id).await {
                    Some(record) => ToolResult::text(format!(
                        "{}; backend log read returned no output",
                        durable_degraded_log_message(&record, retention_log_dir(params).as_ref())
                    )),
                    None => Self::tool_receipt_result(ProcessJobToolResult::Log(chunk)),
                },
                Ok(chunk) => Self::tool_receipt_result(ProcessJobToolResult::Log(chunk)),
                Err(error) => match durable_record(ctx.db(), &session_id).await {
                    Some(record) => ToolResult::text(format!(
                        "{}; backend log read unavailable: {error}",
                        durable_degraded_log_message(&record, retention_log_dir(params).as_ref())
                    )),
                    None => ToolResult::error(error.to_string()),
                },
            };
        }
        if let Some(id) = Self::systemd_id(&session_id) {
            return match Self::systemd_service().log(id, request.range.clone()).await {
                Ok(chunk) if chunk.text.is_empty() => match durable_record(ctx.db(), &session_id).await {
                    Some(record) => ToolResult::text(format!(
                        "{}; backend log read returned no output",
                        durable_degraded_log_message(&record, retention_log_dir(params).as_ref())
                    )),
                    None => Self::tool_receipt_result(ProcessJobToolResult::Log(chunk)),
                },
                Ok(chunk) => Self::tool_receipt_result(ProcessJobToolResult::Log(chunk)),
                Err(error) => match durable_record(ctx.db(), &session_id).await {
                    Some(record) => ToolResult::text(format!(
                        "{}; backend log read unavailable: {error}",
                        durable_degraded_log_message(&record, retention_log_dir(params).as_ref())
                    )),
                    None => ToolResult::error(error.to_string()),
                },
            };
        }
        let entry = match Self::get(&session_id) {
            Some(entry) => entry,
            None => {
                if let Some(record) = durable_record(ctx.db(), &session_id).await {
                    return ToolResult::text(durable_degraded_log_message(&record, retention_log_dir(params).as_ref()));
                }
                return ToolResult::error(format!("Unknown process session_id: {session_id}"));
            }
        };
        persist_entry(ctx.db(), &entry).await;
        let output = entry.snapshot_output();
        let limit = usize::try_from(request.range.limit_bytes).unwrap_or(DEFAULT_LOG_LIMIT).min(DEFAULT_LOG_LIMIT);
        let start = request
            .range
            .offset
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
        let request = match Self::process_job_tool_request(params) {
            Ok(ProcessJobToolRequest::Wait(request)) => request,
            Ok(_) => return ToolResult::error("Parsed unexpected process job request for wait action"),
            Err(result) => return result,
        };
        let session_id = request.id.0.clone();
        if let Some(id) = Self::pueue_id(&session_id) {
            return Self::pueue_receipt_result(Self::pueue_service().wait(id, request.timeout).await).await;
        }
        if let Some(id) = Self::systemd_id(&session_id) {
            return Self::systemd_receipt_result(Self::systemd_service().wait(id, request.timeout).await).await;
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
        let timeout_secs = request.timeout.unwrap_or(Duration::from_secs(30)).as_secs();
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
            text.push_str(&ProcessJobRedactionPolicy::default().safe_log_excerpt(&output.join("\n")));
        }
        ToolResult::text(text)
    }

    async fn handle_kill(ctx: &ToolContext, params: &Value) -> ToolResult {
        let request = match Self::process_job_tool_request(params) {
            Ok(ProcessJobToolRequest::Kill(request)) => request,
            Ok(_) => return ToolResult::error("Parsed unexpected process job request for kill action"),
            Err(result) => return result,
        };
        let session_id = request.id.0.clone();
        if let Some(id) = Self::pueue_id(&session_id) {
            return Self::pueue_receipt_result(Self::pueue_service().kill(id).await).await;
        }
        if let Some(id) = Self::systemd_id(&session_id) {
            return Self::systemd_receipt_result(Self::systemd_service().kill(id).await).await;
        }
        let entry = match Self::get(&session_id) {
            Some(entry) => entry,
            None => {
                if let Some(record) = durable_record(ctx.db(), &session_id).await {
                    return ToolResult::text(format!(
                        "{}; kill not sent because no live process handle is attached (refs: {}).",
                        durable_reconciliation_note(&record),
                        format_log_refs(&record.log_refs)
                    ));
                }
                return ToolResult::error(format!("Unknown process session_id: {session_id}"));
            }
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
        let request = match Self::process_job_tool_request(params) {
            Ok(ProcessJobToolRequest::WriteStdin(request)) => request,
            Ok(_) => return ToolResult::error("Parsed unexpected process job request for write_stdin action"),
            Err(result) => return result,
        };
        debug_assert_eq!(request.newline, newline);
        let session_id = request.id.0.clone();
        if let Some(id) = Self::pueue_id(&session_id) {
            return Self::pueue_receipt_result(
                Self::pueue_service().write_stdin(id, request.data.clone(), request.newline).await,
            )
            .await;
        }
        if let Some(id) = Self::systemd_id(&session_id) {
            return Self::systemd_receipt_result(
                Self::systemd_service().write_stdin(id, request.data.clone(), request.newline).await,
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
        if let Err(e) = stdin.write_all(&request.data).await {
            return ToolResult::error(format!("Failed to write stdin for {}: {e}", entry.id));
        }
        if request.newline
            && let Err(e) = stdin.write_all(b"\n").await
        {
            return ToolResult::error(format!("Failed to write newline for {}: {e}", entry.id));
        }
        if let Err(e) = stdin.flush().await {
            return ToolResult::error(format!("Failed to flush stdin for {}: {e}", entry.id));
        }
        ToolResult::text(format!("Wrote {} bytes to {}", request.data.len() + usize::from(request.newline), entry.id))
    }

    async fn handle_close(params: &Value) -> ToolResult {
        let request = match Self::process_job_tool_request(params) {
            Ok(ProcessJobToolRequest::CloseStdin(request)) => request,
            Ok(_) => return ToolResult::error("Parsed unexpected process job request for close_stdin action"),
            Err(result) => return result,
        };
        let session_id = request.id.0.clone();
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
        let request = match Self::process_job_tool_request(params) {
            Ok(ProcessJobToolRequest::Adopt(request)) => request,
            Ok(_) => return ToolResult::error("Parsed unexpected process job request for adopt action"),
            Err(result) => return result,
        };
        match request.backend {
            ProcessJobBackendKind::Native => match NativeProcessJobService::default().adopt(request).await {
                Ok(receipt) => Self::receipt_result(receipt),
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

    async fn handle_restart(&self, ctx: &ToolContext, params: &Value) -> ToolResult {
        let request = match Self::process_job_tool_request(params) {
            Ok(ProcessJobToolRequest::Restart(request)) => request,
            Ok(_) => return ToolResult::error("Parsed unexpected process job request for restart action"),
            Err(result) => return result,
        };
        let session_id = request.id.0.clone();
        if let Some(id) = Self::pueue_id(&session_id) {
            return Self::pueue_receipt_result(Self::pueue_service().restart(id).await).await;
        }
        if let Some(id) = Self::systemd_id(&session_id) {
            return Self::systemd_receipt_result(Self::systemd_service().restart(id).await).await;
        }
        match restart_native_process_job(
            request.id,
            ctx.db().cloned(),
            self.process_monitor.as_ref(),
            Some(ctx.call_id.as_str()),
        )
        .await
        {
            Ok(receipt) => Self::receipt_result(receipt),
            Err(error) => ToolResult::error(error.to_string()),
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
            "list" => Self::handle_list(ctx, &params).await,
            "poll" => Self::handle_poll(ctx, &params).await,
            "log" => Self::handle_log(ctx, &params).await,
            "wait" => Self::handle_wait(ctx, &params).await,
            "kill" => Self::handle_kill(ctx, &params).await,
            "restart" => self.handle_restart(ctx, &params).await,
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
        if ProcessTool::is_current_entry(&entry) {
            persist_entry(db.as_ref(), &entry).await;
        }
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

    const SHORT_PROCESS_TEST_TIMEOUT_SECS: u64 = 2;
    const RESTART_PERSISTENCE_SETTLE_MILLIS: u64 = 100;
    const GC_TEST_OLD_RECORD_SECS: i64 = 2;
    const GC_TEST_RETENTION_SECS: u64 = 1;
    const GC_TEST_LOG_BYTES: u64 = 12;
    const GC_TEST_MAX_RECORDS: u64 = 100;
    const GC_TEST_LOG_BUDGET_BYTES: u64 = 1_000_000;

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
        if let Ok(payload) = serde_json::from_str::<serde_json::Value>(&text)
            && let Some(id) =
                payload.get("common").and_then(|common| common.get("id")).and_then(serde_json::Value::as_str)
        {
            return id.to_string();
        }
        text.split_whitespace()
            .find(|word| word.starts_with("proc_"))
            .expect("result contains process id")
            .to_string()
    }

    fn tool_receipt_json(result: &ToolResult) -> serde_json::Value {
        serde_json::from_str(&text(result)).expect("tool result is process-job receipt envelope json")
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
            metadata: std::collections::BTreeMap::default(),
        }
    }

    fn adopt_request_for(backend: ProcessJobBackendKind, backend_ref: &str) -> AdoptProcessJobRequest {
        let owner = ProcessJobOwnerScope::DaemonGlobal;
        let mut caller = clankers_runtime::process_jobs::ProcessJobCallerScope {
            capabilities: clankers_runtime::process_jobs::ProcessJobCapabilitySet::full_control(),
            ..clankers_runtime::process_jobs::ProcessJobCallerScope::default()
        };
        caller.daemon_global = true;
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
                Some("kill" | "restart") => Ok(String::new()),
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
        let stdin_error = stdin.error.expect("unsupported receipt");
        assert_eq!(stdin_error.code, ProcessJobErrorCode::UnsupportedActionForBackend);
        assert_eq!(stdin_error.capability_detail.as_deref(), Some("stdin requires stdin support"));

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
        let stdin_error = stdin.error.expect("unsupported receipt");
        assert_eq!(stdin_error.code, ProcessJobErrorCode::UnsupportedActionForBackend);
        assert_eq!(stdin_error.capability_detail.as_deref(), Some("stdin requires stdin support"));

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
        let service = NativeProcessJobService::default();
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
        let mut request = native_start_request("printf service-ok");
        request.metadata.insert("profile".to_string(), "ci-smoke".to_string());
        request.metadata.insert("identity.profile.schema_version".to_string(), "1".to_string());
        request
            .metadata
            .insert("identity.profile.source".to_string(), "workspace:.clankers/process-jobs.json".to_string());
        request.metadata.insert("identity.profile.policy".to_string(), "workspace".to_string());
        let service = NativeProcessJobService::default();
        let started = service.start(request).await.expect("start succeeds");
        assert_eq!(started.backend, Some(ProcessJobBackendKind::Native));
        assert_eq!(started.profile.as_ref().map(|profile| profile.profile_name.as_str()), Some("ci-smoke"));
        let id = started.id.clone().expect("receipt has stable process id");

        let listed = service
            .list(ProcessJobFilter {
                include_terminal: true,
                ..ProcessJobFilter::default()
            })
            .await
            .expect("list succeeds");
        let listed_summary = listed
            .iter()
            .find(|summary| summary.id == id && summary.backend == ProcessJobBackendKind::Native)
            .expect("native process is listed");
        assert_eq!(
            listed_summary.profile.as_ref().map(|profile| profile.profile_source.as_str()),
            Some("workspace:.clankers/process-jobs.json")
        );

        let waited = service.wait(id, Some(Duration::from_secs(2))).await.expect("wait succeeds");
        assert_eq!(waited.profile.as_ref().map(|profile| profile.policy_source.as_str()), Some("workspace"));
        assert!(waited.summary.contains("service-ok"), "{}", waited.summary);
    }

    #[tokio::test]
    async fn native_process_job_service_restarts_running_entry() {
        let service = NativeProcessJobService::default();
        let started = service.start(native_start_request("cat")).await.expect("start succeeds");
        let id = started.id.clone().expect("receipt has stable process id");

        let restarted = service.restart(id.clone()).await.expect("restart succeeds");
        assert_eq!(restarted.operation, ProcessJobOperation::Restart);
        assert_eq!(restarted.id, Some(id.clone()));
        assert_eq!(restarted.status, Some(ProcessJobStatus::Running));

        service
            .write_stdin(id.clone(), b"service-restart".to_vec(), true)
            .await
            .expect("write to restarted stdin succeeds");
        service.close_stdin(id.clone()).await.expect("close restarted stdin succeeds");
        let waited = service
            .wait(id, Some(Duration::from_secs(SHORT_PROCESS_TEST_TIMEOUT_SECS)))
            .await
            .expect("wait succeeds");
        assert!(waited.summary.contains("service-restart"), "{}", waited.summary);
    }

    #[tokio::test]
    async fn native_process_job_service_garbage_collects_completed_native_records() {
        let db = clankers_db::Db::in_memory().expect("db opens");
        let temp = tempfile::tempdir().expect("tempdir");
        let old = Utc::now() - chrono::Duration::seconds(GC_TEST_OLD_RECORD_SECS);
        let mut expired = StoredProcessJobRecord::new_native(
            "proc_service_gc_expired",
            "printf expired",
            StoredProcessJobOwnerScope::DaemonGlobal,
        );
        expired.status = StoredProcessJobStatus::Succeeded { exit_code: Some(0) };
        expired.started_at = old;
        expired.updated_at = old;
        expired.completed_at = Some(old);
        expired.log_refs = vec![StoredProcessJobLogRef {
            stream: StoredProcessJobStream::Combined,
            reference: "native:proc_service_gc_expired/combined.log".to_string(),
            retained_until: None,
            max_bytes: Some(GC_TEST_LOG_BYTES),
        }];
        let log_path = temp.path().join("proc_service_gc_expired").join("combined.log");
        std::fs::create_dir_all(log_path.parent().expect("log parent")).expect("log dir");
        std::fs::write(&log_path, b"expired-log!").expect("log write");

        let mut active = StoredProcessJobRecord::new_native(
            "proc_service_gc_active",
            "sleep active",
            StoredProcessJobOwnerScope::DaemonGlobal,
        );
        active.status = StoredProcessJobStatus::Running;
        active.updated_at = old;
        db.async_process_jobs().upsert(expired).await.expect("insert expired");
        db.async_process_jobs().upsert(active).await.expect("insert active");

        let service = NativeProcessJobService::with_retention(
            db.clone(),
            ProcessJobRetentionPolicy {
                max_age: Some(Duration::from_secs(GC_TEST_RETENTION_SECS)),
                max_records: None,
                max_log_bytes: None,
            },
            Some(temp.path().to_path_buf()),
        );
        let receipt = service
            .garbage_collect(ProcessJobFilter::default())
            .await
            .expect("native service GC returns receipt");

        assert_eq!(receipt.removed_records, vec![ProcessJobId("proc_service_gc_expired".to_string())]);
        assert_eq!(receipt.removed_metadata_count, 1);
        assert_eq!(receipt.deleted_native_log_files, 1);
        assert_eq!(receipt.removed_log_bytes, GC_TEST_LOG_BYTES);
        assert_eq!(receipt.skipped_active_jobs, vec![ProcessJobId("proc_service_gc_active".to_string())]);
        assert!(receipt.failures.is_empty(), "{receipt:?}");
        assert!(db.async_process_jobs().get("proc_service_gc_expired").await.expect("db read").is_none());
        assert!(db.async_process_jobs().get("proc_service_gc_active").await.expect("db read").is_some());
        assert!(!log_path.exists());
    }

    #[tokio::test]
    async fn native_process_job_service_gc_requires_db_and_rejects_foreign_backend_filter() {
        let service = NativeProcessJobService::default();
        let missing_db = service
            .garbage_collect(ProcessJobFilter {
                backend: Some(ProcessJobBackendKind::Native),
                ..ProcessJobFilter::default()
            })
            .await
            .expect("missing db is a typed receipt failure");
        assert_eq!(missing_db.failures.len(), 1);
        assert!(missing_db.failures[0].message.contains("requires a durable process-job database"));

        let unsupported = service
            .garbage_collect(ProcessJobFilter {
                backend: Some(ProcessJobBackendKind::Pueue),
                ..ProcessJobFilter::default()
            })
            .await
            .expect("foreign backend is typed unsupported GC receipt");
        assert_eq!(unsupported.failures.len(), 1);
        assert!(unsupported.failures[0].message.contains("native process service only garbage-collects native"));
    }

    #[tokio::test]
    async fn native_process_job_service_redacts_receipts_and_persisted_metadata() {
        let db = clankers_db::Db::in_memory().expect("db opens");
        let ctx = make_ctx_with_db(db.clone());
        let tool = ProcessTool::new();
        let started = tool
            .execute(
                &ctx,
                json!({"action": "start", "command": "printf 'token=raw-token\n'", "label": "Authorization: Bearer raw-token"}),
            )
            .await;
        assert!(!started.is_error, "{started:?}");
        let id = extract_process_id(&started);
        let waited = tool.execute(&ctx, json!({"action": "wait", "session_id": id, "timeout": 2})).await;
        let waited_text = text(&waited);
        assert!(!waited.is_error, "{waited:?}");
        assert!(waited_text.contains("[REDACTED]"), "{waited_text}");
        assert!(!waited_text.contains("raw-token"), "{waited_text}");

        let stored = db.async_process_jobs().get(id).await.expect("db read").expect("record stored");
        let serialized = serde_json::to_string(&stored).expect("stored record serializes");
        assert!(!serialized.contains("raw-token"), "{serialized}");
        assert!(!serialized.contains("Authorization: Bearer"), "{serialized}");
        assert!(serialized.contains("[REDACTED]"), "{serialized}");
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
        assert!(text(&logged).contains("durable reconciliation"), "{}", text(&logged));
    }

    #[tokio::test]
    async fn durable_degraded_records_project_into_poll_log_and_kill_results() {
        let db = clankers_db::Db::in_memory().expect("db opens");
        let ctx = make_ctx_with_db(db.clone());
        let tool = ProcessTool::new();
        let mut durable_only = StoredProcessJobRecord::new_native(
            "proc_degraded_only",
            "sleep stale",
            StoredProcessJobOwnerScope::DaemonGlobal,
        );
        durable_only.status = StoredProcessJobStatus::LostAfterRestart;
        durable_only.completed_at = Some(Utc::now());
        durable_only.safe_metadata.insert("reconciliation".to_string(), "lost-after-restart".to_string());
        durable_only.log_refs = vec![StoredProcessJobLogRef {
            stream: StoredProcessJobStream::Combined,
            reference: "native:proc_degraded_only/combined.log".to_string(),
            retained_until: None,
            max_bytes: Some(10),
        }];
        db.async_process_jobs().upsert(durable_only).await.expect("insert durable-only record");

        for action in ["poll", "log", "kill"] {
            let result = tool.execute(&ctx, json!({"action": action, "session_id": "proc_degraded_only"})).await;
            let body = text(&result);
            assert!(!result.is_error, "{action}: {result:?}");
            assert!(body.contains("degraded reconciliation"), "{action}: {body}");
            assert!(body.contains("lost-after-restart"), "{action}: {body}");
        }
    }

    #[tokio::test]
    async fn missing_native_log_degrades_list_poll_and_log_without_hiding_metadata() {
        let db = clankers_db::Db::in_memory().expect("db opens");
        let ctx = make_ctx_with_db(db.clone());
        let tool = ProcessTool::new();
        let temp = tempfile::tempdir().expect("tempdir");
        let mut durable_only = StoredProcessJobRecord::new_native(
            "proc_missing_log",
            "printf missing-log",
            StoredProcessJobOwnerScope::DaemonGlobal,
        );
        durable_only.status = StoredProcessJobStatus::LostAfterRestart;
        durable_only.completed_at = Some(Utc::now());
        durable_only.safe_metadata.insert("reconciliation".to_string(), "lost-after-restart".to_string());
        durable_only.log_refs = vec![StoredProcessJobLogRef {
            stream: StoredProcessJobStream::Combined,
            reference: "native:proc_missing_log/combined.log".to_string(),
            retained_until: None,
            max_bytes: Some(10),
        }];
        db.async_process_jobs().upsert(durable_only).await.expect("insert missing-log record");

        for action in ["list", "poll", "log"] {
            let result = tool
                .execute(
                    &ctx,
                    json!({"action": action, "session_id": "proc_missing_log", "log_dir": temp.path().to_string_lossy()}),
                )
                .await;
            let body = text(&result);
            assert!(!result.is_error, "{action}: {result:?}");
            assert!(body.contains("proc_missing_log"), "{action}: {body}");
            assert!(body.contains("log_unavailable:native_missing"), "{action}: {body}");
        }
    }

    #[tokio::test]
    async fn backend_log_reference_degrades_to_durable_metadata_when_backend_read_fails() {
        let db = clankers_db::Db::in_memory().expect("db opens");
        let ctx = make_ctx_with_db(db.clone());
        let tool = ProcessTool::new();
        let mut backend_record = StoredProcessJobRecord::new_native(
            "pueue_999999999",
            "cargo check",
            StoredProcessJobOwnerScope::DaemonGlobal,
        );
        backend_record.backend = StoredProcessJobBackendKind::Pueue;
        backend_record.status = StoredProcessJobStatus::Succeeded { exit_code: Some(0) };
        backend_record.completed_at = Some(Utc::now());
        backend_record.log_refs = vec![StoredProcessJobLogRef {
            stream: StoredProcessJobStream::Combined,
            reference: "pueue:999999999:log".to_string(),
            retained_until: None,
            max_bytes: Some(10),
        }];
        db.async_process_jobs().upsert(backend_record).await.expect("insert backend record");

        for action in ["poll", "log"] {
            let result = tool.execute(&ctx, json!({"action": action, "session_id": "pueue_999999999"})).await;
            let body = text(&result);
            assert!(!result.is_error, "{action}: {result:?}");
            assert!(body.contains("pueue_999999999"), "{action}: {body}");
            assert!(body.contains("log_unavailable:backend_ref_unresolved:pueue:999999999:log"), "{action}: {body}");
            assert!(body.contains("backend"), "{action}: {body}");
        }
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
        let envelope: serde_json::Value = serde_json::from_str(&text(&result)).expect("gc json envelope");
        assert_eq!(envelope["common"]["operation"], "garbage_collect");
        let payload = &envelope["payload"]["data"]["receipt"];
        assert_eq!(payload["removed_metadata_count"], 1);
        assert_eq!(payload["removed_records"][0], "proc_gc_expired");
        assert_eq!(payload["tombstoned_records"].as_array().expect("tombstones").len(), 0);
        assert_eq!(payload["deleted_native_log_files"], 1);
        assert_eq!(payload["removed_log_bytes"], 12);
        assert_eq!(payload["released_log_refs"].as_array().expect("released refs").len(), 1);
        assert_eq!(payload["skipped_active_jobs"][0], "proc_gc_active");
        assert!(payload["failures"].as_array().expect("failures").is_empty(), "{payload}");
        assert!(db.async_process_jobs().get("proc_gc_expired").await.expect("db read").is_none());
        assert!(db.async_process_jobs().get("proc_gc_active").await.expect("db read").is_some());
        assert!(!log_path.exists());
    }

    #[tokio::test]
    async fn process_gc_only_deletes_native_logs_under_configured_temp_dir() {
        let db = clankers_db::Db::in_memory().expect("db opens");
        let ctx = make_ctx_with_db(db.clone());
        let tool = ProcessTool::new();
        let temp = tempfile::tempdir().expect("tempdir");
        let log_dir = temp.path().join("logs");
        let outside_dir = temp.path().join("outside");
        let outside_log = outside_dir.join("combined.log");
        std::fs::create_dir_all(&outside_dir).expect("outside dir");
        std::fs::write(&outside_log, b"must-not-delete").expect("outside log write");

        let old = Utc::now() - chrono::Duration::days(30);
        let mut escaped = StoredProcessJobRecord::new_native(
            "proc_gc_escaped_log",
            "printf escaped",
            StoredProcessJobOwnerScope::DaemonGlobal,
        );
        escaped.status = StoredProcessJobStatus::Succeeded { exit_code: Some(0) };
        escaped.started_at = old;
        escaped.updated_at = old;
        escaped.completed_at = Some(old);
        escaped.log_refs = vec![StoredProcessJobLogRef {
            stream: StoredProcessJobStream::Combined,
            reference: "native:../outside/combined.log".to_string(),
            retained_until: None,
            max_bytes: Some(15),
        }];
        db.async_process_jobs().upsert(escaped).await.expect("insert escaped ref");

        let result = tool
            .execute(
                &ctx,
                json!({
                    "action": "gc",
                    "max_age_days": 1,
                    "max_records": 100,
                    "max_log_bytes": 1_000_000,
                    "log_dir": log_dir.to_string_lossy()
                }),
            )
            .await;
        assert!(!result.is_error, "{result:?}");
        let payload = &tool_receipt_json(&result)["payload"]["data"]["receipt"];
        assert_eq!(payload["removed_metadata_count"], 1);
        assert_eq!(payload["released_log_refs"].as_array().expect("released refs").len(), 1);
        assert_eq!(payload["deleted_native_log_files"], 0);
        assert!(payload["failures"].as_array().expect("failures").is_empty(), "{payload}");
        assert!(outside_log.exists(), "GC must not resolve native log refs outside configured log_dir");
        assert!(db.async_process_jobs().get("proc_gc_escaped_log").await.expect("db read").is_none());
    }

    #[tokio::test]
    async fn process_gc_backend_filter_preserves_unselected_backend_records() {
        let db = clankers_db::Db::in_memory().expect("db opens");
        let ctx = make_ctx_with_db(db.clone());
        let tool = ProcessTool::new();
        let old = Utc::now() - chrono::Duration::seconds(GC_TEST_OLD_RECORD_SECS);

        let mut expired_native = StoredProcessJobRecord::new_native(
            "proc_gc_filter_native",
            "printf native",
            StoredProcessJobOwnerScope::DaemonGlobal,
        );
        expired_native.status = StoredProcessJobStatus::Succeeded { exit_code: Some(0) };
        expired_native.updated_at = old;
        expired_native.completed_at = Some(old);

        let mut expired_pueue = StoredProcessJobRecord::new_native(
            "pueue_gc_filter_keep",
            "printf pueue",
            StoredProcessJobOwnerScope::DaemonGlobal,
        );
        expired_pueue.backend = StoredProcessJobBackendKind::Pueue;
        expired_pueue.status = StoredProcessJobStatus::Succeeded { exit_code: Some(0) };
        expired_pueue.updated_at = old;
        expired_pueue.completed_at = Some(old);

        db.async_process_jobs().upsert(expired_native).await.expect("insert native");
        db.async_process_jobs().upsert(expired_pueue).await.expect("insert pueue");

        let result = tool
            .execute(
                &ctx,
                json!({
                    "action": "gc",
                    "backend": "native",
                    "max_age_days": 0,
                    "max_records": GC_TEST_MAX_RECORDS,
                    "max_log_bytes": GC_TEST_LOG_BUDGET_BYTES
                }),
            )
            .await;
        assert!(!result.is_error, "{result:?}");
        let payload = &tool_receipt_json(&result)["payload"]["data"]["receipt"];
        assert_eq!(payload["removed_metadata_count"], 1);
        assert_eq!(payload["removed_records"][0], "proc_gc_filter_native");
        assert!(db.async_process_jobs().get("proc_gc_filter_native").await.expect("db read").is_none());
        assert!(db.async_process_jobs().get("pueue_gc_filter_keep").await.expect("db read").is_some());
    }

    #[tokio::test]
    async fn process_gc_active_native_jobs_survive_age_count_and_log_pressure() {
        let db = clankers_db::Db::in_memory().expect("db opens");
        let ctx = make_ctx_with_db(db.clone());
        let tool = ProcessTool::new();
        let temp = tempfile::tempdir().expect("tempdir");
        let old = Utc::now() - chrono::Duration::days(30);

        let mut active = StoredProcessJobRecord::new_native(
            "proc_gc_pressure_active",
            "sleep protected",
            StoredProcessJobOwnerScope::DaemonGlobal,
        );
        active.status = StoredProcessJobStatus::Running;
        active.started_at = old;
        active.updated_at = old;
        active.log_refs = vec![StoredProcessJobLogRef {
            stream: StoredProcessJobStream::Combined,
            reference: "native:proc_gc_pressure_active/combined.log".to_string(),
            retained_until: None,
            max_bytes: Some(1_000_000),
        }];
        let active_log_path = temp.path().join("proc_gc_pressure_active").join("combined.log");
        std::fs::create_dir_all(active_log_path.parent().expect("log parent")).expect("active log dir");
        std::fs::write(&active_log_path, b"active-log").expect("active log write");
        db.async_process_jobs().upsert(active).await.expect("insert active");

        let result = tool
            .execute(
                &ctx,
                json!({
                    "action": "gc",
                    "max_age_days": 0,
                    "max_records": 0,
                    "max_log_bytes": 0,
                    "log_dir": temp.path().to_string_lossy()
                }),
            )
            .await;
        assert!(!result.is_error, "{result:?}");
        let payload = &tool_receipt_json(&result)["payload"]["data"]["receipt"];
        assert_eq!(payload["removed_metadata_count"], 0);
        assert_eq!(payload["deleted_native_log_files"], 0);
        assert_eq!(payload["skipped_active_jobs"][0], "proc_gc_pressure_active");
        assert!(payload["failures"].as_array().expect("failures").is_empty(), "{payload}");
        assert!(active_log_path.exists(), "active-job log must survive GC pressure");
        assert!(db.async_process_jobs().get("proc_gc_pressure_active").await.expect("db read").is_some());
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
        let envelope = tool_receipt_json(&listed);
        assert_eq!(envelope["common"]["operation"], "list");
        assert_eq!(envelope["payload"]["kind"], "list");
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
        assert!(listed_text.contains("reattached_log_incomplete"), "{listed_text}");
        assert!(listed_text.contains("lost_after_restart"), "{listed_text}");

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
    async fn process_restart_reconciliation_preserves_stable_id_log_ref_and_reports_lost_status() {
        let temp = tempfile::tempdir().expect("tempdir");
        let db_path = temp.path().join("process-jobs.redb");
        let log_dir = temp.path().join("logs");
        let mut child = std::process::Command::new("sleep").arg("30").spawn().expect("long-lived test child");
        let child_pid = child.id();
        let stable_id = format!("proc_restart_{child_pid}");
        let log_reference = format!("native:{stable_id}/combined.log");
        let log_path = log_dir.join(&stable_id).join("combined.log");
        std::fs::create_dir_all(log_path.parent().expect("log parent")).expect("log dir");
        std::fs::write(
            &log_path,
            b"pre-restart-log
",
        )
        .expect("log write");

        {
            let db = clankers_db::Db::open(&db_path).expect("disk redb opens");
            let mut record =
                StoredProcessJobRecord::new_native(&stable_id, "sleep 30", StoredProcessJobOwnerScope::DaemonGlobal);
            record.status = StoredProcessJobStatus::Running;
            record.backend_ref = Some(format!("pid:{child_pid}"));
            record.os_pid = Some(child_pid);
            record.process_group = i32::try_from(child_pid).ok();
            record.log_refs = vec![StoredProcessJobLogRef {
                stream: StoredProcessJobStream::Combined,
                reference: log_reference.clone(),
                retained_until: None,
                max_bytes: Some(16),
            }];
            db.async_process_jobs().upsert(record).await.expect("insert active process record");
        }

        let restarted_db = clankers_db::Db::open(&db_path).expect("reopened redb");
        let ctx = make_ctx_with_db(restarted_db.clone());
        let tool = ProcessTool::new();
        let listed = tool.execute(&ctx, json!({"action": "list", "backend": "native"})).await;
        assert!(!listed.is_error, "{listed:?}");
        let listed_text = text(&listed);
        assert!(listed_text.contains(&stable_id), "{listed_text}");
        assert!(listed_text.contains("running"), "{listed_text}");

        let reattached = restarted_db
            .async_process_jobs()
            .get(stable_id.clone())
            .await
            .expect("db read")
            .expect("reattached record");
        assert_eq!(reattached.id, stable_id);
        assert_eq!(reattached.status, StoredProcessJobStatus::Running);
        assert_eq!(reattached.log_refs.len(), 1);
        assert_eq!(reattached.log_refs[0].reference, log_reference);
        assert_eq!(reattached.safe_metadata.get("reconciliation").map(String::as_str), Some("reattached"));

        child.kill().expect("kill long-lived child");
        child.wait().expect("reap long-lived child");

        let listed_after_loss = tool.execute(&ctx, json!({"action": "list", "backend": "native"})).await;
        assert!(!listed_after_loss.is_error, "{listed_after_loss:?}");
        let lost_text = text(&listed_after_loss);
        assert!(lost_text.contains(&stable_id), "{lost_text}");
        assert!(lost_text.contains("lost_after_restart"), "{lost_text}");

        let lost = restarted_db
            .async_process_jobs()
            .get(stable_id.clone())
            .await
            .expect("db read")
            .expect("lost record");
        assert_eq!(lost.id, stable_id);
        assert_eq!(lost.status, StoredProcessJobStatus::LostAfterRestart);
        assert_eq!(lost.log_refs.len(), 1);
        assert_eq!(lost.log_refs[0].reference, log_reference);
        assert_eq!(lost.safe_metadata.get("reconciliation").map(String::as_str), Some("lost-after-restart"));
        assert!(lost.completed_at.is_some());
        assert!(log_path.exists(), "reconciliation must preserve existing log reference without GC side effects");
    }

    #[tokio::test]
    async fn starts_and_waits_for_process() {
        let tool = ProcessTool::new();
        let started = tool.execute(&make_ctx(), json!({"action": "start", "command": "printf hello"})).await;
        assert!(!started.is_error, "{started:?}");
        let envelope = tool_receipt_json(&started);
        assert_eq!(envelope["common"]["operation"], "start");
        assert_eq!(envelope["common"]["backend"], "native");
        let id = extract_process_id(&started);
        assert!(ProcessJobId(id.clone()).is_blake3_native(), "{id}");
        let waited = tool.execute(&make_ctx(), json!({"action": "wait", "session_id": id, "timeout": 2})).await;
        assert!(!waited.is_error, "{waited:?}");
        assert!(text(&waited).contains("hello"), "{}", text(&waited));
    }

    #[tokio::test]
    async fn native_process_actions_preserve_default_compatibility() {
        let tool = ProcessTool::new();
        let ctx = make_ctx();
        let started = tool.execute(&ctx, json!({"action": "start", "command": "cat"})).await;
        assert!(!started.is_error, "{started:?}");
        let start_envelope = tool_receipt_json(&started);
        assert_eq!(start_envelope["common"]["operation"], "start");
        assert_eq!(start_envelope["common"]["backend"], "native");
        assert!(start_envelope["common"]["backend_ref"].as_str().expect("backend ref").starts_with("pid:"));
        let id = extract_process_id(&started);
        assert!(ProcessJobId(id.clone()).is_blake3_native(), "{id}");

        let listed_running = tool.execute(&ctx, json!({"action": "list", "backend": "native"})).await;
        assert!(!listed_running.is_error, "{listed_running:?}");
        let listed_running_envelope = tool_receipt_json(&listed_running);
        assert_eq!(listed_running_envelope["common"]["operation"], "list");
        assert!(text(&listed_running).contains(&id), "{}", text(&listed_running));

        let wrote = tool.execute(&ctx, json!({"action": "write", "session_id": id, "data": "raw"})).await;
        assert!(!wrote.is_error, "{wrote:?}");
        assert!(text(&wrote).contains("Wrote 3 bytes"), "{}", text(&wrote));
        let id = extract_process_id(&started);
        let submitted = tool.execute(&ctx, json!({"action": "submit", "session_id": id, "data": "line"})).await;
        assert!(!submitted.is_error, "{submitted:?}");
        assert!(text(&submitted).contains("Wrote 5 bytes"), "{}", text(&submitted));
        let id = extract_process_id(&started);
        let closed = tool.execute(&ctx, json!({"action": "close", "session_id": id})).await;
        assert!(!closed.is_error, "{closed:?}");
        assert!(text(&closed).contains("Closed stdin"), "{}", text(&closed));
        let id = extract_process_id(&started);
        let waited = tool.execute(&ctx, json!({"action": "wait", "session_id": id, "timeout": 2})).await;
        assert!(!waited.is_error, "{waited:?}");
        assert!(text(&waited).contains("rawline"), "{}", text(&waited));

        let id = extract_process_id(&started);
        let log = tool.execute(&ctx, json!({"action": "log", "session_id": id, "limit": 10})).await;
        assert!(!log.is_error, "{log:?}");
        assert!(text(&log).contains("rawline"), "{}", text(&log));

        let id = extract_process_id(&started);
        let listed_terminal = tool.execute(&ctx, json!({"action": "list", "backend": "native"})).await;
        assert!(!listed_terminal.is_error, "{listed_terminal:?}");
        assert!(text(&listed_terminal).contains(&id), "{}", text(&listed_terminal));
        let id = extract_process_id(&started);
        let listed_non_terminal =
            tool.execute(&ctx, json!({"action": "list", "backend": "native", "include_terminal": false})).await;
        assert!(!listed_non_terminal.is_error, "{listed_non_terminal:?}");
        assert!(!text(&listed_non_terminal).contains(&id), "{}", text(&listed_non_terminal));
    }

    #[tokio::test]
    async fn native_restart_relaunches_running_process_under_same_stable_id() {
        let tool = ProcessTool::new();
        let ctx = make_ctx();
        let started = tool.execute(&ctx, json!({"action": "start", "command": "cat"})).await;
        assert!(!started.is_error, "{started:?}");
        let id = extract_process_id(&started);

        let restarted = tool.execute(&ctx, json!({"action": "restart", "session_id": id.clone()})).await;
        assert!(!restarted.is_error, "{restarted:?}");
        let envelope = tool_receipt_json(&restarted);
        assert_eq!(envelope["common"]["operation"].as_str(), Some("restart"));
        assert_eq!(envelope["common"]["id"].as_str(), Some(id.as_str()));
        assert_eq!(envelope["common"]["backend"].as_str(), Some("native"));
        assert_eq!(envelope["common"]["status"]["state"].as_str(), Some("running"));

        let submitted = tool
            .execute(&ctx, json!({"action": "submit", "session_id": id.clone(), "data": "after-restart"}))
            .await;
        assert!(!submitted.is_error, "{submitted:?}");
        let closed = tool.execute(&ctx, json!({"action": "close", "session_id": id.clone()})).await;
        assert!(!closed.is_error, "{closed:?}");
        let waited = tool
            .execute(&ctx, json!({"action": "wait", "session_id": id, "timeout": SHORT_PROCESS_TEST_TIMEOUT_SECS}))
            .await;
        assert!(!waited.is_error, "{waited:?}");
        assert!(text(&waited).contains("after-restart"), "{}", text(&waited));
    }

    #[tokio::test]
    async fn native_restart_registers_relaunched_pid_with_process_monitor() {
        let monitor =
            Arc::new(clankers_procmon::ProcessMonitor::new(clankers_procmon::ProcessMonitorConfig::default(), None));
        let tool = ProcessTool::new().with_process_monitor(monitor.clone());
        let ctx = make_ctx();
        let started = tool.execute(&ctx, json!({"action": "start", "command": "cat"})).await;
        assert!(!started.is_error, "{started:?}");
        let id = extract_process_id(&started);

        let restarted = tool.execute(&ctx, json!({"action": "restart", "session_id": id.clone()})).await;
        assert!(!restarted.is_error, "{restarted:?}");
        let envelope = tool_receipt_json(&restarted);
        let backend_ref = envelope["common"]["backend_ref"].as_str().expect("restart receipt has backend ref");
        let restarted_pid = native_pid_from_backend_ref(&BackendRef(backend_ref.to_string())).expect("pid parses");
        let snapshot = monitor.snapshot();
        assert!(
            snapshot.iter().any(|(pid, proc)| *pid == restarted_pid && proc.meta.call_id == ctx.call_id),
            "restarted pid {restarted_pid} must be tracked in process monitor: {snapshot:?}"
        );

        let closed = tool.execute(&ctx, json!({"action": "close", "session_id": id.clone()})).await;
        assert!(!closed.is_error, "{closed:?}");
        let waited = tool
            .execute(&ctx, json!({"action": "wait", "session_id": id, "timeout": SHORT_PROCESS_TEST_TIMEOUT_SECS}))
            .await;
        assert!(!waited.is_error, "{waited:?}");
    }

    #[tokio::test]
    async fn native_restart_keeps_restarted_record_persisted_after_old_waiter_finishes() {
        let db = clankers_db::Db::in_memory().expect("db opens");
        let ctx = make_ctx_with_db(db.clone());
        let tool = ProcessTool::new();
        let started = tool.execute(&ctx, json!({"action": "start", "command": "cat"})).await;
        assert!(!started.is_error, "{started:?}");
        let id = extract_process_id(&started);

        let restarted = tool.execute(&ctx, json!({"action": "restart", "session_id": id.clone()})).await;
        assert!(!restarted.is_error, "{restarted:?}");
        tokio::time::sleep(Duration::from_millis(RESTART_PERSISTENCE_SETTLE_MILLIS)).await;
        let stored = db.async_process_jobs().get(id.clone()).await.expect("db read").expect("record stored");
        assert_eq!(stored.status, StoredProcessJobStatus::Running);
        assert!(stored.backend_ref.as_deref().unwrap_or_default().starts_with("pid:"));

        let closed = tool.execute(&ctx, json!({"action": "close", "session_id": id.clone()})).await;
        assert!(!closed.is_error, "{closed:?}");
        let waited = tool
            .execute(&ctx, json!({"action": "wait", "session_id": id, "timeout": SHORT_PROCESS_TEST_TIMEOUT_SECS}))
            .await;
        assert!(!waited.is_error, "{waited:?}");
    }

    #[tokio::test]
    async fn native_restart_replays_direct_program_request() {
        let tool = ProcessTool::new();
        let ctx = make_ctx();
        let started = tool
            .execute(&ctx, json!({"action": "start", "program": "printf", "args": ["direct-restart"]}))
            .await;
        assert!(!started.is_error, "{started:?}");
        let id = extract_process_id(&started);
        let waited = tool
            .execute(
                &ctx,
                json!({"action": "wait", "session_id": id.clone(), "timeout": SHORT_PROCESS_TEST_TIMEOUT_SECS}),
            )
            .await;
        assert!(!waited.is_error, "{waited:?}");
        assert!(text(&waited).contains("direct-restart"), "{}", text(&waited));

        let restarted = tool.execute(&ctx, json!({"action": "restart", "session_id": id.clone()})).await;
        assert!(!restarted.is_error, "{restarted:?}");
        let restarted_wait = tool
            .execute(&ctx, json!({"action": "wait", "session_id": id, "timeout": SHORT_PROCESS_TEST_TIMEOUT_SECS}))
            .await;
        assert!(!restarted_wait.is_error, "{restarted_wait:?}");
        assert!(text(&restarted_wait).contains("direct-restart"), "{}", text(&restarted_wait));
    }

    #[tokio::test]
    async fn native_restart_unknown_session_fails_without_mutation() {
        let tool = ProcessTool::new();
        let restarted = tool.execute(&make_ctx(), json!({"action": "restart", "session_id": "proc_missing"})).await;
        assert!(restarted.is_error, "{restarted:?}");
        assert!(text(&restarted).contains("Unknown process session_id: proc_missing"), "{}", text(&restarted));
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

    #[test]
    fn process_parser_produces_backend_neutral_request_dtos_for_all_actions() {
        let start = ProcessTool::process_job_tool_request(&json!({
            "action": "start",
            "backend": "pueue",
            "program": "cargo",
            "args": ["nextest", "run"],
            "group": "ci",
            "notify_on_complete": true
        }))
        .expect("start request parses");
        match start {
            ProcessJobToolRequest::Start(start) => {
                assert_eq!(start.backend, ProcessJobBackendKind::Pueue);
                assert_eq!(start.command_preview, "cargo nextest run");
                assert_eq!(start.program.as_deref(), Some("cargo"));
                assert_eq!(start.args, vec!["nextest", "run"]);
                assert!(start.notification_policy.notify_on_complete);
                assert_eq!(start.metadata.get("group").map(String::as_str), Some("ci"));
            }
            other => panic!("unexpected request: {other:?}"),
        }

        match ProcessTool::process_job_tool_request(
            &json!({"action": "list", "backend": "systemd", "include_terminal": false}),
        )
        .expect("list request parses")
        {
            ProcessJobToolRequest::List(request) => {
                assert_eq!(request.filter.backend, Some(ProcessJobBackendKind::Systemd));
                assert!(!request.filter.include_terminal);
            }
            other => panic!("unexpected request: {other:?}"),
        }

        match ProcessTool::process_job_tool_request(&json!({"action": "poll", "session_id": "proc_b3_poll"}))
            .expect("poll request parses")
        {
            ProcessJobToolRequest::Poll(request) => assert_eq!(request.id.0, "proc_b3_poll"),
            other => panic!("unexpected request: {other:?}"),
        }

        match ProcessTool::process_job_tool_request(
            &json!({"action": "log", "session_id": "proc_b3_log", "offset": 12, "limit": 34}),
        )
        .expect("log request parses")
        {
            ProcessJobToolRequest::Log(request) => {
                assert_eq!(request.id.0, "proc_b3_log");
                assert_eq!(request.range.offset, Some(12));
                assert_eq!(request.range.limit_bytes, 34);
                assert!(!request.raw);
            }
            other => panic!("unexpected request: {other:?}"),
        }

        match ProcessTool::process_job_tool_request(&json!({"action": "log", "session_id": "proc_b3_log", "raw": true}))
            .expect("raw log request parses")
        {
            ProcessJobToolRequest::Log(request) => assert!(request.raw),
            other => panic!("unexpected request: {other:?}"),
        }

        match ProcessTool::process_job_tool_request(
            &json!({"action": "wait", "session_id": "proc_b3_wait", "timeout": 9}),
        )
        .expect("wait request parses")
        {
            ProcessJobToolRequest::Wait(request) => {
                assert_eq!(request.id.0, "proc_b3_wait");
                assert_eq!(request.timeout, Some(Duration::from_secs(9)));
            }
            other => panic!("unexpected request: {other:?}"),
        }

        for (action, expected_variant) in [("kill", "kill"), ("restart", "restart"), ("close", "close")] {
            let request =
                ProcessTool::process_job_tool_request(&json!({"action": action, "session_id": "proc_b3_mutate"}))
                    .expect("mutation request parses");
            match (expected_variant, request) {
                ("kill", ProcessJobToolRequest::Kill(request))
                | ("restart", ProcessJobToolRequest::Restart(request))
                | ("close", ProcessJobToolRequest::CloseStdin(request)) => assert_eq!(request.id.0, "proc_b3_mutate"),
                (_, other) => panic!("unexpected request: {other:?}"),
            }
        }

        match ProcessTool::process_job_tool_request(
            &json!({"action": "write", "session_id": "proc_b3_stdin", "data": "ping"}),
        )
        .expect("write request parses")
        {
            ProcessJobToolRequest::WriteStdin(request) => {
                assert_eq!(request.id.0, "proc_b3_stdin");
                assert_eq!(request.data, b"ping");
                assert!(!request.newline);
            }
            other => panic!("unexpected request: {other:?}"),
        }
        match ProcessTool::process_job_tool_request(
            &json!({"action": "submit", "session_id": "proc_b3_stdin", "data": "pong"}),
        )
        .expect("submit request parses")
        {
            ProcessJobToolRequest::WriteStdin(request) => {
                assert_eq!(request.data, b"pong");
                assert!(request.newline);
            }
            other => panic!("unexpected request: {other:?}"),
        }

        match ProcessTool::process_job_tool_request(
            &json!({"action": "adopt", "backend": "systemd", "systemd_unit": "clankers-build.service"}),
        )
        .expect("adopt request parses")
        {
            ProcessJobToolRequest::Adopt(request) => {
                assert_eq!(request.backend, ProcessJobBackendKind::Systemd);
                assert_eq!(request.backend_ref.0, "clankers-build.service");
            }
            other => panic!("unexpected request: {other:?}"),
        }

        match ProcessTool::process_job_tool_request(&json!({"action": "gc", "backend": "native"}))
            .expect("gc request parses")
        {
            ProcessJobToolRequest::GarbageCollect(request) => {
                assert_eq!(request.filter.backend, Some(ProcessJobBackendKind::Native));
            }
            other => panic!("unexpected request: {other:?}"),
        }

        let unsupported = ProcessTool::process_job_tool_request(&json!({"action": "unsupported_process_action"}));
        assert!(unsupported.is_err(), "unsupported action must fail closed before backend dispatch");
        let bad_start = ProcessTool::process_job_tool_request(
            &json!({"action": "start", "command": "printf shell", "program": "printf"}),
        );
        assert!(bad_start.is_err(), "ambiguous start shape must fail closed before backend dispatch");
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
                    "program": "sh",
                    "args": ["-c", "trap '' TERM; sleep 10"],
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
