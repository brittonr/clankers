//! Backend-neutral process/job contracts.
//!
//! These types are the stable seam between the agent-visible `process` tool,
//! service orchestration, storage, log backends, notification delivery, and UI
//! projections. Concrete native, pueue, systemd, redb, TUI, and daemon adapters
//! should depend on these DTOs rather than on each other.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::time::Duration;

use async_trait::async_trait;
use chrono::DateTime;
use chrono::Utc;
use serde::Deserialize;
use serde::Serialize;

use crate::RuntimeError;

/// Stable Clankers-owned process/job identifier.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ProcessJobId(pub String);

/// Backend-owned reference, such as a PID/process-group, pueue task id, or systemd unit name.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct BackendRef(pub String);

/// Durable notification event id for completion/readiness delivery and deduplication.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ProcessJobEventId(pub String);

/// Supported backend families. Unknown is retained for forward-compatible stored records.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProcessJobBackendKind {
    Native,
    Pueue,
    Systemd,
    Unknown,
}

/// Backend-neutral operation vocabulary used for capability checks and receipts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProcessJobOperation {
    Start,
    List,
    Poll,
    Log,
    Wait,
    Kill,
    Restart,
    WriteStdin,
    CloseStdin,
    Adopt,
    GarbageCollect,
}

/// Shared status vocabulary for native processes and durable queue/supervisor jobs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "state")]
pub enum ProcessJobStatus {
    Pending,
    Running,
    Waiting,
    Succeeded { exit_code: Option<i32> },
    Failed { exit_code: Option<i32>, reason: String },
    Killed,
    Cancelled,
    LostAfterRestart,
    ReattachedLogIncomplete,
    BackendUnavailable { reason: String },
    Unknown { raw: String },
}

impl ProcessJobStatus {
    #[must_use]
    pub fn is_terminal(&self) -> bool {
        matches!(
            self,
            Self::Succeeded { .. }
                | Self::Failed { .. }
                | Self::Killed
                | Self::Cancelled
                | Self::LostAfterRestart
                | Self::BackendUnavailable { .. }
        )
    }
}

/// Scope used to authorize cross-session observation and mutation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind", content = "value")]
pub enum ProcessJobOwnerScope {
    Session(String),
    Workspace(String),
    User(String),
    DaemonGlobal,
}

/// Caller identity used by capability policy checks.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct ProcessJobCallerScope {
    pub session_id: Option<String>,
    pub workspace_id: Option<String>,
    pub user_id: Option<String>,
    pub daemon_global: bool,
    pub capabilities: ProcessJobCapabilitySet,
}

/// Capability classes for read-only observation, log access, execution, mutation, and backend use.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct ProcessJobCapabilitySet {
    pub observe: bool,
    pub read_logs: bool,
    pub start: bool,
    pub mutate: bool,
    pub stdin: bool,
    pub select_backend: bool,
}

impl ProcessJobCapabilitySet {
    #[must_use]
    pub fn observe_only() -> Self {
        Self {
            observe: true,
            read_logs: true,
            ..Self::default()
        }
    }

    #[must_use]
    pub fn full_control() -> Self {
        Self {
            observe: true,
            read_logs: true,
            start: true,
            mutate: true,
            stdin: true,
            select_backend: true,
        }
    }

    #[must_use]
    pub fn allows_operation(&self, operation: ProcessJobOperation, backend: ProcessJobBackendKind) -> bool {
        match operation {
            ProcessJobOperation::List | ProcessJobOperation::Poll => self.observe,
            ProcessJobOperation::Log => self.observe && self.read_logs,
            ProcessJobOperation::Start => {
                self.start && (backend == ProcessJobBackendKind::Native || self.select_backend)
            }
            ProcessJobOperation::Kill
            | ProcessJobOperation::Restart
            | ProcessJobOperation::Adopt
            | ProcessJobOperation::GarbageCollect => self.mutate,
            ProcessJobOperation::WriteStdin | ProcessJobOperation::CloseStdin => self.mutate && self.stdin,
            ProcessJobOperation::Wait => self.observe,
        }
    }
}

impl ProcessJobCallerScope {
    #[must_use]
    pub fn matches_owner(&self, owner: &ProcessJobOwnerScope) -> bool {
        match owner {
            ProcessJobOwnerScope::Session(session) => self.session_id.as_deref() == Some(session.as_str()),
            ProcessJobOwnerScope::Workspace(workspace) => self.workspace_id.as_deref() == Some(workspace.as_str()),
            ProcessJobOwnerScope::User(user) => self.user_id.as_deref() == Some(user.as_str()),
            ProcessJobOwnerScope::DaemonGlobal => self.daemon_global,
        }
    }

    #[must_use]
    pub fn can_access(
        &self,
        owner: &ProcessJobOwnerScope,
        operation: ProcessJobOperation,
        backend: ProcessJobBackendKind,
    ) -> bool {
        self.matches_owner(owner) && self.capabilities.allows_operation(operation, backend)
    }
}

/// Command working-directory policy recorded safely in metadata.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind", content = "path")]
pub enum ProcessJobCwd {
    Inherited,
    Explicit(PathBuf),
}

/// Log stream selector for append-only files or backend logs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProcessJobStream {
    Stdout,
    Stderr,
    Combined,
}

/// Opaque safe reference to native log files or backend log cursors.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProcessJobLogRef {
    pub stream: ProcessJobStream,
    pub reference: String,
    pub retained_until: Option<DateTime<Utc>>,
    pub max_bytes: Option<u64>,
}

/// Native append-only log file naming/layout policy.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NativeProcessJobLogLayout {
    pub job_id: ProcessJobId,
    pub stream: ProcessJobStream,
    pub relative_path: PathBuf,
    pub reference: String,
}

impl NativeProcessJobLogLayout {
    #[must_use]
    pub fn for_stream(job_id: ProcessJobId, stream: ProcessJobStream) -> Self {
        let suffix = match stream {
            ProcessJobStream::Stdout => "stdout.log",
            ProcessJobStream::Stderr => "stderr.log",
            ProcessJobStream::Combined => "combined.log",
        };
        let safe_id = sanitize_log_path_component(&job_id.0);
        let relative_path = PathBuf::from(&safe_id).join(suffix);
        let reference = format!("native:{safe_id}/{suffix}");
        Self {
            job_id,
            stream,
            relative_path,
            reference,
        }
    }
}

/// Log retention policy applied by native append-only log stores.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProcessJobLogRetentionPolicy {
    pub max_bytes_per_job: u64,
    pub max_age: Option<Duration>,
    pub keep_terminal_logs: bool,
}

impl ProcessJobLogRetentionPolicy {
    #[must_use]
    pub fn reference_for(
        &self,
        job_id: ProcessJobId,
        stream: ProcessJobStream,
        now: DateTime<Utc>,
    ) -> ProcessJobLogRef {
        let layout = NativeProcessJobLogLayout::for_stream(job_id, stream);
        let retained_until = self.max_age.and_then(|age| chrono::Duration::from_std(age).ok()).map(|age| now + age);
        ProcessJobLogRef {
            stream,
            reference: layout.reference,
            retained_until,
            max_bytes: Some(self.max_bytes_per_job),
        }
    }
}

fn sanitize_log_path_component(input: &str) -> String {
    input
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.') {
                ch
            } else {
                '_'
            }
        })
        .collect()
}

/// Cursor for incremental log reads.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProcessJobLogCursor {
    pub stream: ProcessJobStream,
    pub offset: u64,
}

/// Bounded range for log reads.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProcessJobLogRange {
    pub stream: ProcessJobStream,
    pub offset: Option<u64>,
    pub limit_bytes: u64,
}

/// Bounded log chunk returned by service/backend APIs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProcessJobLogChunk {
    pub id: ProcessJobId,
    pub backend: ProcessJobBackendKind,
    pub stream: ProcessJobStream,
    pub cursor: ProcessJobLogCursor,
    pub next_cursor: Option<ProcessJobLogCursor>,
    pub text: String,
    pub truncated: bool,
}

pub const MAX_PROCESS_JOB_WATCH_PATTERNS: usize = 8;
pub const MAX_PROCESS_JOB_WATCH_PATTERN_LEN: usize = 128;
pub const PROCESS_JOB_WATCH_RATE_LIMIT_TICKS: u64 = 15;
pub const PROCESS_JOB_WATCH_SUPPRESSION_LIMIT: u32 = 3;

/// Accepted notification policy. Continuous output stays in logs.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct ProcessJobNotificationPolicy {
    #[serde(default)]
    pub notify_on_complete: bool,
    #[serde(default)]
    pub watch_patterns: Vec<String>,
}

impl ProcessJobNotificationPolicy {
    #[must_use]
    pub fn bounded_watch_patterns(&self) -> Vec<String> {
        self.watch_patterns
            .iter()
            .filter_map(|pattern| {
                let trimmed = pattern.trim();
                (!trimmed.is_empty())
                    .then(|| trimmed.chars().take(MAX_PROCESS_JOB_WATCH_PATTERN_LEN).collect::<String>())
            })
            .take(MAX_PROCESS_JOB_WATCH_PATTERNS)
            .collect()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind")]
pub enum ProcessJobNotificationKind {
    Completion,
    WatchPattern { pattern_index: usize, pattern: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProcessJobNotificationDecision {
    pub kind: ProcessJobNotificationKind,
    pub summary: String,
    pub log_excerpt: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProcessJobNotificationObservation {
    pub status: ProcessJobStatus,
    pub line: Option<String>,
    pub tick: u64,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ProcessJobNotificationPolicyState {
    completion_sent: bool,
    watch_states: Vec<ProcessJobWatchPatternState>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct ProcessJobWatchPatternState {
    last_delivered_tick: Option<u64>,
    suppressed_matches: u32,
    disabled: bool,
}

#[async_trait]
pub trait ProcessJobNotificationPolicyEngine: Send + Sync {
    async fn evaluate(
        &self,
        policy: &ProcessJobNotificationPolicy,
        state: &mut ProcessJobNotificationPolicyState,
        observation: ProcessJobNotificationObservation,
    ) -> Vec<ProcessJobNotificationDecision>;
}

#[derive(Debug, Default, Clone, Copy)]
pub struct DefaultProcessJobNotificationPolicyEngine;

#[async_trait]
impl ProcessJobNotificationPolicyEngine for DefaultProcessJobNotificationPolicyEngine {
    async fn evaluate(
        &self,
        policy: &ProcessJobNotificationPolicy,
        state: &mut ProcessJobNotificationPolicyState,
        observation: ProcessJobNotificationObservation,
    ) -> Vec<ProcessJobNotificationDecision> {
        let mut decisions = Vec::new();
        if observation.status.is_terminal() && policy.notify_on_complete && !state.completion_sent {
            state.completion_sent = true;
            decisions.push(ProcessJobNotificationDecision {
                kind: ProcessJobNotificationKind::Completion,
                summary: format!("process job reached terminal status: {:?}", observation.status),
                log_excerpt: observation.line.clone(),
            });
        }

        let Some(line) = observation.line else {
            return decisions;
        };
        let patterns = policy.bounded_watch_patterns();
        if state.watch_states.len() < patterns.len() {
            state.watch_states.resize_with(patterns.len(), ProcessJobWatchPatternState::default);
        }
        for (pattern_index, pattern) in patterns.iter().enumerate() {
            if !line.contains(pattern) {
                continue;
            }
            let watch_state = &mut state.watch_states[pattern_index];
            if watch_state.disabled {
                continue;
            }
            let rate_limited = watch_state
                .last_delivered_tick
                .is_some_and(|last| observation.tick.saturating_sub(last) < PROCESS_JOB_WATCH_RATE_LIMIT_TICKS);
            if rate_limited {
                watch_state.suppressed_matches = watch_state.suppressed_matches.saturating_add(1);
                if watch_state.suppressed_matches >= PROCESS_JOB_WATCH_SUPPRESSION_LIMIT {
                    watch_state.disabled = true;
                }
                continue;
            }
            watch_state.last_delivered_tick = Some(observation.tick);
            watch_state.suppressed_matches = 0;
            decisions.push(ProcessJobNotificationDecision {
                kind: ProcessJobNotificationKind::WatchPattern {
                    pattern_index,
                    pattern: pattern.clone(),
                },
                summary: format!("process job matched readiness pattern {pattern_index}: {pattern}"),
                log_excerpt: Some(line.clone()),
            });
        }
        decisions
    }
}

/// Resource limits accepted by policy before backend dispatch.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct ProcessJobResourcePolicy {
    pub timeout: Option<Duration>,
    pub memory_max_bytes: Option<u64>,
    pub cpu_quota_percent: Option<u32>,
    pub max_log_bytes: Option<u64>,
}

/// A backend-neutral start specification.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StartProcessJobRequest {
    pub backend: ProcessJobBackendKind,
    pub command_preview: String,
    pub program: Option<String>,
    pub args: Vec<String>,
    pub shell_command: Option<String>,
    pub cwd: ProcessJobCwd,
    pub owner: ProcessJobOwnerScope,
    pub resource_policy: ProcessJobResourcePolicy,
    pub notification_policy: ProcessJobNotificationPolicy,
    pub metadata: BTreeMap<String, String>,
}

/// Query filter for process/job list operations.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct ProcessJobFilter {
    pub owner: Option<ProcessJobOwnerScope>,
    pub backend: Option<ProcessJobBackendKind>,
    pub include_terminal: bool,
}

/// Service-level process/job summary safe for list/status surfaces.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProcessJobSummary {
    pub id: ProcessJobId,
    pub backend: ProcessJobBackendKind,
    pub backend_ref: Option<BackendRef>,
    pub owner: ProcessJobOwnerScope,
    pub status: ProcessJobStatus,
    pub command_preview: String,
    pub cwd: ProcessJobCwd,
    pub started_at: Option<DateTime<Utc>>,
    pub updated_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
    pub log_refs: Vec<ProcessJobLogRef>,
}

/// Backend capability descriptor used before dispatching mutations.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct ProcessJobBackendCapabilities {
    pub backend: Option<ProcessJobBackendKind>,
    pub supports_shell: bool,
    pub supports_direct_exec: bool,
    pub supports_stdin: bool,
    pub supports_restart: bool,
    pub supports_kill: bool,
    pub supports_adopt: bool,
    pub supports_resource_limits: bool,
    pub supports_log_range: bool,
    pub durable_across_daemon_restart: bool,
    pub unavailable_reason: Option<String>,
}

impl ProcessJobBackendCapabilities {
    #[must_use]
    pub fn unavailable(backend: ProcessJobBackendKind, reason: impl Into<String>) -> Self {
        Self {
            backend: Some(backend),
            unavailable_reason: Some(reason.into()),
            ..Self::default()
        }
    }
}

/// Typed error code for receipts and projection surfaces.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProcessJobErrorCode {
    NotFound,
    PermissionDenied,
    BackendUnavailable,
    UnsupportedActionForBackend,
    InvalidRequest,
    AdmissionDenied,
    ConcurrencyLimitExceeded,
    StorageUnavailable,
    LogUnavailable,
    BackendFailed,
}

/// Safe machine-readable error detail.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProcessJobError {
    pub code: ProcessJobErrorCode,
    pub operation: ProcessJobOperation,
    pub id: Option<ProcessJobId>,
    pub backend: Option<ProcessJobBackendKind>,
    pub action: Option<String>,
    pub message: String,
}

/// Import/adoption request for externally-created process jobs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdoptProcessJobRequest {
    pub backend: ProcessJobBackendKind,
    pub backend_ref: BackendRef,
    pub owner: ProcessJobOwnerScope,
    pub caller: ProcessJobCallerScope,
}

impl AdoptProcessJobRequest {
    #[must_use]
    pub fn is_authorized(&self) -> bool {
        self.caller.can_access(&self.owner, ProcessJobOperation::Adopt, self.backend)
            && (self.backend == ProcessJobBackendKind::Native || self.caller.capabilities.select_backend)
    }
}

/// Shared receipt for mutations and state transitions.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProcessJobReceipt {
    pub operation: ProcessJobOperation,
    pub id: Option<ProcessJobId>,
    pub backend: Option<ProcessJobBackendKind>,
    pub status: Option<ProcessJobStatus>,
    pub backend_ref: Option<BackendRef>,
    pub log_refs: Vec<ProcessJobLogRef>,
    pub summary: String,
    pub error: Option<ProcessJobError>,
}

impl ProcessJobReceipt {
    #[must_use]
    pub fn permission_denied(
        operation: ProcessJobOperation,
        backend: ProcessJobBackendKind,
        action: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        let message = message.into();
        Self {
            operation,
            id: None,
            backend: Some(backend),
            status: None,
            backend_ref: None,
            log_refs: Vec::new(),
            summary: message.clone(),
            error: Some(ProcessJobError {
                code: ProcessJobErrorCode::PermissionDenied,
                operation,
                id: None,
                backend: Some(backend),
                action: Some(action.into()),
                message,
            }),
        }
    }

    #[must_use]
    pub fn unsupported(
        operation: ProcessJobOperation,
        id: Option<ProcessJobId>,
        backend: ProcessJobBackendKind,
        action: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        let message = message.into();
        Self {
            operation,
            id: id.clone(),
            backend: Some(backend),
            status: None,
            backend_ref: None,
            log_refs: Vec::new(),
            summary: message.clone(),
            error: Some(ProcessJobError {
                code: ProcessJobErrorCode::UnsupportedActionForBackend,
                operation,
                id,
                backend: Some(backend),
                action: Some(action.into()),
                message,
            }),
        }
    }
}

/// Typed tool result surface for every durable process/job operation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "operation", content = "result")]
pub enum ProcessJobToolResult {
    Start(ProcessJobReceipt),
    List(Vec<ProcessJobSummary>),
    Poll(ProcessJobReceipt),
    Log(ProcessJobLogChunk),
    Wait(ProcessJobReceipt),
    Kill(ProcessJobReceipt),
    Restart(ProcessJobReceipt),
    WriteStdin(ProcessJobReceipt),
    CloseStdin(ProcessJobReceipt),
    Adopt(ProcessJobReceipt),
    GarbageCollect(ProcessJobReceipt),
}

/// Backend result after accepting a start request.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProcessJobBackendStart {
    pub backend_ref: BackendRef,
    pub status: ProcessJobStatus,
    pub log_refs: Vec<ProcessJobLogRef>,
}

/// Backend-observed status payload.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProcessJobBackendStatus {
    pub backend_ref: BackendRef,
    pub status: ProcessJobStatus,
    pub updated_at: DateTime<Utc>,
    pub log_refs: Vec<ProcessJobLogRef>,
}

/// Persisted notification event payload.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProcessJobNotificationEvent {
    pub event_id: ProcessJobEventId,
    pub id: ProcessJobId,
    pub backend: ProcessJobBackendKind,
    pub owner: ProcessJobOwnerScope,
    pub kind: ProcessJobNotificationKind,
    pub status: ProcessJobStatus,
    pub created_at: DateTime<Utc>,
    pub summary: String,
    pub log_excerpt: Option<String>,
    pub log_refs: Vec<ProcessJobLogRef>,
}

/// Tool-facing service boundary. Implementations own policy orchestration and storage wiring.
#[async_trait]
pub trait ProcessJobService: Send + Sync {
    async fn start(&self, request: StartProcessJobRequest) -> Result<ProcessJobReceipt, RuntimeError>;
    async fn list(&self, filter: ProcessJobFilter) -> Result<Vec<ProcessJobSummary>, RuntimeError>;
    async fn poll(
        &self,
        id: ProcessJobId,
        cursor: Option<ProcessJobLogCursor>,
    ) -> Result<ProcessJobReceipt, RuntimeError>;
    async fn log(&self, id: ProcessJobId, range: ProcessJobLogRange) -> Result<ProcessJobLogChunk, RuntimeError>;
    async fn wait(&self, id: ProcessJobId, timeout: Option<Duration>) -> Result<ProcessJobReceipt, RuntimeError>;
    async fn kill(&self, id: ProcessJobId) -> Result<ProcessJobReceipt, RuntimeError>;
    async fn restart(&self, id: ProcessJobId) -> Result<ProcessJobReceipt, RuntimeError>;
    async fn write_stdin(
        &self,
        id: ProcessJobId,
        data: Vec<u8>,
        newline: bool,
    ) -> Result<ProcessJobReceipt, RuntimeError>;
    async fn close_stdin(&self, id: ProcessJobId) -> Result<ProcessJobReceipt, RuntimeError>;
    async fn adopt(&self, request: AdoptProcessJobRequest) -> Result<ProcessJobReceipt, RuntimeError>;
    async fn garbage_collect(&self, filter: ProcessJobFilter) -> Result<ProcessJobReceipt, RuntimeError>;
}

/// Backend adapter boundary. Backends expose facts and capabilities; they do not own UI/storage
/// policy.
#[async_trait]
pub trait ProcessJobBackend: Send + Sync {
    fn kind(&self) -> ProcessJobBackendKind;
    fn capabilities(&self) -> ProcessJobBackendCapabilities;
    async fn start(&self, request: StartProcessJobRequest) -> Result<ProcessJobBackendStart, RuntimeError>;
    async fn observe(&self, backend_ref: BackendRef) -> Result<ProcessJobBackendStatus, RuntimeError>;
    async fn log(&self, backend_ref: BackendRef, range: ProcessJobLogRange)
    -> Result<ProcessJobLogChunk, RuntimeError>;
    async fn kill(&self, backend_ref: BackendRef) -> Result<ProcessJobReceipt, RuntimeError>;
    async fn restart(&self, backend_ref: BackendRef) -> Result<ProcessJobReceipt, RuntimeError>;
    async fn write_stdin(
        &self,
        backend_ref: BackendRef,
        data: Vec<u8>,
        newline: bool,
    ) -> Result<ProcessJobReceipt, RuntimeError>;
    async fn close_stdin(&self, backend_ref: BackendRef) -> Result<ProcessJobReceipt, RuntimeError>;
}

/// Metadata persistence boundary, backed by redb in production and fakes in tests.
#[async_trait]
pub trait ProcessJobStore: Send + Sync {
    async fn upsert(&self, summary: ProcessJobSummary) -> Result<(), RuntimeError>;
    async fn get(&self, id: ProcessJobId) -> Result<Option<ProcessJobSummary>, RuntimeError>;
    async fn list(&self, filter: ProcessJobFilter) -> Result<Vec<ProcessJobSummary>, RuntimeError>;
    async fn record_notification(&self, event: ProcessJobNotificationEvent) -> Result<(), RuntimeError>;
    async fn list_notifications(
        &self,
        caller: ProcessJobCallerScope,
        after: Option<ProcessJobEventId>,
    ) -> Result<Vec<ProcessJobNotificationEvent>, RuntimeError>;
}

/// Log storage boundary. Native implementations can append files; pueue/systemd can store
/// references.
#[async_trait]
pub trait ProcessJobLogStore: Send + Sync {
    async fn append(
        &self,
        id: ProcessJobId,
        stream: ProcessJobStream,
        chunk: &[u8],
    ) -> Result<ProcessJobLogCursor, RuntimeError>;
    async fn read(&self, id: ProcessJobId, range: ProcessJobLogRange) -> Result<ProcessJobLogChunk, RuntimeError>;
    async fn references(&self, id: ProcessJobId) -> Result<Vec<ProcessJobLogRef>, RuntimeError>;
}

/// Delivery boundary for completion/readiness notifications.
#[async_trait]
pub trait ProcessJobNotificationSink: Send + Sync {
    async fn deliver(&self, event: ProcessJobNotificationEvent) -> Result<(), RuntimeError>;
}

pub async fn persist_and_deliver_notification(
    store: &dyn ProcessJobStore,
    sink: &dyn ProcessJobNotificationSink,
    event: ProcessJobNotificationEvent,
) -> Result<(), RuntimeError> {
    store.record_notification(event.clone()).await?;
    sink.deliver(event).await
}

pub async fn replay_authorized_notifications(
    store: &dyn ProcessJobStore,
    caller: ProcessJobCallerScope,
    after: Option<ProcessJobEventId>,
) -> Result<Vec<ProcessJobNotificationEvent>, RuntimeError> {
    store.list_notifications(caller, after).await
}

/// Projection boundary for agent/TUI/daemon surfaces.
pub trait ProcessJobProjection: Send + Sync {
    type Output;

    fn project_summary(&self, summary: &ProcessJobSummary) -> Self::Output;
    fn project_receipt(&self, receipt: &ProcessJobReceipt) -> Self::Output;
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use super::*;

    #[derive(Default)]
    struct FakeBackend {
        calls: Mutex<Vec<&'static str>>,
    }

    #[async_trait]
    impl ProcessJobBackend for FakeBackend {
        fn kind(&self) -> ProcessJobBackendKind {
            ProcessJobBackendKind::Native
        }

        fn capabilities(&self) -> ProcessJobBackendCapabilities {
            ProcessJobBackendCapabilities {
                backend: Some(ProcessJobBackendKind::Native),
                supports_shell: true,
                supports_direct_exec: true,
                supports_stdin: true,
                supports_restart: false,
                supports_kill: true,
                supports_adopt: false,
                supports_resource_limits: false,
                supports_log_range: true,
                durable_across_daemon_restart: false,
                unavailable_reason: None,
            }
        }

        async fn start(&self, _request: StartProcessJobRequest) -> Result<ProcessJobBackendStart, RuntimeError> {
            self.calls.lock().expect("fake backend calls lock poisoned").push("start");
            Ok(ProcessJobBackendStart {
                backend_ref: BackendRef("pid:123".to_string()),
                status: ProcessJobStatus::Running,
                log_refs: vec![ProcessJobLogRef {
                    stream: ProcessJobStream::Combined,
                    reference: "native:proc_1/combined.log".to_string(),
                    retained_until: None,
                    max_bytes: Some(1024),
                }],
            })
        }

        async fn observe(&self, backend_ref: BackendRef) -> Result<ProcessJobBackendStatus, RuntimeError> {
            self.calls.lock().expect("fake backend calls lock poisoned").push("observe");
            Ok(ProcessJobBackendStatus {
                backend_ref,
                status: ProcessJobStatus::Running,
                updated_at: Utc::now(),
                log_refs: Vec::new(),
            })
        }

        async fn log(
            &self,
            _backend_ref: BackendRef,
            range: ProcessJobLogRange,
        ) -> Result<ProcessJobLogChunk, RuntimeError> {
            self.calls.lock().expect("fake backend calls lock poisoned").push("log");
            Ok(ProcessJobLogChunk {
                id: ProcessJobId("proc_1".to_string()),
                backend: ProcessJobBackendKind::Native,
                stream: range.stream,
                cursor: ProcessJobLogCursor {
                    stream: range.stream,
                    offset: range.offset.unwrap_or(0),
                },
                next_cursor: Some(ProcessJobLogCursor {
                    stream: range.stream,
                    offset: range.limit_bytes,
                }),
                text: "bounded fake log".to_string(),
                truncated: false,
            })
        }

        async fn kill(&self, _backend_ref: BackendRef) -> Result<ProcessJobReceipt, RuntimeError> {
            self.calls.lock().expect("fake backend calls lock poisoned").push("kill");
            Ok(ProcessJobReceipt {
                operation: ProcessJobOperation::Kill,
                id: Some(ProcessJobId("proc_1".to_string())),
                backend: Some(ProcessJobBackendKind::Native),
                status: Some(ProcessJobStatus::Killed),
                backend_ref: Some(BackendRef("pid:123".to_string())),
                log_refs: Vec::new(),
                summary: "killed".to_string(),
                error: None,
            })
        }

        async fn restart(&self, backend_ref: BackendRef) -> Result<ProcessJobReceipt, RuntimeError> {
            self.calls.lock().expect("fake backend calls lock poisoned").push("restart");
            Ok(ProcessJobReceipt::unsupported(
                ProcessJobOperation::Restart,
                None,
                ProcessJobBackendKind::Native,
                "restart",
                format!("restart unsupported for {backend_ref:?}"),
            ))
        }

        async fn write_stdin(
            &self,
            _backend_ref: BackendRef,
            _data: Vec<u8>,
            _newline: bool,
        ) -> Result<ProcessJobReceipt, RuntimeError> {
            self.calls.lock().expect("fake backend calls lock poisoned").push("write_stdin");
            Ok(ProcessJobReceipt {
                operation: ProcessJobOperation::WriteStdin,
                id: Some(ProcessJobId("proc_1".to_string())),
                backend: Some(ProcessJobBackendKind::Native),
                status: Some(ProcessJobStatus::Running),
                backend_ref: Some(BackendRef("pid:123".to_string())),
                log_refs: Vec::new(),
                summary: "wrote stdin".to_string(),
                error: None,
            })
        }

        async fn close_stdin(&self, _backend_ref: BackendRef) -> Result<ProcessJobReceipt, RuntimeError> {
            self.calls.lock().expect("fake backend calls lock poisoned").push("close_stdin");
            Ok(ProcessJobReceipt {
                operation: ProcessJobOperation::CloseStdin,
                id: Some(ProcessJobId("proc_1".to_string())),
                backend: Some(ProcessJobBackendKind::Native),
                status: Some(ProcessJobStatus::Running),
                backend_ref: Some(BackendRef("pid:123".to_string())),
                log_refs: Vec::new(),
                summary: "closed stdin".to_string(),
                error: None,
            })
        }
    }

    #[derive(Default)]
    struct FakeStore {
        summaries: Mutex<Vec<ProcessJobSummary>>,
        notifications: Mutex<Vec<ProcessJobNotificationEvent>>,
    }

    #[async_trait]
    impl ProcessJobStore for FakeStore {
        async fn upsert(&self, summary: ProcessJobSummary) -> Result<(), RuntimeError> {
            self.summaries.lock().expect("fake store lock poisoned").push(summary);
            Ok(())
        }

        async fn get(&self, id: ProcessJobId) -> Result<Option<ProcessJobSummary>, RuntimeError> {
            Ok(self
                .summaries
                .lock()
                .expect("fake store lock poisoned")
                .iter()
                .find(|summary| summary.id == id)
                .cloned())
        }

        async fn list(&self, _filter: ProcessJobFilter) -> Result<Vec<ProcessJobSummary>, RuntimeError> {
            Ok(self.summaries.lock().expect("fake store lock poisoned").clone())
        }

        async fn record_notification(&self, event: ProcessJobNotificationEvent) -> Result<(), RuntimeError> {
            self.notifications.lock().expect("fake notification lock poisoned").push(event);
            Ok(())
        }

        async fn list_notifications(
            &self,
            caller: ProcessJobCallerScope,
            after: Option<ProcessJobEventId>,
        ) -> Result<Vec<ProcessJobNotificationEvent>, RuntimeError> {
            let notifications = self.notifications.lock().expect("fake notification lock poisoned");
            let mut past_cursor = after.is_none();
            Ok(notifications
                .iter()
                .filter_map(|event| {
                    if !past_cursor {
                        past_cursor = after.as_ref() == Some(&event.event_id);
                        return None;
                    }
                    caller.can_access(&event.owner, ProcessJobOperation::Poll, event.backend).then_some(event.clone())
                })
                .collect())
        }
    }

    #[derive(Default)]
    struct FakeLogStore {
        appends: Mutex<Vec<(ProcessJobId, ProcessJobStream, usize)>>,
    }

    #[async_trait]
    impl ProcessJobLogStore for FakeLogStore {
        async fn append(
            &self,
            id: ProcessJobId,
            stream: ProcessJobStream,
            chunk: &[u8],
        ) -> Result<ProcessJobLogCursor, RuntimeError> {
            self.appends.lock().expect("fake log store lock poisoned").push((id, stream, chunk.len()));
            Ok(ProcessJobLogCursor {
                stream,
                offset: u64::try_from(chunk.len()).expect("chunk len fits u64"),
            })
        }

        async fn read(&self, id: ProcessJobId, range: ProcessJobLogRange) -> Result<ProcessJobLogChunk, RuntimeError> {
            Ok(ProcessJobLogChunk {
                id,
                backend: ProcessJobBackendKind::Native,
                stream: range.stream,
                cursor: ProcessJobLogCursor {
                    stream: range.stream,
                    offset: range.offset.unwrap_or(0),
                },
                next_cursor: None,
                text: "fake".to_string(),
                truncated: false,
            })
        }

        async fn references(&self, id: ProcessJobId) -> Result<Vec<ProcessJobLogRef>, RuntimeError> {
            Ok(vec![NativeProcessJobLogLayout::for_stream(id, ProcessJobStream::Combined).into_log_ref(1024)])
        }
    }

    impl NativeProcessJobLogLayout {
        fn into_log_ref(self, max_bytes: u64) -> ProcessJobLogRef {
            ProcessJobLogRef {
                stream: self.stream,
                reference: self.reference,
                retained_until: None,
                max_bytes: Some(max_bytes),
            }
        }
    }

    #[derive(Default)]
    struct FakeSink {
        delivered: Mutex<Vec<ProcessJobEventId>>,
    }

    #[async_trait]
    impl ProcessJobNotificationSink for FakeSink {
        async fn deliver(&self, event: ProcessJobNotificationEvent) -> Result<(), RuntimeError> {
            self.delivered.lock().expect("fake sink lock poisoned").push(event.event_id);
            Ok(())
        }
    }

    struct TextProjection;

    impl ProcessJobProjection for TextProjection {
        type Output = String;

        fn project_summary(&self, summary: &ProcessJobSummary) -> Self::Output {
            format!("{}:{:?}", summary.id.0, summary.status)
        }

        fn project_receipt(&self, receipt: &ProcessJobReceipt) -> Self::Output {
            format!("{:?}:{}", receipt.operation, receipt.summary)
        }
    }

    #[tokio::test]
    async fn fake_boundaries_compose_without_concrete_coupling() {
        let backend: &dyn ProcessJobBackend = &FakeBackend::default();
        let store: &dyn ProcessJobStore = &FakeStore::default();
        let logs: &dyn ProcessJobLogStore = &FakeLogStore::default();
        let sink: &dyn ProcessJobNotificationSink = &FakeSink::default();
        let projection = TextProjection;

        let request = StartProcessJobRequest {
            backend: ProcessJobBackendKind::Native,
            command_preview: "sleep 1".to_string(),
            program: Some("sleep".to_string()),
            args: vec!["1".to_string()],
            shell_command: None,
            cwd: ProcessJobCwd::Inherited,
            owner: ProcessJobOwnerScope::Session("sess".to_string()),
            resource_policy: ProcessJobResourcePolicy::default(),
            notification_policy: ProcessJobNotificationPolicy::default(),
            metadata: BTreeMap::new(),
        };
        let backend_start = backend.start(request).await.expect("fake backend starts");
        let id = ProcessJobId("proc_1".to_string());
        let summary = ProcessJobSummary {
            id: id.clone(),
            backend: backend.kind(),
            backend_ref: Some(backend_start.backend_ref.clone()),
            owner: ProcessJobOwnerScope::Session("sess".to_string()),
            status: backend_start.status.clone(),
            command_preview: "sleep 1".to_string(),
            cwd: ProcessJobCwd::Inherited,
            started_at: Some(Utc::now()),
            updated_at: Utc::now(),
            completed_at: None,
            log_refs: backend_start.log_refs,
        };
        store.upsert(summary.clone()).await.expect("store accepts summary");
        let cursor = logs.append(id.clone(), ProcessJobStream::Combined, b"hello").await.expect("log append works");
        let event = ProcessJobNotificationEvent {
            event_id: ProcessJobEventId("evt_1".to_string()),
            id: id.clone(),
            backend: ProcessJobBackendKind::Native,
            owner: ProcessJobOwnerScope::Session("sess".to_string()),
            kind: ProcessJobNotificationKind::WatchPattern {
                pattern_index: 0,
                pattern: "hello".to_string(),
            },
            status: ProcessJobStatus::Running,
            created_at: Utc::now(),
            summary: "ready".to_string(),
            log_excerpt: Some("hello".to_string()),
            log_refs: logs.references(id.clone()).await.expect("refs work"),
        };
        store.record_notification(event.clone()).await.expect("notification persists");
        sink.deliver(event).await.expect("notification delivers");
        let receipt = backend.kill(BackendRef("pid:123".to_string())).await.expect("kill returns receipt");

        assert_eq!(cursor.offset, 5);
        assert_eq!(store.get(id).await.expect("store get works"), Some(summary.clone()));
        assert!(projection.project_summary(&summary).contains("proc_1"));
        assert_eq!(projection.project_receipt(&receipt), "Kill:killed");
    }

    #[derive(Default)]
    struct FailingSink;

    #[async_trait]
    impl ProcessJobNotificationSink for FailingSink {
        async fn deliver(&self, _event: ProcessJobNotificationEvent) -> Result<(), RuntimeError> {
            Err(RuntimeError::InvalidTool("delivery unavailable".to_string()))
        }
    }

    fn notification_event(event_id: &str, owner: ProcessJobOwnerScope) -> ProcessJobNotificationEvent {
        ProcessJobNotificationEvent {
            event_id: ProcessJobEventId(event_id.to_string()),
            id: ProcessJobId("proc_notify".to_string()),
            backend: ProcessJobBackendKind::Native,
            owner,
            kind: ProcessJobNotificationKind::Completion,
            status: ProcessJobStatus::Succeeded { exit_code: Some(0) },
            created_at: Utc::now(),
            summary: "process completed".to_string(),
            log_excerpt: Some("done".to_string()),
            log_refs: vec![ProcessJobLogRef {
                stream: ProcessJobStream::Combined,
                reference: "native:proc_notify/combined.log".to_string(),
                retained_until: None,
                max_bytes: Some(1024),
            }],
        }
    }

    #[tokio::test]
    async fn default_notification_policy_delivers_completion_once() {
        let engine = DefaultProcessJobNotificationPolicyEngine;
        let policy = ProcessJobNotificationPolicy {
            notify_on_complete: true,
            watch_patterns: Vec::new(),
        };
        let mut state = ProcessJobNotificationPolicyState::default();
        let observation = ProcessJobNotificationObservation {
            status: ProcessJobStatus::Succeeded { exit_code: Some(0) },
            line: Some("done".to_string()),
            tick: 1,
        };

        let first = engine.evaluate(&policy, &mut state, observation.clone()).await;
        let second = engine.evaluate(&policy, &mut state, observation).await;

        assert_eq!(first.len(), 1);
        assert_eq!(first[0].kind, ProcessJobNotificationKind::Completion);
        assert!(second.is_empty(), "completion notification is one-shot");
    }

    #[tokio::test]
    async fn watch_patterns_are_bounded_rate_limited_and_suppress_noisy_matches() {
        let engine = DefaultProcessJobNotificationPolicyEngine;
        let policy = ProcessJobNotificationPolicy {
            notify_on_complete: true,
            watch_patterns: (0..(MAX_PROCESS_JOB_WATCH_PATTERNS + 3)).map(|index| format!("ready-{index}")).collect(),
        };
        let mut state = ProcessJobNotificationPolicyState::default();
        let first = engine
            .evaluate(&policy, &mut state, ProcessJobNotificationObservation {
                status: ProcessJobStatus::Running,
                line: Some("ready-0".to_string()),
                tick: 1,
            })
            .await;
        let noisy_1 = engine
            .evaluate(&policy, &mut state, ProcessJobNotificationObservation {
                status: ProcessJobStatus::Running,
                line: Some("ready-0".to_string()),
                tick: 2,
            })
            .await;
        let noisy_2 = engine
            .evaluate(&policy, &mut state, ProcessJobNotificationObservation {
                status: ProcessJobStatus::Running,
                line: Some("ready-0".to_string()),
                tick: 3,
            })
            .await;
        let noisy_3 = engine
            .evaluate(&policy, &mut state, ProcessJobNotificationObservation {
                status: ProcessJobStatus::Running,
                line: Some("ready-0".to_string()),
                tick: 4,
            })
            .await;
        let after_window = engine
            .evaluate(&policy, &mut state, ProcessJobNotificationObservation {
                status: ProcessJobStatus::Running,
                line: Some("ready-0".to_string()),
                tick: PROCESS_JOB_WATCH_RATE_LIMIT_TICKS + 2,
            })
            .await;
        let out_of_bounds = engine
            .evaluate(&policy, &mut state, ProcessJobNotificationObservation {
                status: ProcessJobStatus::Running,
                line: Some(format!("ready-{}", MAX_PROCESS_JOB_WATCH_PATTERNS + 1)),
                tick: 100,
            })
            .await;
        let completion = engine
            .evaluate(&policy, &mut state, ProcessJobNotificationObservation {
                status: ProcessJobStatus::Succeeded { exit_code: Some(0) },
                line: Some("done".to_string()),
                tick: 101,
            })
            .await;

        assert_eq!(first.len(), 1);
        assert!(noisy_1.is_empty());
        assert!(noisy_2.is_empty());
        assert!(noisy_3.is_empty());
        assert!(after_window.is_empty(), "pattern is disabled after noisy suppression");
        assert!(out_of_bounds.is_empty(), "patterns beyond the bounded limit are ignored");
        assert_eq!(completion.len(), 1, "completion delivery does not depend on watch patterns");
        assert_eq!(completion[0].kind, ProcessJobNotificationKind::Completion);
    }

    #[tokio::test]
    async fn notification_events_persist_before_delivery_and_replay_on_authorized_reattach() {
        let store = FakeStore::default();
        let sink = FakeSink::default();
        let owner = ProcessJobOwnerScope::Session("sess".to_string());
        let event = notification_event("evt_notify_1", owner.clone());

        persist_and_deliver_notification(&store, &sink, event.clone())
            .await
            .expect("persist and deliver succeeds");
        assert_eq!(sink.delivered.lock().expect("sink lock").as_slice(), &[event.event_id.clone()]);

        let authorized = ProcessJobCallerScope {
            session_id: Some("sess".to_string()),
            capabilities: ProcessJobCapabilitySet::observe_only(),
            ..ProcessJobCallerScope::default()
        };
        let replayed =
            replay_authorized_notifications(&store, authorized, None).await.expect("authorized replay succeeds");
        assert_eq!(replayed, vec![event.clone()]);

        let unauthorized = ProcessJobCallerScope {
            session_id: Some("other".to_string()),
            capabilities: ProcessJobCapabilitySet::observe_only(),
            ..ProcessJobCallerScope::default()
        };
        assert!(
            replay_authorized_notifications(&store, unauthorized, None)
                .await
                .expect("unauthorized replay is empty")
                .is_empty()
        );

        let after = replay_authorized_notifications(
            &store,
            ProcessJobCallerScope {
                session_id: Some("sess".to_string()),
                capabilities: ProcessJobCapabilitySet::observe_only(),
                ..ProcessJobCallerScope::default()
            },
            Some(ProcessJobEventId("evt_notify_1".to_string())),
        )
        .await
        .expect("cursor replay succeeds");
        assert!(after.is_empty());
    }

    #[tokio::test]
    async fn notification_persistence_survives_transient_delivery_failure() {
        let store = FakeStore::default();
        let event = notification_event("evt_notify_fail", ProcessJobOwnerScope::DaemonGlobal);
        let error = persist_and_deliver_notification(&store, &FailingSink, event.clone())
            .await
            .expect_err("delivery failure is returned");
        assert!(error.to_string().contains("delivery unavailable"));

        let replayed = replay_authorized_notifications(
            &store,
            ProcessJobCallerScope {
                daemon_global: true,
                capabilities: ProcessJobCapabilitySet::observe_only(),
                ..ProcessJobCallerScope::default()
            },
            None,
        )
        .await
        .expect("persisted event replays after failure");
        assert_eq!(replayed, vec![event]);
    }

    #[tokio::test]
    async fn fake_backend_contract_covers_projection_and_mutations() {
        let fake = FakeBackend::default();
        let backend: &dyn ProcessJobBackend = &fake;
        let store: &dyn ProcessJobStore = &FakeStore::default();
        let projection = TextProjection;
        let id = ProcessJobId("proc_contract".to_string());
        let owner = ProcessJobOwnerScope::Session("sess".to_string());

        let start = backend
            .start(StartProcessJobRequest {
                backend: ProcessJobBackendKind::Native,
                command_preview: "sleep 1".to_string(),
                program: Some("sleep".to_string()),
                args: vec!["1".to_string()],
                shell_command: None,
                cwd: ProcessJobCwd::Inherited,
                owner: owner.clone(),
                resource_policy: ProcessJobResourcePolicy::default(),
                notification_policy: ProcessJobNotificationPolicy::default(),
                metadata: BTreeMap::new(),
            })
            .await
            .expect("fake start projects backend state");
        let observed = backend.observe(start.backend_ref.clone()).await.expect("fake observe projects status");
        let summary = ProcessJobSummary {
            id: id.clone(),
            backend: backend.kind(),
            backend_ref: Some(observed.backend_ref.clone()),
            owner,
            status: observed.status.clone(),
            command_preview: "sleep 1".to_string(),
            cwd: ProcessJobCwd::Inherited,
            started_at: Some(observed.updated_at),
            updated_at: observed.updated_at,
            completed_at: None,
            log_refs: observed.log_refs,
        };
        store.upsert(summary.clone()).await.expect("summary upserts");

        let listed = store.list(ProcessJobFilter::default()).await.expect("list projects rows");
        let log = backend
            .log(start.backend_ref.clone(), ProcessJobLogRange {
                stream: ProcessJobStream::Combined,
                offset: Some(7),
                limit_bytes: 32,
            })
            .await
            .expect("fake log projects chunk");
        let kill = backend.kill(start.backend_ref.clone()).await.expect("fake kill projects receipt");
        let restart = backend.restart(start.backend_ref).await.expect("fake restart returns typed unsupported receipt");

        assert_eq!(listed, vec![summary.clone()]);
        assert!(projection.project_summary(&summary).contains("Running"));
        assert_eq!(log.cursor.offset, 7);
        assert_eq!(kill.status, Some(ProcessJobStatus::Killed));
        assert_eq!(
            restart.error.expect("restart is unsupported").code,
            ProcessJobErrorCode::UnsupportedActionForBackend
        );
        assert_eq!(fake.calls.lock().expect("fake calls lock poisoned").as_slice(), [
            "start", "observe", "log", "kill", "restart"
        ]);
    }

    #[test]
    fn fake_backend_capability_matrix_and_unavailable_receipts_are_explicit() {
        let capabilities = FakeBackend::default().capabilities();
        assert_eq!(capabilities.backend, Some(ProcessJobBackendKind::Native));
        assert!(capabilities.supports_shell);
        assert!(capabilities.supports_direct_exec);
        assert!(capabilities.supports_stdin);
        assert!(capabilities.supports_kill);
        assert!(!capabilities.supports_restart);
        assert!(!capabilities.supports_adopt);

        let unavailable = ProcessJobReceipt {
            operation: ProcessJobOperation::Start,
            id: None,
            backend: Some(ProcessJobBackendKind::Systemd),
            status: None,
            backend_ref: None,
            log_refs: Vec::new(),
            summary: "systemd not enabled".to_string(),
            error: Some(ProcessJobError {
                code: ProcessJobErrorCode::BackendUnavailable,
                operation: ProcessJobOperation::Start,
                id: None,
                backend: Some(ProcessJobBackendKind::Systemd),
                action: Some("start".to_string()),
                message: "systemd not enabled".to_string(),
            }),
        };
        let unsupported = ProcessJobReceipt::unsupported(
            ProcessJobOperation::Restart,
            Some(ProcessJobId("proc_1".to_string())),
            ProcessJobBackendKind::Pueue,
            "restart",
            "restart unsupported",
        );

        assert_eq!(unavailable.error.expect("backend unavailable").code, ProcessJobErrorCode::BackendUnavailable);
        assert_eq!(
            unsupported.error.expect("restart unsupported").code,
            ProcessJobErrorCode::UnsupportedActionForBackend
        );
    }

    #[test]
    fn status_terminal_classification_is_explicit() {
        assert!(!ProcessJobStatus::Running.is_terminal());
        assert!(ProcessJobStatus::Succeeded { exit_code: Some(0) }.is_terminal());
        assert!(ProcessJobStatus::LostAfterRestart.is_terminal());
    }

    #[test]
    fn native_log_layout_is_append_only_bounded_and_safe() {
        let policy = ProcessJobLogRetentionPolicy {
            max_bytes_per_job: 1024,
            max_age: Some(Duration::from_secs(60)),
            keep_terminal_logs: true,
        };
        let now = DateTime::parse_from_rfc3339("2026-05-17T05:52:12Z").expect("timestamp parses").with_timezone(&Utc);
        let log_ref =
            policy.reference_for(ProcessJobId("../job with spaces".to_string()), ProcessJobStream::Combined, now);

        assert_eq!(log_ref.reference, "native:.._job_with_spaces/combined.log");
        assert_eq!(log_ref.max_bytes, Some(1024));
        assert_eq!(log_ref.retained_until, Some(now + chrono::Duration::seconds(60)));
    }

    #[test]
    fn log_chunks_carry_cursor_and_truncation_explicitly() {
        let chunk = ProcessJobLogChunk {
            id: ProcessJobId("proc_1".to_string()),
            backend: ProcessJobBackendKind::Native,
            stream: ProcessJobStream::Stdout,
            cursor: ProcessJobLogCursor {
                stream: ProcessJobStream::Stdout,
                offset: 4096,
            },
            next_cursor: Some(ProcessJobLogCursor {
                stream: ProcessJobStream::Stdout,
                offset: 8192,
            }),
            text: "bounded".to_string(),
            truncated: true,
        };
        let json = serde_json::to_value(chunk).expect("chunk serializes");
        assert_eq!(json["cursor"]["offset"], 4096);
        assert_eq!(json["next_cursor"]["offset"], 8192);
        assert_eq!(json["truncated"], true);
    }

    #[test]
    fn unavailable_backend_capabilities_are_fail_closed() {
        let capabilities = ProcessJobBackendCapabilities::unavailable(ProcessJobBackendKind::Systemd, "not systemd");
        assert_eq!(capabilities.backend, Some(ProcessJobBackendKind::Systemd));
        assert_eq!(capabilities.unavailable_reason.as_deref(), Some("not systemd"));
        assert!(!capabilities.supports_kill);
        assert!(!capabilities.durable_across_daemon_restart);
    }

    #[test]
    fn observe_only_scope_denies_cross_session_mutation() {
        let owner = ProcessJobOwnerScope::Session("sess-a".to_string());
        let observer = ProcessJobCallerScope {
            session_id: Some("sess-a".to_string()),
            capabilities: ProcessJobCapabilitySet::observe_only(),
            ..ProcessJobCallerScope::default()
        };
        let other_session = ProcessJobCallerScope {
            session_id: Some("sess-b".to_string()),
            capabilities: ProcessJobCapabilitySet::full_control(),
            ..ProcessJobCallerScope::default()
        };

        assert!(observer.can_access(&owner, ProcessJobOperation::List, ProcessJobBackendKind::Native));
        assert!(observer.can_access(&owner, ProcessJobOperation::Log, ProcessJobBackendKind::Native));
        assert!(!observer.can_access(&owner, ProcessJobOperation::Kill, ProcessJobBackendKind::Native));
        assert!(!observer.can_access(&owner, ProcessJobOperation::WriteStdin, ProcessJobBackendKind::Native));
        assert!(!other_session.can_access(&owner, ProcessJobOperation::Kill, ProcessJobBackendKind::Native));
    }

    #[test]
    fn durable_backend_and_stdin_require_explicit_capabilities() {
        let local_start_only = ProcessJobCapabilitySet {
            observe: true,
            start: true,
            mutate: true,
            ..ProcessJobCapabilitySet::default()
        };

        assert!(local_start_only.allows_operation(ProcessJobOperation::Start, ProcessJobBackendKind::Native));
        assert!(!local_start_only.allows_operation(ProcessJobOperation::Start, ProcessJobBackendKind::Pueue));
        assert!(!local_start_only.allows_operation(ProcessJobOperation::WriteStdin, ProcessJobBackendKind::Native));

        let full = ProcessJobCapabilitySet::full_control();
        assert!(full.allows_operation(ProcessJobOperation::Start, ProcessJobBackendKind::Systemd));
        assert!(full.allows_operation(ProcessJobOperation::WriteStdin, ProcessJobBackendKind::Native));
    }

    #[test]
    fn unsupported_action_receipt_is_machine_readable() {
        let receipt = ProcessJobReceipt::unsupported(
            ProcessJobOperation::WriteStdin,
            Some(ProcessJobId("proc_1".to_string())),
            ProcessJobBackendKind::Pueue,
            "write_stdin",
            "stdin is not supported by pueue backend",
        );

        let json = serde_json::to_value(&receipt).expect("receipt serializes");
        assert_eq!(json["operation"], "write_stdin");
        assert_eq!(json["backend"], "pueue");
        assert_eq!(json["error"]["code"], "unsupported_action_for_backend");
    }

    #[test]
    fn tool_result_variants_cover_public_operations() {
        let receipt = ProcessJobReceipt {
            operation: ProcessJobOperation::Start,
            id: Some(ProcessJobId("proc_1".to_string())),
            backend: Some(ProcessJobBackendKind::Native),
            status: Some(ProcessJobStatus::Running),
            backend_ref: Some(BackendRef("pid:123".to_string())),
            log_refs: Vec::new(),
            summary: "started".to_string(),
            error: None,
        };
        let log_chunk = ProcessJobLogChunk {
            id: ProcessJobId("proc_1".to_string()),
            backend: ProcessJobBackendKind::Native,
            stream: ProcessJobStream::Combined,
            cursor: ProcessJobLogCursor {
                stream: ProcessJobStream::Combined,
                offset: 0,
            },
            next_cursor: None,
            text: "ok".to_string(),
            truncated: false,
        };
        let variants = vec![
            ProcessJobToolResult::Start(receipt.clone()),
            ProcessJobToolResult::List(Vec::new()),
            ProcessJobToolResult::Poll(receipt.clone()),
            ProcessJobToolResult::Log(log_chunk),
            ProcessJobToolResult::Wait(receipt.clone()),
            ProcessJobToolResult::Kill(receipt.clone()),
            ProcessJobToolResult::Restart(receipt.clone()),
            ProcessJobToolResult::WriteStdin(receipt.clone()),
            ProcessJobToolResult::CloseStdin(receipt.clone()),
            ProcessJobToolResult::Adopt(receipt.clone()),
            ProcessJobToolResult::GarbageCollect(receipt),
        ];

        let operation_names: Vec<_> = variants
            .into_iter()
            .map(|variant| serde_json::to_value(variant).expect("variant serializes")["operation"].clone())
            .collect();
        assert_eq!(operation_names, vec![
            "start",
            "list",
            "poll",
            "log",
            "wait",
            "kill",
            "restart",
            "write_stdin",
            "close_stdin",
            "adopt",
            "garbage_collect"
        ]);
    }
}
