use clankers_runtime::process_jobs::DefaultProcessJobNotificationPolicyEngine;
use clankers_runtime::process_jobs::ProcessJobEventId;
use clankers_runtime::process_jobs::ProcessJobNotificationDecision;
use clankers_runtime::process_jobs::ProcessJobNotificationPolicyEngine;

use super::native::ProcessEntry;
use super::native::ProcessStatus;
use super::*;

pub(super) async fn evaluate_process_entry_notification(
    entry: &ProcessEntry,
    observation: ProcessJobNotificationObservation,
) {
    let engine = DefaultProcessJobNotificationPolicyEngine;
    let mut state = entry.notification_state.lock().await;
    let decisions = engine.evaluate(&entry.notification_policy, &mut state, observation).await;
    drop(state);
    for decision in decisions {
        record_process_notification(entry, decision);
    }
}

fn record_process_notification(entry: &ProcessEntry, decision: ProcessJobNotificationDecision) {
    let event = ProcessJobNotificationEvent {
        event_id: ProcessJobEventId(format!(
            "{}_evt_{}",
            entry.id,
            entry.next_notification_seq.fetch_add(1, Ordering::Relaxed) + 1
        )),
        id: ProcessJobId(entry.id.clone()),
        backend: ProcessJobBackendKind::Native,
        owner: ProcessJobOwnerScope::DaemonGlobal,
        kind: decision.kind,
        status: entry.job_status(),
        created_at: Utc::now(),
        summary: decision.summary,
        log_excerpt: decision.log_excerpt,
        log_refs: Vec::new(),
    };
    entry.notifications.lock().expect("process notification lock poisoned").push(event);
}

pub(super) fn stored_record_from_entry(entry: &ProcessEntry) -> StoredProcessJobRecord {
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

pub(super) fn stored_status_label(status: &StoredProcessJobStatus) -> String {
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

pub(super) fn durable_reconciliation_note(record: &StoredProcessJobRecord) -> String {
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

pub(super) fn stored_status_to_job_status(status: &StoredProcessJobStatus) -> ProcessJobStatus {
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

pub(super) fn stored_backend_to_job_backend(backend: StoredProcessJobBackendKind) -> ProcessJobBackendKind {
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

pub(super) fn stored_record_summary(record: &StoredProcessJobRecord) -> ProcessJobSummary {
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
pub(super) fn native_pid_is_alive(pid: u32) -> bool {
    let pid = match libc::pid_t::try_from(pid) {
        Ok(pid) if pid > 0 => pid,
        _ => return false,
    };
    // SAFETY: kill(pid, 0) does not send a signal; it only asks the kernel whether
    // the process exists and is signalable from this daemon's credentials.
    unsafe { libc::kill(pid, 0) == 0 || std::io::Error::last_os_error().raw_os_error() == Some(libc::EPERM) }
}

#[cfg(not(unix))]
pub(super) fn native_pid_is_alive(_pid: u32) -> bool {
    false
}

pub(super) fn reconciled_native_record(mut record: StoredProcessJobRecord) -> StoredProcessJobRecord {
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

pub(super) async fn persist_entry(db: Option<&clankers_db::Db>, entry: &ProcessEntry) {
    let Some(db) = db else {
        return;
    };
    if let Err(error) = db.async_process_jobs().upsert(stored_record_from_entry(entry)).await {
        tracing::warn!("failed to persist native process job metadata: {error}");
    }
}

pub(super) async fn durable_record(db: Option<&clankers_db::Db>, id: &str) -> Option<StoredProcessJobRecord> {
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

pub(super) fn process_job_retention_policy(params: &Value) -> ProcessJobRetentionPolicy {
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

pub(super) fn safe_native_log_path(log_dir: Option<&PathBuf>, reference: &str) -> Option<PathBuf> {
    let log_dir = log_dir?;
    let relative = reference.strip_prefix("native:")?;
    if relative.split('/').any(|part| part.is_empty() || part == "." || part == "..") {
        return None;
    }
    Some(log_dir.join(relative))
}

pub(super) fn retention_log_dir(params: &Value) -> Option<PathBuf> {
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

pub(super) fn append_log_degradation(
    summary: &mut ProcessJobSummary,
    record: &StoredProcessJobRecord,
    log_dir: Option<&PathBuf>,
) {
    if let Some(detail) = log_reference_degradation_detail(record, log_dir) {
        summary.command_preview = format!("{} [{detail}]", summary.command_preview);
    }
}

pub(super) fn durable_degraded_log_message(record: &StoredProcessJobRecord, log_dir: Option<&PathBuf>) -> String {
    let detail = log_reference_degradation_detail(record, log_dir)
        .unwrap_or_else(|| "log_unavailable:live_output_stream_detached".to_string());
    format!(
        "process job {}; {}; {detail}; durable log refs: {}",
        record.id,
        durable_reconciliation_note(record),
        format_log_refs(&record.log_refs)
    )
}

pub(super) async fn apply_process_job_retention(
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

    let live_ids = native::all_entries()
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

pub(super) fn format_log_refs(refs: &[StoredProcessJobLogRef]) -> String {
    if refs.is_empty() {
        return "none".to_string();
    }
    refs.iter().map(|log_ref| log_ref.reference.as_str()).collect::<Vec<_>>().join(", ")
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

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
    async fn durable_policy_helpers_project_redacted_reconciliation_gc_and_notifications_without_root_tool() {
        let db = clankers_db::Db::in_memory().expect("db opens");
        let temp = tempfile::tempdir().expect("tempdir");
        let old = Utc::now() - chrono::Duration::days(30);
        let mut expired = StoredProcessJobRecord::new_native(
            "proc_durable_policy_expired",
            "printf token=raw-token",
            StoredProcessJobOwnerScope::DaemonGlobal,
        );
        expired.status = StoredProcessJobStatus::Succeeded { exit_code: Some(0) };
        expired.started_at = old;
        expired.updated_at = old;
        expired.completed_at = Some(old);
        expired.log_refs = vec![StoredProcessJobLogRef {
            stream: StoredProcessJobStream::Combined,
            reference: "native:proc_durable_policy_expired/combined.log".to_string(),
            retained_until: None,
            max_bytes: Some(17),
        }];
        assert!(!expired.command_preview.contains("raw-token"), "{}", expired.command_preview);
        assert!(expired.command_preview.contains("[REDACTED]"), "{}", expired.command_preview);
        let log_path = temp.path().join("proc_durable_policy_expired").join("combined.log");
        std::fs::create_dir_all(log_path.parent().expect("log parent")).expect("log dir");
        std::fs::write(&log_path, b"expired durable").expect("log write");
        db.async_process_jobs().upsert(expired).await.expect("insert expired");

        let receipt = apply_process_job_retention(
            Some(&db),
            ProcessJobRetentionPolicy {
                max_age: Some(Duration::from_secs(1)),
                max_records: None,
                max_log_bytes: None,
            },
            Some(temp.path().to_path_buf()),
            ProcessJobFilter::default(),
        )
        .await;
        assert_eq!(receipt.removed_records, vec![ProcessJobId("proc_durable_policy_expired".to_string())]);
        assert_eq!(receipt.deleted_native_log_files, 1);
        assert!(!log_path.exists());

        let current_pid = std::process::id();
        let mut running = StoredProcessJobRecord::new_native(
            "proc_durable_policy_running",
            "sleep token=raw-token",
            StoredProcessJobOwnerScope::DaemonGlobal,
        );
        running.status = StoredProcessJobStatus::Running;
        running.os_pid = Some(current_pid);
        running.log_refs = vec![StoredProcessJobLogRef {
            stream: StoredProcessJobStream::Combined,
            reference: "native:proc_durable_policy_running/combined.log".to_string(),
            retained_until: None,
            max_bytes: Some(1),
        }];
        db.async_process_jobs().upsert(running).await.expect("insert running");
        let reconciled = reconcile_durable_native_process_jobs(&db).await;
        let reconciled_running = reconciled
            .iter()
            .find(|record| record.id == "proc_durable_policy_running")
            .expect("running record reconciled");
        assert_eq!(reconciled_running.status, StoredProcessJobStatus::Running);
        assert_eq!(reconciled_running.safe_metadata.get("reconciliation").map(String::as_str), Some("reattached"));

        let mut degraded = StoredProcessJobRecord::new_native(
            "proc_durable_policy_degraded",
            "printf token=raw-token",
            StoredProcessJobOwnerScope::DaemonGlobal,
        );
        degraded.status = StoredProcessJobStatus::LostAfterRestart;
        degraded.safe_metadata.insert("reconciliation".to_string(), "lost-after-restart".to_string());
        degraded.log_refs = vec![StoredProcessJobLogRef {
            stream: StoredProcessJobStream::Combined,
            reference: "native:proc_durable_policy_degraded/combined.log".to_string(),
            retained_until: None,
            max_bytes: Some(9),
        }];
        let mut summary = stored_record_summary(&degraded);
        append_log_degradation(&mut summary, &degraded, Some(&temp.path().to_path_buf()));
        let degraded_message = durable_degraded_log_message(&degraded, Some(&temp.path().to_path_buf()));
        assert!(degraded_message.contains("degraded reconciliation"), "{degraded_message}");
        let degraded_json = serde_json::to_string(&summary).expect("summary serializes");
        assert!(!degraded_json.contains("raw-token"), "{degraded_json}");
        assert!(degraded_json.contains("[REDACTED]"), "{degraded_json}");
        assert!(degraded_json.contains("log_unavailable:native_missing"), "{degraded_json}");

        let (kill_tx, _kill_rx) = oneshot::channel();
        let entry = ProcessEntry::new(
            "proc_durable_policy_notify".to_string(),
            "printf token=raw-token".to_string(),
            native_start_request("printf token=raw-token"),
            None,
            kill_tx,
            None,
            ProcessJobNotificationPolicy {
                notify_on_complete: true,
                watch_patterns: vec!["READY".to_string()],
            },
            None,
        );
        evaluate_process_entry_notification(&entry, ProcessJobNotificationObservation {
            status: ProcessJobStatus::Running,
            line: Some("READY token=raw-token".to_string()),
            tick: 0,
        })
        .await;
        entry.set_status(ProcessStatus::Exited {
            code: Some(0),
            elapsed: Duration::from_secs(0),
        });
        evaluate_process_entry_notification(&entry, ProcessJobNotificationObservation {
            status: entry.job_status(),
            line: Some("done token=raw-token".to_string()),
            tick: 20,
        })
        .await;
        let events = entry.drain_new_notifications();
        assert_eq!(events.len(), 2, "{events:?}");
        assert!(events.iter().any(|event| matches!(event.kind, ProcessJobNotificationKind::WatchPattern { .. })));
        assert!(events.iter().any(|event| matches!(event.kind, ProcessJobNotificationKind::Completion)));
        let events_json = serde_json::to_string(&events).expect("events serialize");
        assert!(!events_json.contains("raw-token"), "{events_json}");
        assert!(events_json.contains("[REDACTED]"), "{events_json}");
    }
}
