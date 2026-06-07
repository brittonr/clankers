//! Agent-visible background process management.
//!
//! This complements the foreground `bash` tool by keeping long-running child
//! processes alive behind stable session IDs. Agents can poll incremental
//! output, inspect logs, wait, send stdin, and terminate processes.

use std::fmt::Write as _;
use std::path::PathBuf;
use std::sync::Arc;
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
use clankers_runtime::process_jobs::ProcessJobBackendCapabilities;
use clankers_runtime::process_jobs::ProcessJobBackendCapabilitiesReceiptExt;
use clankers_runtime::process_jobs::ProcessJobBackendKind;
use clankers_runtime::process_jobs::ProcessJobCwd;
use clankers_runtime::process_jobs::ProcessJobError;
use clankers_runtime::process_jobs::ProcessJobErrorCode;
use clankers_runtime::process_jobs::ProcessJobFilter;
use clankers_runtime::process_jobs::ProcessJobGarbageCollectionFailure;
use clankers_runtime::process_jobs::ProcessJobGarbageCollectionReceipt;
use clankers_runtime::process_jobs::ProcessJobId;
use clankers_runtime::process_jobs::ProcessJobIdentityEnvelope;
use clankers_runtime::process_jobs::ProcessJobLogChunk;
use clankers_runtime::process_jobs::ProcessJobLogCursor;
use clankers_runtime::process_jobs::ProcessJobLogRange;
use clankers_runtime::process_jobs::ProcessJobLogRef;
use clankers_runtime::process_jobs::ProcessJobNativeAdmissionDecision as NativeAdmissionDecision;
use clankers_runtime::process_jobs::ProcessJobNativeAdmissionInput;
use clankers_runtime::process_jobs::ProcessJobNotificationEvent;
use clankers_runtime::process_jobs::ProcessJobNotificationKind;
use clankers_runtime::process_jobs::ProcessJobNotificationObservation;
use clankers_runtime::process_jobs::ProcessJobNotificationPolicy;
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
use clankers_runtime::process_jobs::native_process_job_admission_decision as native_admission_decision;
use clankers_runtime::process_jobs::process_job_timestamp;
use clankers_util::ansi::strip_ansi;
use serde_json::Value;
use serde_json::json;
use tokio::io::AsyncWriteExt;
use tokio::process::ChildStdin;
use tokio::process::Command;
use tokio::sync::oneshot;

use super::Tool;
use super::ToolContext;
use super::ToolDefinition;
use super::ToolResult;

#[async_trait]
pub(super) trait ProcessJobCommandRunner: Send + Sync {
    async fn run_command(&self, program: &str, args: &[String]) -> Result<String, RuntimeError>;
}

#[derive(Debug, Default, Clone, Copy)]
pub(super) struct TokioProcessJobCommandRunner;

#[async_trait]
impl ProcessJobCommandRunner for TokioProcessJobCommandRunner {
    async fn run_command(&self, program: &str, args: &[String]) -> Result<String, RuntimeError> {
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

mod adapter;
mod durable;
mod native;
mod pueue;
mod systemd;
use adapter::ProcessToolJsonAdapter;
use durable::append_log_degradation;
use durable::apply_process_job_retention;
use durable::durable_degraded_log_message;
use durable::durable_reconciliation_note;
use durable::durable_record;
use durable::evaluate_process_entry_notification;
use durable::format_log_refs;
use durable::native_pid_is_alive;
use durable::persist_entry;
use durable::process_job_retention_policy;
pub(crate) use durable::reconcile_durable_native_process_jobs;
use durable::retention_log_dir;
use durable::stored_record_summary;
use durable::stored_status_label;
use native::NativeProcessJobBackendAdapter;
use native::NativeProcessJobService;
#[cfg(test)]
use native::native_pid_from_backend_ref;
use pueue::PueueProcessJobService;
#[cfg(test)]
use pueue::PueueRunner;
use systemd::SystemdProcessJobService;
#[cfg(test)]
use systemd::SystemdRunner;

const DEFAULT_LOG_LIMIT: usize = 200;
const MAX_COMMAND_PREVIEW_LEN: usize = 200;
const MAX_NATIVE_ACTIVE_PROCESS_JOBS: usize = 32;
const NATIVE_KILL_GRACE: Duration = Duration::from_secs(2);
const NATIVE_RESTART_TERMINATION_TIMEOUT: Duration = Duration::from_secs(5);
const NATIVE_RESTART_TERMINATION_POLL: Duration = Duration::from_millis(50);
const ADOPTED_NATIVE_ID_PREFIX: &str = "native_pid_";

#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ProcessJobPolicyCluster {
    RootProjection,
    NativeBackend,
    PueueBackend,
    SystemdBackend,
    DurableStorage,
    RetentionGarbageCollection,
    NotificationDelivery,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ProcessJobBackendOwnershipEntry {
    cluster: ProcessJobPolicyCluster,
    current_owner: &'static str,
    target_owner: &'static str,
    root_accountability: &'static str,
    migration_step: &'static str,
}

#[allow(dead_code)]
const PROCESS_JOB_BACKEND_ADAPTER_OWNERSHIP: &[ProcessJobBackendOwnershipEntry] = &[
    ProcessJobBackendOwnershipEntry {
        cluster: ProcessJobPolicyCluster::RootProjection,
        current_owner: "src/tools/process.rs::ProcessTool",
        target_owner: "src/tools/process.rs::ProcessTool",
        root_accountability: "parse request, select backend service, project typed receipts",
        migration_step: "keep root file as projection shell",
    },
    ProcessJobBackendOwnershipEntry {
        cluster: ProcessJobPolicyCluster::NativeBackend,
        current_owner: "src/tools/process/native.rs::NativeProcessJobBackendAdapter",
        target_owner: "src/tools/process/native.rs::NativeProcessJobBackendAdapter",
        root_accountability: "select native backend and project typed ProcessJobReceipt",
        migration_step: "I2 native adapter extraction complete",
    },
    ProcessJobBackendOwnershipEntry {
        cluster: ProcessJobPolicyCluster::PueueBackend,
        current_owner: "src/tools/process/pueue.rs::PueueProcessJobService",
        target_owner: "src/tools/process/pueue.rs::PueueProcessJobService",
        root_accountability: "select pueue backend and surface degraded unavailable receipts",
        migration_step: "I3 pueue fakeable runner extraction",
    },
    ProcessJobBackendOwnershipEntry {
        cluster: ProcessJobPolicyCluster::SystemdBackend,
        current_owner: "src/tools/process/systemd.rs::SystemdProcessJobService",
        target_owner: "src/tools/process/systemd.rs::SystemdProcessJobService",
        root_accountability: "select systemd backend and surface degraded unsupported receipts",
        migration_step: "I4 systemd fakeable runner extraction",
    },
    ProcessJobBackendOwnershipEntry {
        cluster: ProcessJobPolicyCluster::DurableStorage,
        current_owner: "src/tools/process/durable.rs::stored_record_from_entry",
        target_owner: "src/tools/process/durable.rs::ProcessJobDurableRecordPolicy",
        root_accountability: "wire optional durable storage service",
        migration_step: "I5 durable reconciliation extraction",
    },
    ProcessJobBackendOwnershipEntry {
        cluster: ProcessJobPolicyCluster::RetentionGarbageCollection,
        current_owner: "src/tools/process/durable.rs::apply_process_job_retention",
        target_owner: "src/tools/process/durable.rs::ProcessJobRetentionPolicyService",
        root_accountability: "invoke retention service and project typed GC receipt",
        migration_step: "I5 retention and log-degradation extraction",
    },
    ProcessJobBackendOwnershipEntry {
        cluster: ProcessJobPolicyCluster::NotificationDelivery,
        current_owner: "src/tools/process/durable.rs::evaluate_process_entry_notification",
        target_owner: "src/tools/process/durable.rs::ProcessJobNotificationPolicyService",
        root_accountability: "wire notification sink and project redacted observations",
        migration_step: "I5 notification delivery extraction",
    },
];

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
        match NativeProcessJobBackendAdapter::for_invocation(
            ctx.service::<clankers_db::Db>().cloned(),
            self.process_monitor.clone(),
            ctx.call_id.clone(),
        )
        .start(request)
        .await
        {
            Ok(receipt) => Self::tool_receipt_result(ProcessJobToolResult::Start(receipt)),
            Err(error) => ToolResult::error(error.to_string()),
        }
    }

    async fn handle_list(ctx: &ToolContext, params: &Value) -> ToolResult {
        let request = match Self::process_job_tool_request(params) {
            Ok(ProcessJobToolRequest::List(request)) => request,
            Ok(_) => return ToolResult::error("Parsed unexpected process job request for list action"),
            Err(result) => return result,
        };
        let policy = process_job_retention_policy(&json!({}));
        let log_dir = retention_log_dir(params);
        let _ = apply_process_job_retention(ctx.service::<clankers_db::Db>(), policy, log_dir.clone(), ProcessJobFilter::default()).await;
        let backend_filter = request.filter.backend;
        let mut summaries = Vec::new();
        if backend_filter.is_none_or(|backend| backend == ProcessJobBackendKind::Native) {
            let entries = native::all_entries();
            let mut durable = Vec::new();
            if let Some(db) = ctx.service::<clankers_db::Db>() {
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
        let receipt = apply_process_job_retention(ctx.service::<clankers_db::Db>(), policy, retention_log_dir(params), request.filter).await;
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
                Err(error) => match durable_record(ctx.service::<clankers_db::Db>(), &session_id).await {
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
                Err(error) => match durable_record(ctx.service::<clankers_db::Db>(), &session_id).await {
                    Some(record) => ToolResult::text(format!(
                        "{}; backend poll unavailable: {error}",
                        durable_degraded_log_message(&record, retention_log_dir(params).as_ref())
                    )),
                    None => ToolResult::error(error.to_string()),
                },
            };
        }
        let entry = match native::get(&session_id) {
            Some(entry) => entry,
            None => {
                if let Some(record) = durable_record(ctx.service::<clankers_db::Db>(), &session_id).await {
                    return ToolResult::text(durable_degraded_log_message(&record, retention_log_dir(params).as_ref()));
                }
                return ToolResult::error(format!("Unknown process session_id: {session_id}"));
            }
        };
        persist_entry(ctx.service::<clankers_db::Db>(), &entry).await;
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
                Ok(chunk) if chunk.text.is_empty() => match durable_record(ctx.service::<clankers_db::Db>(), &session_id).await {
                    Some(record) => ToolResult::text(format!(
                        "{}; backend log read returned no output",
                        durable_degraded_log_message(&record, retention_log_dir(params).as_ref())
                    )),
                    None => Self::tool_receipt_result(ProcessJobToolResult::Log(chunk)),
                },
                Ok(chunk) => Self::tool_receipt_result(ProcessJobToolResult::Log(chunk)),
                Err(error) => match durable_record(ctx.service::<clankers_db::Db>(), &session_id).await {
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
                Ok(chunk) if chunk.text.is_empty() => match durable_record(ctx.service::<clankers_db::Db>(), &session_id).await {
                    Some(record) => ToolResult::text(format!(
                        "{}; backend log read returned no output",
                        durable_degraded_log_message(&record, retention_log_dir(params).as_ref())
                    )),
                    None => Self::tool_receipt_result(ProcessJobToolResult::Log(chunk)),
                },
                Ok(chunk) => Self::tool_receipt_result(ProcessJobToolResult::Log(chunk)),
                Err(error) => match durable_record(ctx.service::<clankers_db::Db>(), &session_id).await {
                    Some(record) => ToolResult::text(format!(
                        "{}; backend log read unavailable: {error}",
                        durable_degraded_log_message(&record, retention_log_dir(params).as_ref())
                    )),
                    None => ToolResult::error(error.to_string()),
                },
            };
        }
        let entry = match native::get(&session_id) {
            Some(entry) => entry,
            None => {
                if let Some(record) = durable_record(ctx.service::<clankers_db::Db>(), &session_id).await {
                    return ToolResult::text(durable_degraded_log_message(&record, retention_log_dir(params).as_ref()));
                }
                return ToolResult::error(format!("Unknown process session_id: {session_id}"));
            }
        };
        persist_entry(ctx.service::<clankers_db::Db>(), &entry).await;
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
        let entry = match native::get(&session_id) {
            Some(entry) => entry,
            None => {
                if let Some(record) = durable_record(ctx.service::<clankers_db::Db>(), &session_id).await {
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
                persist_entry(ctx.service::<clankers_db::Db>(), &entry).await;
                return ToolResult::text(format!("{} still running after {}s", entry.id, timeout_secs));
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        persist_entry(ctx.service::<clankers_db::Db>(), &entry).await;
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
        let entry = match native::get(&session_id) {
            Some(entry) => entry,
            None => {
                if let Some(record) = durable_record(ctx.service::<clankers_db::Db>(), &session_id).await {
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
        let entry = match native::get(&session_id) {
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
        let entry = match native::get(&session_id) {
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
        match NativeProcessJobBackendAdapter::for_invocation(
            ctx.service::<clankers_db::Db>().cloned(),
            self.process_monitor.clone(),
            ctx.call_id.clone(),
        )
        .restart(request.id)
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

#[cfg(test)]
mod tests {
    use serde_json::json;
    use tokio_util::sync::CancellationToken;

    use super::*;

    const SHORT_PROCESS_TEST_TIMEOUT_SECS: u64 = 2;
    const RESTART_PERSISTENCE_SETTLE_MILLIS: u64 = 100;
    const GC_TEST_OLD_RECORD_SECS: i64 = 2;
    const GC_TEST_MAX_RECORDS: u64 = 100;
    const GC_TEST_LOG_BUDGET_BYTES: u64 = 1_000_000;

    fn make_ctx() -> ToolContext {
        ToolContext::new("process-test".to_string(), CancellationToken::new(), None)
    }

    fn make_ctx_with_db(db: clankers_db::Db) -> ToolContext {
        make_ctx().with_service(std::sync::Arc::new(db))
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
    async fn process_tool_adopt_routes_to_backend_service_seams() {
        let tool = ProcessTool::new();
        let adopted = tool
            .execute(&make_ctx(), json!({"action": "adopt", "backend": "native", "pid": std::process::id()}))
            .await;
        assert!(!adopted.is_error, "{adopted:?}");
        assert!(text(&adopted).contains("native_pid_"), "{}", text(&adopted));
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
        let accepted = native_admission_decision(ProcessJobNativeAdmissionInput {
            active: MAX_NATIVE_ACTIVE_PROCESS_JOBS - 1,
            limit: MAX_NATIVE_ACTIVE_PROCESS_JOBS,
        });
        assert!(accepted.accepted);

        let rejected = native_admission_decision(ProcessJobNativeAdmissionInput {
            active: MAX_NATIVE_ACTIVE_PROCESS_JOBS,
            limit: MAX_NATIVE_ACTIVE_PROCESS_JOBS,
        });
        assert!(!rejected.accepted);
        let receipt = native::admission_denied_receipt(rejected);
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
        let mut registry = native::ProcessRegistry::default();
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
