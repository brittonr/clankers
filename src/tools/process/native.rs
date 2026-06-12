use std::collections::HashMap;
use std::sync::LazyLock;

use tokio::io::AsyncBufReadExt;
use tokio::io::BufReader;

use super::*;

#[derive(Clone, Debug)]
pub(super) enum ProcessStatus {
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
pub(super) enum NativeTerminationOutcome {
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
    pub(super) fn is_done(&self) -> bool {
        !matches!(self, Self::Running)
    }

    pub(super) fn label(&self) -> String {
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

// Lock order: notification_state -> notifications -> notification_cursor; output -> poll_cursor;
// status, stdin, and kill_tx are single-lock paths. Do not hold std::sync locks across await points.
pub(super) struct ProcessEntry {
    pub(super) id: String,
    pub(super) command: String,
    pub(super) restart_request: StartProcessJobRequest,
    pub(super) started_at: Instant,
    pub(super) started_at_wall: DateTime<Utc>,
    pub(super) backend_ref: Option<BackendRef>,
    pub(super) profile: Option<ProcessJobProfileReceiptMetadata>,
    /// Lock order: acquire before `poll_cursor` when draining output.
    pub(super) output: std::sync::Mutex<Vec<String>>,
    /// Lock order: acquire after `output`; no other process locks are held.
    pub(super) poll_cursor: std::sync::Mutex<usize>,
    pub(super) notification_policy: ProcessJobNotificationPolicy,
    /// Lock order: acquire before notification event locks, then drop before std::sync locks.
    pub(super) notification_state: tokio::sync::Mutex<ProcessJobNotificationPolicyState>,
    /// Lock order: acquire before `notification_cursor` when draining notifications.
    pub(super) notifications: std::sync::Mutex<Vec<ProcessJobNotificationEvent>>,
    /// Lock order: acquire after `notifications`; no async guard may be held.
    pub(super) notification_cursor: std::sync::Mutex<usize>,
    pub(super) next_notification_seq: AtomicU64,
    /// Lock order: independent status lock; do not hold with stdio or kill locks.
    pub(super) status: std::sync::Mutex<ProcessStatus>,
    /// Lock order: independent stdio lock; do not hold with status or kill locks.
    pub(super) stdin: tokio::sync::Mutex<Option<ChildStdin>>,
    /// Lock order: independent kill lock; do not hold with status or stdio locks.
    pub(super) kill_tx: std::sync::Mutex<Option<oneshot::Sender<()>>>,
}

impl ProcessEntry {
    pub(super) fn new(
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

    pub(super) fn push_output(&self, stream: &str, raw: &str) -> String {
        let line = strip_ansi(raw);
        let mut output = self.output.lock().expect("process output lock poisoned");
        output.push(format!("[{stream}] {line}"));
        line
    }

    pub(super) async fn evaluate_output_notification(&self, line: String) {
        self.evaluate_notification(ProcessJobNotificationObservation {
            status: self.job_status(),
            line: Some(line),
            tick: self.started_at.elapsed().as_secs(),
        })
        .await;
    }

    pub(super) async fn evaluate_completion_notification(&self) {
        let excerpt = self.snapshot_output().last().cloned();
        self.evaluate_notification(ProcessJobNotificationObservation {
            status: self.job_status(),
            line: excerpt,
            tick: self.started_at.elapsed().as_secs(),
        })
        .await;
    }

    pub(super) async fn evaluate_notification(&self, observation: ProcessJobNotificationObservation) {
        evaluate_process_entry_notification(self, observation).await;
    }

    pub(super) fn drain_new_notifications(&self) -> Vec<ProcessJobNotificationEvent> {
        let notifications = self.notifications.lock().expect("process notification lock poisoned");
        let mut cursor = self.notification_cursor.lock().expect("process notification cursor lock poisoned");
        let new = notifications.get(*cursor..).unwrap_or(&[]).to_vec();
        *cursor = notifications.len();
        new
    }

    pub(super) fn job_status(&self) -> ProcessJobStatus {
        status_to_job_status(&self.status())
    }

    pub(super) fn set_status(&self, status: ProcessStatus) {
        let mut current = self.status.lock().expect("process status lock poisoned");
        *current = status;
    }

    pub(super) fn status(&self) -> ProcessStatus {
        self.status.lock().expect("process status lock poisoned").clone()
    }

    pub(super) fn snapshot_output(&self) -> Vec<String> {
        self.output.lock().expect("process output lock poisoned").clone()
    }

    pub(super) fn drain_new_output(&self) -> Vec<String> {
        let output = self.output.lock().expect("process output lock poisoned");
        let mut cursor = self.poll_cursor.lock().expect("process poll cursor lock poisoned");
        let new = output.get(*cursor..).unwrap_or(&[]).to_vec();
        *cursor = output.len();
        new
    }

    pub(super) fn summary(&self) -> ProcessJobSummary {
        ProcessJobSummary {
            id: ProcessJobId(self.id.clone()),
            backend: ProcessJobBackendKind::Native,
            backend_ref: self.backend_ref.clone(),
            owner: clankers_runtime::process_jobs::ProcessJobOwnerScope::DaemonGlobal,
            status: status_to_job_status(&self.status()),
            command_preview: ProcessJobRedactionPolicy::default().safe_command_preview(&self.command),
            cwd: clankers_runtime::process_jobs::ProcessJobCwd::Inherited,
            started_at: Some(process_job_timestamp(self.started_at_wall)),
            updated_at: process_job_timestamp(Utc::now()),
            completed_at: self.status().is_done().then(|| process_job_timestamp(Utc::now())),
            log_refs: Vec::new(),
            profile: self.profile.clone(),
        }
    }
}

#[derive(Debug, Clone, Default)]
pub(super) struct NativeProcessJobService {
    db: Option<clankers_db::Db>,
    retention_policy: ProcessJobRetentionPolicy,
    log_dir: Option<PathBuf>,
}

impl NativeProcessJobService {
    #[cfg(test)]
    #[must_use]
    pub(super) fn with_retention(
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

#[async_trait]
impl ProcessJobService for NativeProcessJobService {
    async fn start(&self, request: StartProcessJobRequest) -> Result<ProcessJobReceipt, RuntimeError> {
        start_native_process_job(request, self.db.clone(), None, None).await
    }

    async fn list(&self, filter: ProcessJobFilter) -> Result<Vec<ProcessJobSummary>, RuntimeError> {
        let mut entries = all_entries();
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
        should_append_newline: bool,
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
        if should_append_newline {
            stdin.write_all(b"\n").await.map_err(|e| RuntimeError::InvalidTool(e.to_string()))?;
        }
        stdin.flush().await.map_err(|e| RuntimeError::InvalidTool(e.to_string()))?;
        Ok(native_receipt(
            ProcessJobOperation::WriteStdin,
            &entry,
            format!("Wrote {} bytes to {}", data.len() + usize::from(should_append_newline), entry.id),
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

pub(super) fn native_pid_from_backend_ref(backend_ref: &BackendRef) -> Result<u32, RuntimeError> {
    let raw = backend_ref.0.strip_prefix("pid:").unwrap_or(backend_ref.0.as_str());
    let pid = raw
        .parse::<u32>()
        .map_err(|_| RuntimeError::InvalidTool(format!("invalid native pid backend_ref: {}", backend_ref.0)))?;
    if pid == 0 {
        return Err(RuntimeError::InvalidTool("native pid adoption requires a non-zero pid".to_string()));
    }
    Ok(pid)
}

fn native_entry(id: &ProcessJobId) -> Result<Arc<ProcessEntry>, RuntimeError> {
    get(&id.0).ok_or_else(|| RuntimeError::InvalidTool(format!("Unknown process session_id: {}", id.0)))
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
                super::super::ToolResultContent::Text { text } => Some(text.as_str()),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("\n"),
    )
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

static REGISTRY: LazyLock<std::sync::Mutex<ProcessRegistry>> =
    LazyLock::new(|| std::sync::Mutex::new(ProcessRegistry::default()));

#[derive(Default)]
pub(super) struct ProcessRegistry {
    pub(super) next_id: u64,
    pub(super) entries: HashMap<String, Arc<ProcessEntry>>,
    pub(super) reserved_starts: usize,
}

impl ProcessRegistry {
    pub(super) fn active_or_reserved_count(&self) -> usize {
        self.entries.values().filter(|entry| !entry.status().is_done()).count() + self.reserved_starts
    }

    pub(super) fn admission_decision(&self, limit: usize) -> NativeAdmissionDecision {
        native_admission_decision(ProcessJobNativeAdmissionInput {
            active: self.active_or_reserved_count(),
            limit,
        })
    }

    pub(super) fn reserve_start(
        &mut self,
        limit: usize,
    ) -> Result<NativeAdmissionReservation, NativeAdmissionDecision> {
        let decision = self.admission_decision(limit);
        if !decision.accepted {
            return Err(decision);
        }
        self.reserved_starts += 1;
        Ok(NativeAdmissionReservation { released: false })
    }

    pub(super) fn release_start_reservation(&mut self) {
        self.reserved_starts = self.reserved_starts.saturating_sub(1);
    }
}

pub(super) struct NativeAdmissionReservation {
    released: bool,
}

impl NativeAdmissionReservation {
    pub(super) fn release(mut self) {
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

pub(super) fn next_native_job_id(request: &StartProcessJobRequest) -> ProcessJobId {
    let mut registry = REGISTRY.lock().expect("process registry lock poisoned");
    registry.next_id += 1;
    let request_nonce = format!("native:{}", registry.next_id);
    ProcessJobIdentityEnvelope::for_start_request(request, request_nonce).derive_id()
}

pub(super) fn insert(entry: Arc<ProcessEntry>) {
    let mut registry = REGISTRY.lock().expect("process registry lock poisoned");
    registry.entries.insert(entry.id.clone(), entry);
}

pub(super) fn reserve_native_start() -> Result<NativeAdmissionReservation, NativeAdmissionDecision> {
    REGISTRY
        .lock()
        .expect("process registry lock poisoned")
        .reserve_start(MAX_NATIVE_ACTIVE_PROCESS_JOBS)
}

pub(super) fn get(session_id: &str) -> Option<Arc<ProcessEntry>> {
    let registry = REGISTRY.lock().expect("process registry lock poisoned");
    registry.entries.get(session_id).cloned()
}

pub(super) fn all_entries() -> Vec<Arc<ProcessEntry>> {
    let registry = REGISTRY.lock().expect("process registry lock poisoned");
    registry.entries.values().cloned().collect()
}

pub(super) fn is_current_entry(entry: &Arc<ProcessEntry>) -> bool {
    get(&entry.id).is_some_and(|current| Arc::ptr_eq(&current, entry))
}

pub(super) fn admission_denied_receipt(decision: NativeAdmissionDecision) -> ProcessJobReceipt {
    admission_denied_receipt_for(ProcessJobOperation::Start, decision)
}

pub(super) fn admission_denied_receipt_for(
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

pub(super) struct NativeProcessJobBackendAdapter {
    db: Option<clankers_db::Db>,
    process_monitor: Option<clankers_procmon::ProcessMonitorHandle>,
    call_id: Option<String>,
}

impl NativeProcessJobBackendAdapter {
    pub(super) fn for_invocation(
        db: Option<clankers_db::Db>,
        process_monitor: Option<clankers_procmon::ProcessMonitorHandle>,
        call_id: String,
    ) -> Self {
        Self {
            db,
            process_monitor,
            call_id: Some(call_id),
        }
    }

    pub(super) async fn start(&self, request: StartProcessJobRequest) -> Result<ProcessJobReceipt, RuntimeError> {
        start_native_process_job(request, self.db.clone(), self.process_monitor.as_ref(), self.call_id.as_deref()).await
    }

    pub(super) async fn restart(&self, id: ProcessJobId) -> Result<ProcessJobReceipt, RuntimeError> {
        restart_native_process_job(id, self.db.clone(), self.process_monitor.as_ref(), self.call_id.as_deref()).await
    }
}

pub(super) async fn start_native_process_job(
    request: StartProcessJobRequest,
    db: Option<clankers_db::Db>,
    process_monitor: Option<&clankers_procmon::ProcessMonitorHandle>,
    call_id: Option<&str>,
) -> Result<ProcessJobReceipt, RuntimeError> {
    if request.backend != ProcessJobBackendKind::Native {
        return Ok(unsupported_backend_receipt(
            ProcessJobOperation::Start,
            None,
            request.backend,
            "current process tool default service supports only native backend",
        ));
    }
    let admission = match reserve_native_start() {
        Ok(admission) => admission,
        Err(decision) => return Ok(admission_denied_receipt(decision)),
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
    let id = next_native_job_id(&request);
    let entry = Arc::new(ProcessEntry::new(
        id.0.clone(),
        display_command,
        request.clone(),
        stdin,
        kill_tx,
        pid,
        request.notification_policy.clone(),
        ProcessJobProfileReceiptMetadata::from_metadata(&request.metadata),
    ));
    let backend_ref = entry.backend_ref.clone();
    insert(entry.clone());
    admission.release();
    if let Some(monitor) = process_monitor
        && let Some(pid) = pid
    {
        monitor.register_at(pid, clankers_procmon::ProcessMeta {
            tool_name: "process".to_string(),
            command: ProcessJobRedactionPolicy::default().safe_command_preview(&entry.command),
            call_id: call_id.unwrap_or("process-start").to_string(),
        }, std::time::Instant::now());
    }
    persist_entry(db.as_ref(), &entry).await;
    spawn_reader(entry.clone(), "stdout", stdout);
    spawn_reader(entry.clone(), "stderr", stderr);
    spawn_waiter(entry, child, pid, kill_rx, db);

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

pub(super) async fn restart_native_process_job(
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

    let admission = match reserve_native_start() {
        Ok(admission) => admission,
        Err(decision) => return Ok(admission_denied_receipt_for(ProcessJobOperation::Restart, decision)),
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
    insert(new_entry.clone());
    admission.release();
    if let Some(monitor) = process_monitor
        && let Some(pid) = pid
    {
        monitor.register_at(pid, clankers_procmon::ProcessMeta {
            tool_name: "process".to_string(),
            command: ProcessJobRedactionPolicy::default().safe_command_preview(&new_entry.command),
            call_id: call_id.unwrap_or("process-restart").to_string(),
        }, std::time::Instant::now());
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
        if is_current_entry(&entry) {
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

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::time::Duration;

    use chrono::Utc;
    use tokio::sync::oneshot;

    use super::super::*;
    use super::*;

    fn native_start_request(command: &str) -> StartProcessJobRequest {
        StartProcessJobRequest {
            backend: ProcessJobBackendKind::Native,
            command_preview: command.to_string(),
            program: None,
            args: Vec::new(),
            shell_command: Some(command.to_string()),
            cwd: ProcessJobCwd::Inherited,
            owner: ProcessJobOwnerScope::DaemonGlobal,
            resource_policy: clankers_runtime::process_jobs::ProcessJobResourcePolicy::default(),
            notification_policy: ProcessJobNotificationPolicy::default(),
            metadata: std::collections::BTreeMap::default(),
        }
    }

    const SHORT_PROCESS_TEST_TIMEOUT_SECS: u64 = 2;
    const GC_TEST_OLD_RECORD_SECS: i64 = 2;
    const GC_TEST_RETENTION_SECS: u64 = 1;
    const GC_TEST_LOG_BYTES: u64 = 12;

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

    #[tokio::test]
    async fn native_backend_in_memory_fixture_projects_list_poll_log_and_errors_without_spawning() {
        let service = NativeProcessJobService::default();
        let mut unsupported_start = native_start_request("printf should-not-spawn");
        unsupported_start.backend = ProcessJobBackendKind::Pueue;
        let unsupported = service.start(unsupported_start).await.expect("unsupported start is typed receipt");
        assert_eq!(unsupported.operation, ProcessJobOperation::Start);
        assert_eq!(unsupported.backend, Some(ProcessJobBackendKind::Pueue));
        assert_eq!(
            unsupported.error.expect("unsupported backend error").code,
            ProcessJobErrorCode::UnsupportedActionForBackend
        );

        let (kill_tx, _kill_rx) = oneshot::channel();
        let id = format!("proc_in_memory_fixture_{}", Utc::now().timestamp_nanos_opt().unwrap_or_default());
        let entry = Arc::new(ProcessEntry::new(
            id.clone(),
            "printf fixture".to_string(),
            native_start_request("printf fixture"),
            None,
            kill_tx,
            None,
            ProcessJobNotificationPolicy::default(),
            None,
        ));
        entry.push_output("stdout", "fixture line one");
        entry.push_output("stderr", "fixture line two");
        insert(entry.clone());

        let summaries = service
            .list(ProcessJobFilter {
                backend: Some(ProcessJobBackendKind::Native),
                include_terminal: true,
                ..ProcessJobFilter::default()
            })
            .await
            .expect("list projects registry entries");
        let summary = summaries.iter().find(|summary| summary.id.0 == id).expect("fixture summary listed");
        assert_eq!(summary.backend, ProcessJobBackendKind::Native);
        assert_eq!(summary.status, ProcessJobStatus::Running);
        assert_eq!(summary.command_preview, "printf fixture");

        let poll = service.poll(ProcessJobId(id.clone()), None).await.expect("poll projects output");
        assert_eq!(poll.operation, ProcessJobOperation::Poll);
        assert_eq!(poll.status, Some(ProcessJobStatus::Running));
        assert!(poll.summary.contains("fixture line one"), "{}", poll.summary);

        let log = service
            .log(ProcessJobId(id.clone()), ProcessJobLogRange {
                stream: ProcessJobStream::Combined,
                offset: Some(0),
                limit_bytes: 10,
            })
            .await
            .expect("log projects output snapshot");
        assert!(log.text.contains("[stdout] fixture line one"), "{}", log.text);
        assert!(log.text.contains("[stderr] fixture line two"), "{}", log.text);

        let missing = service
            .poll(ProcessJobId("proc_in_memory_missing".to_string()), None)
            .await
            .expect_err("unknown in-memory id fails closed");
        assert!(missing.to_string().contains("Unknown process session_id"), "{missing}");

        let stdin = service
            .write_stdin(ProcessJobId(id.clone()), b"input".to_vec(), false)
            .await
            .expect_err("in-memory entry has no live stdin");
        assert!(stdin.to_string().contains("has no open stdin"), "{stdin}");
        entry.set_status(ProcessStatus::Exited {
            code: Some(0),
            elapsed: Duration::from_secs(0),
        });
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
}
