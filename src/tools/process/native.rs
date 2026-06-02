use std::collections::HashMap;
use std::sync::LazyLock;

use tokio::io::AsyncBufReadExt;
use tokio::io::BufReader;

use super::*;

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
        native_admission_decision(self.active_or_reserved_count(), limit)
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
        start_native_process_job(
            request,
            self.db.clone(),
            self.process_monitor.as_ref(),
            self.call_id.as_deref(),
        )
        .await
    }

    pub(super) async fn restart(&self, id: ProcessJobId) -> Result<ProcessJobReceipt, RuntimeError> {
        restart_native_process_job(
            id,
            self.db.clone(),
            self.process_monitor.as_ref(),
            self.call_id.as_deref(),
        )
        .await
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
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| RuntimeError::InvalidTool("failed to capture stdout from native background process".to_string()))?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| RuntimeError::InvalidTool("failed to capture stderr from native background process".to_string()))?;
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
        monitor.register(pid, clankers_procmon::ProcessMeta {
            tool_name: "process".to_string(),
            command: ProcessJobRedactionPolicy::default().safe_command_preview(&entry.command),
            call_id: call_id.unwrap_or("process-start").to_string(),
        });
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
}
