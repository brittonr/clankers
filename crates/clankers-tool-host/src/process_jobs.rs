//! Backend-neutral process/job contracts shared by tool adapters.
//!
//! This module owns pure process-job request/decision DTOs that do not depend
//! on the Clankers runtime facade, daemon actors, TUI state, procmon handles,
//! filesystem storage, or backend command execution.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::time::Duration;

use chrono::DateTime;
use chrono::Utc;
use serde::Deserialize;
use serde::Serialize;

pub const PROCESS_JOB_PROFILE_METADATA_NAME: &str = "profile";
pub const PROCESS_JOB_PROFILE_METADATA_SCHEMA_VERSION: &str = "identity.profile.schema_version";
pub const PROCESS_JOB_PROFILE_METADATA_SOURCE: &str = "identity.profile.source";
pub const PROCESS_JOB_PROFILE_METADATA_POLICY: &str = "identity.profile.policy";

pub const MAX_PROCESS_JOB_WATCH_PATTERNS: usize = 8;
pub const MAX_PROCESS_JOB_WATCH_PATTERN_LEN: usize = 128;
pub const PROCESS_JOB_WATCH_RATE_LIMIT_TICKS: u64 = 15;
pub const PROCESS_JOB_WATCH_SUPPRESSION_LIMIT: u32 = 3;

pub const PROCESS_JOB_REDACTED: &str = "[REDACTED]";
pub const PROCESS_JOB_MAX_SAFE_PREVIEW_CHARS: usize = 160;
pub const PROCESS_JOB_MAX_SAFE_EXCERPT_CHARS: usize = 512;
pub const PROCESS_JOB_MAX_SAFE_METADATA_VALUE_CHARS: usize = 128;

pub const PROCESS_JOB_ID_PREFIX: &str = "proc_b3_";
pub const PROCESS_JOB_IDENTITY_DOMAIN: &str = "clankers.process-job.identity";
pub const PROCESS_JOB_IDENTITY_VERSION: u8 = 1;

const DEFAULT_PROCESS_JOB_RETENTION_SECS: u64 = 1_209_600;
const DEFAULT_PROCESS_JOB_LOG_LINE_BYTES: u64 = 65_536;
const DEFAULT_PROCESS_JOB_LOG_CHUNK_BYTES: u64 = 1_048_576;
const DEFAULT_PROCESS_JOB_LOG_FILE_BYTES: u64 = 67_108_864;
const DEFAULT_PROCESS_JOB_LOG_TOTAL_BYTES: u64 = 1_073_741_824;

/// Unix timestamp used by backend-neutral process/job contracts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ProcessJobTimestamp(pub i64);

impl ProcessJobTimestamp {
    #[must_use]
    pub fn from_unix_seconds(seconds: i64) -> Self {
        Self(seconds)
    }

    #[must_use]
    pub fn unix_seconds(self) -> i64 {
        self.0
    }

    #[must_use]
    pub fn saturating_add_seconds(self, seconds: i64) -> Self {
        Self(self.0.saturating_add(seconds))
    }
}

#[must_use]
pub fn process_job_timestamp(timestamp: DateTime<Utc>) -> ProcessJobTimestamp {
    ProcessJobTimestamp::from_unix_seconds(timestamp.timestamp())
}

fn add_timestamp_duration(timestamp: ProcessJobTimestamp, duration: Duration) -> Option<ProcessJobTimestamp> {
    let seconds = i64::try_from(duration.as_secs()).ok()?;
    Some(timestamp.saturating_add_seconds(seconds))
}

fn absent_process_job_profile_metadata() -> Option<ProcessJobProfileReceiptMetadata> {
    None
}

fn absent_capability_detail() -> Option<String> {
    None
}

fn empty_process_job_identity_metadata() -> BTreeMap<String, String> {
    BTreeMap::new()
}

fn empty_process_job_watch_patterns() -> Vec<String> {
    Vec::new()
}

/// Stable Clankers-owned process/job identifier.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ProcessJobId(pub String);

impl ProcessJobId {
    #[must_use]
    pub fn from_identity_envelope(envelope: &ProcessJobIdentityEnvelope) -> Self {
        envelope.derive_id()
    }

    #[must_use]
    pub fn is_blake3_native(&self) -> bool {
        self.0
            .strip_prefix(PROCESS_JOB_ID_PREFIX)
            .is_some_and(|digest| digest.len() == 64 && digest.chars().all(|ch| ch.is_ascii_hexdigit()))
    }

    #[must_use]
    pub fn legacy(raw: impl Into<String>) -> Self {
        Self(raw.into())
    }
}

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

impl ProcessJobBackendKind {
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Native => "native",
            Self::Pueue => "pueue",
            Self::Systemd => "systemd",
            Self::Unknown => "unknown",
        }
    }
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

impl ProcessJobOperation {
    #[must_use]
    pub const fn action_name(self) -> &'static str {
        match self {
            Self::Start => "start",
            Self::List => "list",
            Self::Poll => "poll",
            Self::Log => "log",
            Self::Wait => "wait",
            Self::Kill => "kill",
            Self::Restart => "restart",
            Self::WriteStdin => "write_stdin",
            Self::CloseStdin => "close_stdin",
            Self::Adopt => "adopt",
            Self::GarbageCollect => "garbage_collect",
        }
    }
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

    #[must_use]
    pub fn label(&self) -> String {
        match self {
            Self::Pending => "pending".to_string(),
            Self::Running => "running".to_string(),
            Self::Waiting => "waiting".to_string(),
            Self::Succeeded { exit_code } => {
                format!("succeeded({})", exit_code.map(|code| code.to_string()).unwrap_or_else(|| "ok".to_string()))
            }
            Self::Failed { exit_code, reason } => format!(
                "failed({}:{reason})",
                exit_code.map(|code| code.to_string()).unwrap_or_else(|| "unknown".to_string())
            ),
            Self::Killed => "killed".to_string(),
            Self::Cancelled => "cancelled".to_string(),
            Self::LostAfterRestart => "lost-after-restart".to_string(),
            Self::ReattachedLogIncomplete => "reattached-log-incomplete".to_string(),
            Self::BackendUnavailable { reason } => format!("backend-unavailable({reason})"),
            Self::Unknown { raw } => format!("unknown({raw})"),
        }
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
    pub read_raw_logs: bool,
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
            ..Self::default()
        }
    }

    #[must_use]
    pub fn bounded_log_reader() -> Self {
        Self {
            observe: true,
            read_logs: true,
            ..Self::default()
        }
    }

    #[must_use]
    pub fn raw_log_reader() -> Self {
        Self {
            observe: true,
            read_logs: true,
            read_raw_logs: true,
            ..Self::default()
        }
    }

    #[must_use]
    pub fn full_control() -> Self {
        Self {
            observe: true,
            read_logs: true,
            read_raw_logs: true,
            start: true,
            mutate: true,
            stdin: true,
            select_backend: true,
        }
    }

    #[must_use]
    pub fn allows_log_access(&self, raw: bool) -> bool {
        self.observe && self.read_logs && (!raw || self.read_raw_logs)
    }

    #[must_use]
    pub fn allows_operation(&self, operation: ProcessJobOperation, backend: ProcessJobBackendKind) -> bool {
        match operation {
            ProcessJobOperation::List | ProcessJobOperation::Poll => self.observe,
            ProcessJobOperation::Log => self.allows_log_access(false),
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

/// Backend capability descriptor used before dispatching mutations.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct ProcessJobBackendCapabilities {
    pub backend: Option<ProcessJobBackendKind>,
    pub supports_shell: bool,
    pub supports_direct_exec: bool,
    pub supports_stdin: bool,
    pub supports_restart: bool,
    pub supports_kill: bool,
    pub supports_kill_tree: bool,
    pub supports_control_group: bool,
    pub supports_adopt: bool,
    #[serde(default)]
    pub supports_garbage_collect: bool,
    pub supports_resource_limits: bool,
    pub supports_log_cursor: bool,
    pub supports_log_range: bool,
    pub supports_queueing: bool,
    pub supports_priority: bool,
    pub supports_dependencies: bool,
    pub supports_live_status: bool,
    pub supports_completion_notifications: bool,
    pub supports_readiness_watch: bool,
    pub durable_across_daemon_restart: bool,
    pub unavailable_reason: Option<String>,
}

/// Short alias used by Cairn and service-layer callers for the process/job backend matrix.
pub type BackendCapabilities = ProcessJobBackendCapabilities;

impl ProcessJobBackendCapabilities {
    #[must_use]
    pub fn native() -> Self {
        Self {
            backend: Some(ProcessJobBackendKind::Native),
            supports_shell: true,
            supports_direct_exec: true,
            supports_stdin: true,
            supports_restart: true,
            supports_kill: true,
            supports_kill_tree: true,
            supports_control_group: true,
            supports_adopt: true,
            supports_garbage_collect: true,
            supports_resource_limits: false,
            supports_log_cursor: true,
            supports_log_range: true,
            supports_queueing: false,
            supports_priority: false,
            supports_dependencies: false,
            supports_live_status: true,
            supports_completion_notifications: true,
            supports_readiness_watch: true,
            durable_across_daemon_restart: false,
            unavailable_reason: None,
        }
    }

    #[must_use]
    pub fn pueue() -> Self {
        Self {
            backend: Some(ProcessJobBackendKind::Pueue),
            supports_shell: true,
            supports_direct_exec: true,
            supports_stdin: false,
            supports_restart: true,
            supports_kill: true,
            supports_kill_tree: false,
            supports_control_group: false,
            supports_adopt: true,
            supports_garbage_collect: false,
            supports_resource_limits: false,
            supports_log_cursor: true,
            supports_log_range: true,
            supports_queueing: true,
            supports_priority: true,
            supports_dependencies: true,
            supports_live_status: true,
            supports_completion_notifications: true,
            supports_readiness_watch: false,
            durable_across_daemon_restart: true,
            unavailable_reason: None,
        }
    }

    #[must_use]
    pub fn systemd() -> Self {
        Self {
            backend: Some(ProcessJobBackendKind::Systemd),
            supports_shell: true,
            supports_direct_exec: true,
            supports_stdin: false,
            supports_restart: true,
            supports_kill: true,
            supports_kill_tree: true,
            supports_control_group: true,
            supports_adopt: true,
            supports_garbage_collect: false,
            supports_resource_limits: true,
            supports_log_cursor: true,
            supports_log_range: true,
            supports_queueing: false,
            supports_priority: false,
            supports_dependencies: false,
            supports_live_status: true,
            supports_completion_notifications: true,
            supports_readiness_watch: true,
            durable_across_daemon_restart: true,
            unavailable_reason: None,
        }
    }

    #[must_use]
    pub fn unavailable(backend: ProcessJobBackendKind, reason: impl Into<String>) -> Self {
        Self {
            backend: Some(backend),
            unavailable_reason: Some(reason.into()),
            ..Self::default()
        }
    }

    #[must_use]
    pub fn for_backend(backend: ProcessJobBackendKind) -> Self {
        match backend {
            ProcessJobBackendKind::Native => Self::native(),
            ProcessJobBackendKind::Pueue => Self::pueue(),
            ProcessJobBackendKind::Systemd => Self::systemd(),
            ProcessJobBackendKind::Unknown => Self::default(),
        }
    }

    #[must_use]
    pub fn supports_operation(&self, operation: ProcessJobOperation) -> bool {
        if self.unavailable_reason.is_some() {
            return false;
        }
        match operation {
            ProcessJobOperation::Start => self.supports_shell || self.supports_direct_exec,
            ProcessJobOperation::List | ProcessJobOperation::Poll | ProcessJobOperation::Wait => {
                self.supports_live_status
            }
            ProcessJobOperation::Log => self.supports_log_cursor || self.supports_log_range,
            ProcessJobOperation::Kill => self.supports_kill,
            ProcessJobOperation::Restart => self.supports_restart,
            ProcessJobOperation::WriteStdin | ProcessJobOperation::CloseStdin => self.supports_stdin,
            ProcessJobOperation::Adopt => self.supports_adopt,
            ProcessJobOperation::GarbageCollect => self.supports_garbage_collect,
        }
    }

    #[must_use]
    pub fn unsupported_detail(&self, operation: ProcessJobOperation) -> Option<&'static str> {
        if self.supports_operation(operation) {
            return None;
        }
        Some(match operation {
            ProcessJobOperation::Start => "start requires shell or direct_exec support",
            ProcessJobOperation::List | ProcessJobOperation::Poll | ProcessJobOperation::Wait => {
                "status operations require live_status support"
            }
            ProcessJobOperation::Log => "log requires log_cursor or bounded_log support",
            ProcessJobOperation::Kill => "kill requires kill support",
            ProcessJobOperation::Restart => "restart requires restart support",
            ProcessJobOperation::WriteStdin | ProcessJobOperation::CloseStdin => "stdin requires stdin support",
            ProcessJobOperation::Adopt => "adoption requires adoption support",
            ProcessJobOperation::GarbageCollect => "gc requires garbage collection support",
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProcessJobSafeCapabilityHints {
    pub supports_kill: bool,
    pub supports_restart: bool,
    pub supports_stdin: bool,
    pub supports_logs: bool,
    pub supports_resource_limits: bool,
}

impl ProcessJobSafeCapabilityHints {
    #[must_use]
    pub fn from_capabilities(capabilities: &ProcessJobBackendCapabilities) -> Self {
        Self {
            supports_kill: capabilities.supports_kill,
            supports_restart: capabilities.supports_restart,
            supports_stdin: capabilities.supports_stdin,
            supports_logs: capabilities.supports_log_cursor || capabilities.supports_log_range,
            supports_resource_limits: capabilities.supports_resource_limits,
        }
    }

    #[must_use]
    pub fn for_backend(backend: ProcessJobBackendKind) -> Self {
        Self::from_capabilities(&ProcessJobBackendCapabilities::for_backend(backend))
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
    pub retained_until: Option<ProcessJobTimestamp>,
    pub max_bytes: Option<u64>,
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

    #[must_use]
    pub fn into_log_ref(self, max_bytes: u64) -> ProcessJobLogRef {
        ProcessJobLogRef {
            stream: self.stream,
            reference: self.reference,
            retained_until: None,
            max_bytes: Some(max_bytes),
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

/// A backend-neutral process job specification.
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ListProcessJobsRequest {
    pub filter: ProcessJobFilter,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PollProcessJobRequest {
    pub id: ProcessJobId,
    pub cursor: Option<ProcessJobLogCursor>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReadProcessJobLogRequest {
    pub id: ProcessJobId,
    pub range: ProcessJobLogRange,
    #[serde(default)]
    pub raw: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WaitProcessJobRequest {
    pub id: ProcessJobId,
    pub timeout: Option<Duration>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MutateProcessJobRequest {
    pub id: ProcessJobId,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WriteProcessJobStdinRequest {
    pub id: ProcessJobId,
    pub data: Vec<u8>,
    pub newline: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StartProcessJobProfileRequest {
    pub profile: String,
    pub owner: ProcessJobOwnerScope,
    pub metadata: BTreeMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GarbageCollectProcessJobsRequest {
    pub filter: ProcessJobFilter,
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

/// Backend-neutral public request vocabulary for process/job tool actions.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "action", content = "request")]
pub enum ProcessJobToolRequest {
    Start(StartProcessJobRequest),
    List(ListProcessJobsRequest),
    Poll(PollProcessJobRequest),
    Log(ReadProcessJobLogRequest),
    Wait(WaitProcessJobRequest),
    Kill(MutateProcessJobRequest),
    Restart(MutateProcessJobRequest),
    WriteStdin(WriteProcessJobStdinRequest),
    CloseStdin(MutateProcessJobRequest),
    StartProfile(StartProcessJobProfileRequest),
    Adopt(AdoptProcessJobRequest),
    GarbageCollect(GarbageCollectProcessJobsRequest),
}

impl ProcessJobToolRequest {
    #[must_use]
    pub fn operation(&self) -> ProcessJobOperation {
        match self {
            Self::Start(_) | Self::StartProfile(_) => ProcessJobOperation::Start,
            Self::List(_) => ProcessJobOperation::List,
            Self::Poll(_) => ProcessJobOperation::Poll,
            Self::Log(_) => ProcessJobOperation::Log,
            Self::Wait(_) => ProcessJobOperation::Wait,
            Self::Kill(_) => ProcessJobOperation::Kill,
            Self::Restart(_) => ProcessJobOperation::Restart,
            Self::WriteStdin(_) => ProcessJobOperation::WriteStdin,
            Self::CloseStdin(_) => ProcessJobOperation::CloseStdin,
            Self::Adopt(_) => ProcessJobOperation::Adopt,
            Self::GarbageCollect(_) => ProcessJobOperation::GarbageCollect,
        }
    }
}

/// Alias used by config/profile parsing code that resolves named jobs before dispatch.
pub type ProcessJobSpec = StartProcessJobRequest;

/// Restart/crash reconciliation state for a persisted process/job record.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProcessJobReconciliationState {
    Running,
    Reattached,
    ReattachedLogIncomplete,
    Exited,
    LostAfterRestart,
    BackendUnavailable,
    Orphaned,
    IdentityMismatch,
}

impl ProcessJobReconciliationState {
    #[must_use]
    pub const fn is_adopted(self) -> bool {
        matches!(self, Self::Running | Self::Reattached | Self::ReattachedLogIncomplete)
    }

    #[must_use]
    pub const fn is_fail_closed(self) -> bool {
        matches!(self, Self::BackendUnavailable | Self::Orphaned | Self::IdentityMismatch)
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProcessJobReconciliationReport {
    pub checked: usize,
    pub updated: usize,
    pub unavailable: usize,
    pub skipped_terminal: usize,
}

impl ProcessJobReconciliationReport {
    pub fn record_observation(&mut self, state: ProcessJobReconciliationState) {
        self.checked += 1;
        if matches!(state, ProcessJobReconciliationState::BackendUnavailable) {
            self.unavailable += 1;
        } else {
            self.updated += 1;
        }
    }
}

/// Log continuity after reconciliation. Status and log ownership can degrade independently.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProcessJobLogReconciliationState {
    Complete,
    Incomplete,
    Unavailable { reason: String },
    BackendReferenced,
}

/// Native process identity facts persisted at start time.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NativeProcessJobIdentity {
    pub pid: u32,
    pub process_group: Option<i32>,
    pub start_time_ticks: Option<u64>,
    pub command_fingerprint: Option<String>,
    pub cwd_fingerprint: Option<String>,
}

/// Host-observed native process facts used during conservative restart reconciliation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NativeProcessJobObservation {
    pub pid: u32,
    pub process_group: Option<i32>,
    pub start_time_ticks: Option<u64>,
    pub command_fingerprint: Option<String>,
    pub cwd_fingerprint: Option<String>,
}

impl NativeProcessJobIdentity {
    #[must_use]
    pub fn verify_observation(
        &self,
        observation: Option<&NativeProcessJobObservation>,
    ) -> ProcessJobReconciliationState {
        let Some(observation) = observation else {
            return ProcessJobReconciliationState::LostAfterRestart;
        };
        if self.pid != observation.pid || self.process_group != observation.process_group {
            return ProcessJobReconciliationState::IdentityMismatch;
        }
        let comparable_facts = [
            (
                self.start_time_ticks.map(|value| value.to_string()),
                observation.start_time_ticks.map(|value| value.to_string()),
            ),
            (self.command_fingerprint.clone(), observation.command_fingerprint.clone()),
            (self.cwd_fingerprint.clone(), observation.cwd_fingerprint.clone()),
        ];
        let mut matched_any = false;
        for (expected, actual) in comparable_facts {
            match (expected, actual) {
                (Some(left), Some(right)) if left == right => matched_any = true,
                (Some(_), Some(_)) => return ProcessJobReconciliationState::IdentityMismatch,
                _ => {}
            }
        }
        if matched_any {
            ProcessJobReconciliationState::ReattachedLogIncomplete
        } else {
            ProcessJobReconciliationState::IdentityMismatch
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "state")]
pub enum ExternalProcessJobBackendState {
    Running,
    Succeeded { exit_code: Option<i32> },
    Failed { exit_code: Option<i32>, reason: String },
    Missing,
    BackendUnavailable { reason: String },
    Ambiguous { reason: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExternalProcessJobReconciliationFacts {
    pub id: ProcessJobId,
    pub backend: ProcessJobBackendKind,
    pub expected_backend_ref: BackendRef,
    pub observed_backend_ref: Option<BackendRef>,
    pub state: ExternalProcessJobBackendState,
    pub log_refs: Vec<ProcessJobLogRef>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProcessJobReconciliationOutcome {
    pub id: ProcessJobId,
    pub backend: ProcessJobBackendKind,
    pub backend_ref: Option<BackendRef>,
    pub state: ProcessJobReconciliationState,
    pub log_state: ProcessJobLogReconciliationState,
    pub status: ProcessJobStatus,
    pub log_refs: Vec<ProcessJobLogRef>,
    pub reason: Option<String>,
}

#[must_use]
pub fn reconcile_external_backend_reference(
    facts: ExternalProcessJobReconciliationFacts,
) -> ProcessJobReconciliationOutcome {
    let ref_matches = facts.observed_backend_ref.as_ref() == Some(&facts.expected_backend_ref);
    let (state, log_state, status, reason) = match facts.state {
        ExternalProcessJobBackendState::Running if ref_matches => (
            ProcessJobReconciliationState::Reattached,
            ProcessJobLogReconciliationState::BackendReferenced,
            ProcessJobStatus::Running,
            None,
        ),
        ExternalProcessJobBackendState::Succeeded { exit_code } if ref_matches => (
            ProcessJobReconciliationState::Exited,
            ProcessJobLogReconciliationState::BackendReferenced,
            ProcessJobStatus::Succeeded { exit_code },
            None,
        ),
        ExternalProcessJobBackendState::Failed { exit_code, reason } if ref_matches => (
            ProcessJobReconciliationState::Exited,
            ProcessJobLogReconciliationState::BackendReferenced,
            ProcessJobStatus::Failed {
                exit_code,
                reason: reason.clone(),
            },
            Some(reason),
        ),
        ExternalProcessJobBackendState::Missing => (
            ProcessJobReconciliationState::Orphaned,
            ProcessJobLogReconciliationState::Unavailable {
                reason: "backend reference is missing".to_string(),
            },
            ProcessJobStatus::LostAfterRestart,
            Some("backend reference is missing".to_string()),
        ),
        ExternalProcessJobBackendState::BackendUnavailable { reason } => (
            ProcessJobReconciliationState::BackendUnavailable,
            ProcessJobLogReconciliationState::Unavailable { reason: reason.clone() },
            ProcessJobStatus::BackendUnavailable { reason: reason.clone() },
            Some(reason),
        ),
        ExternalProcessJobBackendState::Ambiguous { reason } => (
            ProcessJobReconciliationState::IdentityMismatch,
            ProcessJobLogReconciliationState::Unavailable { reason: reason.clone() },
            ProcessJobStatus::LostAfterRestart,
            Some(reason),
        ),
        _ => (
            ProcessJobReconciliationState::IdentityMismatch,
            ProcessJobLogReconciliationState::Unavailable {
                reason: "observed backend reference did not match persisted reference".to_string(),
            },
            ProcessJobStatus::LostAfterRestart,
            Some("observed backend reference did not match persisted reference".to_string()),
        ),
    };
    ProcessJobReconciliationOutcome {
        id: facts.id,
        backend: facts.backend,
        backend_ref: facts
            .observed_backend_ref
            .filter(|_| state.is_adopted() || matches!(state, ProcessJobReconciliationState::Exited)),
        state,
        log_state,
        status,
        log_refs: facts.log_refs,
        reason,
    }
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
    pub started_at: Option<ProcessJobTimestamp>,
    pub updated_at: ProcessJobTimestamp,
    pub completed_at: Option<ProcessJobTimestamp>,
    pub log_refs: Vec<ProcessJobLogRef>,
    #[serde(
        default = "absent_process_job_profile_metadata",
        skip_serializing_if = "Option::is_none"
    )]
    pub profile: Option<ProcessJobProfileReceiptMetadata>,
}

impl ProcessJobReconciliationOutcome {
    #[must_use]
    pub fn into_summary_update(self, mut summary: ProcessJobSummary, updated_at: DateTime<Utc>) -> ProcessJobSummary {
        summary.backend = self.backend;
        summary.backend_ref = self.backend_ref;
        summary.status = self.status;
        let updated_at = process_job_timestamp(updated_at);
        summary.updated_at = updated_at;
        summary.log_refs = self.log_refs;
        if summary.status.is_terminal() && summary.completed_at.is_none() {
            summary.completed_at = Some(updated_at);
        }
        summary
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProcessJobLifecycleBucket {
    Active,
    Completed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProcessJobProjectionBounds {
    pub max_active: usize,
    pub max_completed: usize,
}

impl Default for ProcessJobProjectionBounds {
    fn default() -> Self {
        Self {
            max_active: 32,
            max_completed: 32,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProcessJobProjectionItem {
    pub id: ProcessJobId,
    pub backend: ProcessJobBackendKind,
    pub backend_label: String,
    pub backend_ref: Option<BackendRef>,
    pub capability_hints: ProcessJobSafeCapabilityHints,
    pub lifecycle: ProcessJobLifecycleBucket,
    pub status: ProcessJobStatus,
    pub status_label: String,
    pub command_preview: String,
    pub cwd: ProcessJobCwd,
    pub started_at: Option<ProcessJobTimestamp>,
    pub updated_at: ProcessJobTimestamp,
    pub completed_at: Option<ProcessJobTimestamp>,
    pub log_refs: Vec<ProcessJobLogRef>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub profile: Option<ProcessJobProfileReceiptMetadata>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProcessJobListProjection {
    pub active: Vec<ProcessJobProjectionItem>,
    pub completed: Vec<ProcessJobProjectionItem>,
    pub total_active: usize,
    pub total_completed: usize,
    pub truncated_active: bool,
    pub truncated_completed: bool,
}

#[must_use]
pub fn project_process_job_list(
    summaries: impl IntoIterator<Item = ProcessJobSummary>,
    bounds: ProcessJobProjectionBounds,
) -> ProcessJobListProjection {
    let mut active = Vec::new();
    let mut completed = Vec::new();
    for summary in summaries {
        let lifecycle = if summary.status.is_terminal() {
            ProcessJobLifecycleBucket::Completed
        } else {
            ProcessJobLifecycleBucket::Active
        };
        let item = ProcessJobProjectionItem {
            id: summary.id,
            backend: summary.backend,
            backend_label: summary.backend.label().to_string(),
            backend_ref: summary.backend_ref,
            capability_hints: ProcessJobSafeCapabilityHints::for_backend(summary.backend),
            lifecycle: lifecycle.clone(),
            status_label: summary.status.label(),
            status: summary.status,
            command_preview: summary.command_preview,
            cwd: summary.cwd,
            started_at: summary.started_at,
            updated_at: summary.updated_at,
            completed_at: summary.completed_at,
            log_refs: summary.log_refs,
            profile: summary.profile,
        };
        match lifecycle {
            ProcessJobLifecycleBucket::Active => active.push(item),
            ProcessJobLifecycleBucket::Completed => completed.push(item),
        }
    }
    active.sort_by(|left, right| right.updated_at.cmp(&left.updated_at).then_with(|| left.id.0.cmp(&right.id.0)));
    completed.sort_by(|left, right| right.updated_at.cmp(&left.updated_at).then_with(|| left.id.0.cmp(&right.id.0)));
    let total_active = active.len();
    let total_completed = completed.len();
    active.truncate(bounds.max_active);
    completed.truncate(bounds.max_completed);
    ProcessJobListProjection {
        active,
        completed,
        total_active,
        total_completed,
        truncated_active: total_active > bounds.max_active,
        truncated_completed: total_completed > bounds.max_completed,
    }
}

/// Completed-job retention policy shared by daemon automation and explicit GC requests.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProcessJobRetentionPolicy {
    pub max_age: Option<Duration>,
    pub max_records: Option<usize>,
    pub max_log_bytes: Option<u64>,
}

impl Default for ProcessJobRetentionPolicy {
    fn default() -> Self {
        Self {
            max_age: Some(Duration::from_secs(DEFAULT_PROCESS_JOB_RETENTION_SECS)),
            max_records: Some(1000),
            max_log_bytes: Some(DEFAULT_PROCESS_JOB_LOG_TOTAL_BYTES),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProcessJobRetentionMetadata {
    pub class: ProcessJobRetentionClass,
    pub metadata_retained_until: Option<ProcessJobTimestamp>,
    pub log_retained_until: Option<ProcessJobTimestamp>,
    pub event_retained_until: Option<ProcessJobTimestamp>,
    pub tombstone_retained_until: Option<ProcessJobTimestamp>,
    pub policy_ref: Option<String>,
}

impl ProcessJobRetentionPolicy {
    #[must_use]
    pub fn classify_summary(
        &self,
        summary: &ProcessJobSummary,
        _now: ProcessJobTimestamp,
        policy_ref: Option<String>,
    ) -> ProcessJobRetentionMetadata {
        let class = match &summary.status {
            ProcessJobStatus::Running | ProcessJobStatus::Pending | ProcessJobStatus::Waiting => {
                ProcessJobRetentionClass::Active
            }
            ProcessJobStatus::ReattachedLogIncomplete | ProcessJobStatus::BackendUnavailable { .. } => {
                ProcessJobRetentionClass::Adopted
            }
            ProcessJobStatus::Failed { .. }
            | ProcessJobStatus::Killed
            | ProcessJobStatus::Cancelled
            | ProcessJobStatus::LostAfterRestart => ProcessJobRetentionClass::Failed,
            ProcessJobStatus::Succeeded { .. } => ProcessJobRetentionClass::RecentCompleted,
            ProcessJobStatus::Unknown { .. } => ProcessJobRetentionClass::Tombstone,
        };
        let retention_base = summary.completed_at.unwrap_or(summary.updated_at);
        let retained_until = self.max_age.and_then(|age| add_timestamp_duration(retention_base, age));
        ProcessJobRetentionMetadata {
            class,
            metadata_retained_until: retained_until,
            log_retained_until: retained_until,
            event_retained_until: retained_until,
            tombstone_retained_until: retained_until,
            policy_ref,
        }
    }

    #[must_use]
    pub fn eligibility_for_summary(
        &self,
        summary: &ProcessJobSummary,
        now: ProcessJobTimestamp,
        policy_ref: Option<String>,
    ) -> ProcessJobRetentionEligibility {
        let metadata = self.classify_summary(summary, now, policy_ref);
        if metadata.class.protects_active_state() || !summary.status.is_terminal() {
            return ProcessJobRetentionEligibility::ProtectActive {
                id: summary.id.clone(),
                class: metadata.class,
            };
        }
        if let Some(retained_until) = metadata.metadata_retained_until
            && now < retained_until
        {
            return ProcessJobRetentionEligibility::KeepUntil {
                id: summary.id.clone(),
                retained_until,
            };
        }
        ProcessJobRetentionEligibility::Eligible {
            id: summary.id.clone(),
            class: metadata.class,
            log_refs: summary.log_refs.clone(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "decision")]
pub enum ProcessJobRetentionEligibility {
    ProtectActive {
        id: ProcessJobId,
        class: ProcessJobRetentionClass,
    },
    KeepUntil {
        id: ProcessJobId,
        retained_until: ProcessJobTimestamp,
    },
    Eligible {
        id: ProcessJobId,
        class: ProcessJobRetentionClass,
        log_refs: Vec<ProcessJobLogRef>,
    },
}

/// Backend/log reference that retention released without owning concrete backend cleanup.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProcessJobReleasedLogRef {
    pub id: ProcessJobId,
    pub backend: ProcessJobBackendKind,
    pub reference: String,
    pub bytes: u64,
}

/// Retention failure reported without aborting the whole GC request.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProcessJobGarbageCollectionFailure {
    pub id: Option<ProcessJobId>,
    pub reference: Option<String>,
    pub message: String,
}

/// Typed GC receipt for explicit and automatic completed-job retention.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProcessJobGarbageCollectionReceipt {
    pub operation: ProcessJobOperation,
    pub removed_metadata_count: usize,
    pub removed_records: Vec<ProcessJobId>,
    pub tombstoned_records: Vec<ProcessJobId>,
    pub deleted_native_log_files: usize,
    pub removed_log_bytes: u64,
    pub skipped_active_jobs: Vec<ProcessJobId>,
    pub released_log_refs: Vec<ProcessJobReleasedLogRef>,
    pub failures: Vec<ProcessJobGarbageCollectionFailure>,
    pub summary: String,
}

impl ProcessJobGarbageCollectionReceipt {
    #[must_use]
    pub fn empty() -> Self {
        Self {
            operation: ProcessJobOperation::GarbageCollect,
            removed_metadata_count: 0,
            removed_records: Vec::new(),
            tombstoned_records: Vec::new(),
            deleted_native_log_files: 0,
            removed_log_bytes: 0,
            skipped_active_jobs: Vec::new(),
            released_log_refs: Vec::new(),
            failures: Vec::new(),
            summary:
                "process job GC removed 0 metadata records, tombstoned 0 records, deleted 0 native log files, released 0 backend log refs, skipped 0 active jobs, 0 failures"
                    .to_string(),
        }
    }

    pub fn refresh_summary(&mut self) {
        self.removed_metadata_count = self.removed_records.len();
        self.summary = format!(
            "process job GC removed {} metadata records, tombstoned {} records, deleted {} native log files, released {} backend log refs, reclaimed {} log bytes, skipped {} active jobs, {} failures",
            self.removed_metadata_count,
            self.tombstoned_records.len(),
            self.deleted_native_log_files,
            self.released_log_refs.len(),
            self.removed_log_bytes,
            self.skipped_active_jobs.len(),
            self.failures.len()
        );
    }
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

/// Persisted notification event payload.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProcessJobNotificationEvent {
    pub event_id: ProcessJobEventId,
    pub id: ProcessJobId,
    pub backend: ProcessJobBackendKind,
    pub owner: ProcessJobOwnerScope,
    pub kind: ProcessJobNotificationKind,
    pub status: ProcessJobStatus,
    pub created_at: ProcessJobTimestamp,
    pub summary: String,
    pub log_excerpt: Option<String>,
    pub log_refs: Vec<ProcessJobLogRef>,
}

impl ProcessJobNotificationRedactionTarget for ProcessJobNotificationEvent {
    fn redact_with(mut self, policy: &ProcessJobRedactionPolicy) -> Self {
        self.summary = policy.safe_log_excerpt(&self.summary);
        self.log_excerpt = self.log_excerpt.as_deref().map(|excerpt| policy.safe_log_excerpt(excerpt));
        if let ProcessJobNotificationKind::WatchPattern { pattern, .. } = &mut self.kind {
            *pattern = policy.safe_command_preview(pattern);
        }
        self
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
    #[serde(default = "absent_capability_detail", skip_serializing_if = "Option::is_none")]
    pub capability_detail: Option<String>,
    pub message: String,
}

/// Fields every process/job receipt surface carries, independent of operation payload.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProcessJobReceiptCommon {
    pub operation: ProcessJobOperation,
    pub id: Option<ProcessJobId>,
    pub backend: Option<ProcessJobBackendKind>,
    pub status: Option<ProcessJobStatus>,
    pub backend_ref: Option<BackendRef>,
    #[serde(
        default = "absent_process_job_profile_metadata",
        skip_serializing_if = "Option::is_none"
    )]
    pub profile: Option<ProcessJobProfileReceiptMetadata>,
    pub summary: String,
    pub error: Option<ProcessJobError>,
}

/// Operation-specific receipt payloads layered behind a stable common envelope.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind", content = "data")]
pub enum ProcessJobReceiptPayload {
    None,
    State {
        log_refs: Vec<ProcessJobLogRef>,
    },
    List {
        jobs: Vec<ProcessJobSummary>,
    },
    Log {
        chunk: ProcessJobLogChunk,
    },
    GarbageCollect {
        receipt: ProcessJobGarbageCollectionReceipt,
    },
}

/// Stable receipt envelope for all process/job operations.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProcessJobToolReceipt {
    pub common: ProcessJobReceiptCommon,
    pub payload: ProcessJobReceiptPayload,
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
    #[serde(
        default = "absent_process_job_profile_metadata",
        skip_serializing_if = "Option::is_none"
    )]
    pub profile: Option<ProcessJobProfileReceiptMetadata>,
    pub summary: String,
    pub error: Option<ProcessJobError>,
}

/// Options for constructing an unsupported-backend receipt with capability detail.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProcessJobUnsupportedDetail {
    pub operation: ProcessJobOperation,
    pub id: Option<ProcessJobId>,
    pub backend: ProcessJobBackendKind,
    pub action: String,
    pub capability_detail: Option<String>,
    pub message: String,
}

impl ProcessJobReceipt {
    #[must_use]
    pub fn common(&self) -> ProcessJobReceiptCommon {
        ProcessJobReceiptCommon {
            operation: self.operation,
            id: self.id.clone(),
            backend: self.backend,
            status: self.status.clone(),
            backend_ref: self.backend_ref.clone(),
            profile: self.profile.clone(),
            summary: self.summary.clone(),
            error: self.error.clone(),
        }
    }

    #[must_use]
    pub fn state_payload(&self) -> ProcessJobReceiptPayload {
        ProcessJobReceiptPayload::State {
            log_refs: self.log_refs.clone(),
        }
    }

    #[must_use]
    pub fn into_tool_receipt(self) -> ProcessJobToolReceipt {
        ProcessJobToolReceipt {
            common: self.common(),
            payload: self.state_payload(),
        }
    }

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
            profile: None,
            summary: message.clone(),
            error: Some(ProcessJobError {
                code: ProcessJobErrorCode::PermissionDenied,
                operation,
                id: None,
                backend: Some(backend),
                action: Some(action.into()),
                capability_detail: None,
                message,
            }),
        }
    }

    #[must_use]
    pub fn backend_unavailable(
        operation: ProcessJobOperation,
        backend: ProcessJobBackendKind,
        reason: impl Into<String>,
    ) -> Self {
        let reason = reason.into();
        Self {
            operation,
            id: None,
            backend: Some(backend),
            status: Some(ProcessJobStatus::BackendUnavailable { reason: reason.clone() }),
            backend_ref: None,
            log_refs: Vec::new(),
            profile: None,
            summary: reason.clone(),
            error: Some(ProcessJobError {
                code: ProcessJobErrorCode::BackendUnavailable,
                operation,
                id: None,
                backend: Some(backend),
                action: Some(operation.action_name().to_string()),
                capability_detail: None,
                message: reason,
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
        Self::unsupported_with_detail(ProcessJobUnsupportedDetail {
            operation,
            id,
            backend,
            action: action.into(),
            capability_detail: None,
            message: message.into(),
        })
    }

    #[must_use]
    pub fn unsupported_with_detail(detail: ProcessJobUnsupportedDetail) -> Self {
        let operation = detail.operation;
        let id = detail.id;
        let backend = detail.backend;
        let message = detail.message;
        Self {
            operation,
            id: id.clone(),
            backend: Some(backend),
            status: None,
            backend_ref: None,
            log_refs: Vec::new(),
            profile: None,
            summary: message.clone(),
            error: Some(ProcessJobError {
                code: ProcessJobErrorCode::UnsupportedActionForBackend,
                operation,
                id,
                backend: Some(backend),
                action: Some(detail.action),
                capability_detail: detail.capability_detail,
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
    GarbageCollect(ProcessJobGarbageCollectionReceipt),
}

impl ProcessJobToolResult {
    #[must_use]
    pub fn into_receipt(self) -> ProcessJobToolReceipt {
        match self {
            Self::Start(receipt)
            | Self::Poll(receipt)
            | Self::Wait(receipt)
            | Self::Kill(receipt)
            | Self::Restart(receipt)
            | Self::WriteStdin(receipt)
            | Self::CloseStdin(receipt)
            | Self::Adopt(receipt) => receipt.into_tool_receipt(),
            Self::List(jobs) => ProcessJobToolReceipt {
                common: ProcessJobReceiptCommon {
                    operation: ProcessJobOperation::List,
                    id: None,
                    backend: None,
                    status: None,
                    backend_ref: None,
                    profile: None,
                    summary: format!("Listed {} process jobs", jobs.len()),
                    error: None,
                },
                payload: ProcessJobReceiptPayload::List { jobs },
            },
            Self::Log(chunk) => ProcessJobToolReceipt {
                common: ProcessJobReceiptCommon {
                    operation: ProcessJobOperation::Log,
                    id: Some(chunk.id.clone()),
                    backend: None,
                    status: None,
                    backend_ref: None,
                    profile: None,
                    summary: format!("Read {} bytes of process job log", chunk.text.len()),
                    error: None,
                },
                payload: ProcessJobReceiptPayload::Log { chunk },
            },
            Self::GarbageCollect(receipt) => ProcessJobToolReceipt {
                common: ProcessJobReceiptCommon {
                    operation: ProcessJobOperation::GarbageCollect,
                    id: None,
                    backend: None,
                    status: None,
                    backend_ref: None,
                    profile: None,
                    summary: receipt.summary.clone(),
                    error: None,
                },
                payload: ProcessJobReceiptPayload::GarbageCollect { receipt },
            },
        }
    }
}

/// Canonical, versioned input envelope for BLAKE3-native public process/job ids.
///
/// Backend-owned locators such as PIDs, pueue task ids, and systemd unit names do
/// not belong in this envelope. They are carried separately by [`BackendRef`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProcessJobIdentityEnvelope {
    pub version: u8,
    pub domain: String,
    pub backend: ProcessJobBackendKind,
    pub owner: ProcessJobOwnerScope,
    pub command_preview: String,
    pub cwd: ProcessJobCwd,
    pub profile: Option<String>,
    pub request_nonce: String,
    #[serde(default = "empty_process_job_identity_metadata")]
    pub metadata: BTreeMap<String, String>,
}

impl ProcessJobIdentityEnvelope {
    #[must_use]
    pub fn for_start_request(request: &StartProcessJobRequest, request_nonce: impl Into<String>) -> Self {
        let redaction = ProcessJobRedactionPolicy::default();
        Self {
            version: PROCESS_JOB_IDENTITY_VERSION,
            domain: PROCESS_JOB_IDENTITY_DOMAIN.to_string(),
            backend: request.backend,
            owner: request.owner.clone(),
            command_preview: redaction.safe_command_preview(&request.command_preview),
            cwd: request.cwd.clone(),
            profile: request.metadata.get("profile").cloned(),
            request_nonce: request_nonce.into(),
            metadata: redaction.safe_identity_metadata(&request.metadata),
        }
    }

    #[must_use]
    pub fn canonical_bytes(&self) -> Vec<u8> {
        let mut fields = vec![
            ("version".to_string(), self.version.to_string()),
            ("domain".to_string(), self.domain.clone()),
            ("backend".to_string(), self.backend.label().to_string()),
            ("owner.kind".to_string(), owner_kind(&self.owner).to_string()),
            ("owner.value".to_string(), owner_value(&self.owner).unwrap_or_default()),
            ("command_preview".to_string(), self.command_preview.clone()),
            ("cwd.kind".to_string(), cwd_kind(&self.cwd).to_string()),
            ("cwd.path".to_string(), cwd_path(&self.cwd).unwrap_or_default()),
            ("profile".to_string(), self.profile.clone().unwrap_or_default()),
            ("request_nonce".to_string(), self.request_nonce.clone()),
        ];
        fields.reserve(self.metadata.len());
        for (key, value) in &self.metadata {
            fields.push((format!("metadata.{key}"), value.clone()));
        }
        fields.sort_by(|left, right| left.0.cmp(&right.0));

        let canonical_capacity_bytes =
            fields.iter().fold(b"clankers-process-job-identity-v1\n".len(), |capacity, (key, value)| {
                capacity.saturating_add(key.len()).saturating_add(value.len()).saturating_add(32)
            });
        let mut canonical = Vec::with_capacity(canonical_capacity_bytes);
        canonical.extend_from_slice(b"clankers-process-job-identity-v1\n");
        for (key, value) in fields {
            canonical.extend_from_slice(key.len().to_string().as_bytes());
            canonical.push(b':');
            canonical.extend_from_slice(key.as_bytes());
            canonical.push(b'=');
            canonical.extend_from_slice(value.len().to_string().as_bytes());
            canonical.push(b':');
            canonical.extend_from_slice(value.as_bytes());
            canonical.push(b'\n');
        }
        canonical
    }

    #[must_use]
    pub fn derive_id(&self) -> ProcessJobId {
        let hash = blake3::hash(&self.canonical_bytes());
        ProcessJobId(format!("{PROCESS_JOB_ID_PREFIX}{}", hash.to_hex()))
    }
}

fn owner_kind(owner: &ProcessJobOwnerScope) -> &'static str {
    match owner {
        ProcessJobOwnerScope::Session(_) => "session",
        ProcessJobOwnerScope::Workspace(_) => "workspace",
        ProcessJobOwnerScope::User(_) => "user",
        ProcessJobOwnerScope::DaemonGlobal => "daemon_global",
    }
}

fn owner_value(owner: &ProcessJobOwnerScope) -> Option<String> {
    match owner {
        ProcessJobOwnerScope::Session(value)
        | ProcessJobOwnerScope::Workspace(value)
        | ProcessJobOwnerScope::User(value) => Some(value.clone()),
        ProcessJobOwnerScope::DaemonGlobal => None,
    }
}

fn cwd_kind(cwd: &ProcessJobCwd) -> &'static str {
    match cwd {
        ProcessJobCwd::Inherited => "inherited",
        ProcessJobCwd::Explicit(_) => "explicit",
    }
}

fn cwd_path(cwd: &ProcessJobCwd) -> Option<String> {
    match cwd {
        ProcessJobCwd::Inherited => None,
        ProcessJobCwd::Explicit(path) => Some(path.to_string_lossy().into_owned()),
    }
}

/// Accepted notification policy. Continuous output stays in logs.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct ProcessJobNotificationPolicy {
    #[serde(default)]
    pub notify_on_complete: bool,
    #[serde(default = "empty_process_job_watch_patterns")]
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProcessJobRedactionPolicy {
    pub max_preview_chars: usize,
    pub max_excerpt_chars: usize,
    pub max_metadata_value_chars: usize,
}

impl Default for ProcessJobRedactionPolicy {
    fn default() -> Self {
        Self {
            max_preview_chars: PROCESS_JOB_MAX_SAFE_PREVIEW_CHARS,
            max_excerpt_chars: PROCESS_JOB_MAX_SAFE_EXCERPT_CHARS,
            max_metadata_value_chars: PROCESS_JOB_MAX_SAFE_METADATA_VALUE_CHARS,
        }
    }
}

impl ProcessJobRedactionPolicy {
    #[must_use]
    pub fn safe_command_preview(&self, raw: &str) -> String {
        self.safe_text(raw, self.max_preview_chars)
    }

    #[must_use]
    pub fn safe_log_excerpt(&self, raw: &str) -> String {
        self.safe_text(raw, self.max_excerpt_chars)
    }

    #[must_use]
    pub fn safe_metadata_value(&self, key: &str, value: &str) -> String {
        if is_sensitive_process_job_key(key) || contains_sensitive_process_job_marker(value) {
            PROCESS_JOB_REDACTED.to_string()
        } else {
            bound_chars(value, self.max_metadata_value_chars)
        }
    }

    #[must_use]
    pub fn safe_identity_metadata(&self, metadata: &BTreeMap<String, String>) -> BTreeMap<String, String> {
        metadata
            .iter()
            .filter(|(key, _)| key.starts_with("identity.") || key.as_str() == "profile")
            .map(|(key, value)| (key.clone(), self.safe_metadata_value(key, value)))
            .collect()
    }

    #[must_use]
    pub fn safe_notification_decision(
        &self,
        mut decision: ProcessJobNotificationDecision,
    ) -> ProcessJobNotificationDecision {
        decision.summary = self.safe_log_excerpt(&decision.summary);
        decision.log_excerpt = decision.log_excerpt.as_deref().map(|excerpt| self.safe_log_excerpt(excerpt));
        if let ProcessJobNotificationKind::WatchPattern { pattern, .. } = &mut decision.kind {
            *pattern = self.safe_command_preview(pattern);
        }
        decision
    }

    #[must_use]
    pub fn safe_notification_event<Event>(&self, event: Event) -> Event
    where Event: ProcessJobNotificationRedactionTarget {
        event.redact_with(self)
    }

    fn safe_text(&self, raw: &str, max_chars: usize) -> String {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            return String::new();
        }
        if contains_sensitive_process_job_marker(trimmed) {
            return PROCESS_JOB_REDACTED.to_string();
        }
        bound_chars(trimmed, max_chars)
    }
}

pub trait ProcessJobNotificationRedactionTarget: Sized {
    #[must_use]
    fn redact_with(self, policy: &ProcessJobRedactionPolicy) -> Self;
}

fn is_sensitive_process_job_key(key: &str) -> bool {
    let lowered = key.to_ascii_lowercase();
    PROCESS_JOB_SENSITIVE_MARKERS.iter().any(|marker| lowered.contains(marker))
}

fn contains_sensitive_process_job_marker(text: &str) -> bool {
    let lowered = text.to_ascii_lowercase();
    PROCESS_JOB_SENSITIVE_MARKERS.iter().any(|marker| lowered.contains(marker))
}

fn bound_chars(value: &str, max_chars: usize) -> String {
    let mut bounded = String::new();
    for (index, ch) in value.chars().enumerate() {
        if index >= max_chars {
            bounded.push('…');
            return bounded;
        }
        bounded.push(ch);
    }
    bounded
}

const PROCESS_JOB_SENSITIVE_MARKERS: &[&str] = &[
    "authorization",
    "bearer ",
    "cookie",
    "password",
    "passwd",
    "secret",
    "token",
    "api_key",
    "apikey",
    "access_key",
    "credential",
];

/// Safe, backend-neutral profile metadata copied into process/job receipts.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProcessJobProfileReceiptMetadata {
    pub profile_name: String,
    pub manifest_schema_version: u32,
    pub profile_source: String,
    pub policy_source: String,
}

impl ProcessJobProfileReceiptMetadata {
    #[must_use]
    pub fn from_metadata(metadata: &BTreeMap<String, String>) -> Option<Self> {
        let profile_name = metadata.get(PROCESS_JOB_PROFILE_METADATA_NAME)?.clone();
        let manifest_schema_version = metadata.get(PROCESS_JOB_PROFILE_METADATA_SCHEMA_VERSION)?.parse().ok()?;
        let profile_source = metadata.get(PROCESS_JOB_PROFILE_METADATA_SOURCE)?.clone();
        let policy_source = metadata.get(PROCESS_JOB_PROFILE_METADATA_POLICY)?.clone();
        Some(Self {
            profile_name,
            manifest_schema_version,
            profile_source,
            policy_source,
        })
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

/// Policy bounds for resolving project-defined process/job profiles.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectProcessJobProfilePolicy {
    pub default_backend: ProcessJobBackendKind,
    pub allowed_backends: Vec<ProcessJobBackendKind>,
    pub max_timeout: Option<Duration>,
    pub max_memory_bytes: Option<u64>,
    pub max_cpu_quota_percent: Option<u32>,
    pub max_log_bytes: Option<u64>,
    pub allowed_env_prefixes: Vec<String>,
    pub allowed_cwd_prefixes: Vec<PathBuf>,
    pub allowed_writable_path_prefixes: Vec<PathBuf>,
    pub policy_source: String,
}

impl Default for ProjectProcessJobProfilePolicy {
    fn default() -> Self {
        Self {
            default_backend: ProcessJobBackendKind::Native,
            allowed_backends: vec![ProcessJobBackendKind::Native],
            max_timeout: None,
            max_memory_bytes: None,
            max_cpu_quota_percent: None,
            max_log_bytes: None,
            allowed_env_prefixes: Vec::new(),
            allowed_cwd_prefixes: Vec::new(),
            allowed_writable_path_prefixes: Vec::new(),
            policy_source: "default".to_string(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProjectProcessJobProfileSourcePrecedence {
    Global,
    Workspace,
    Explicit,
}

impl ProjectProcessJobProfileSourcePrecedence {
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Global => "global",
            Self::Workspace => "workspace",
            Self::Explicit => "explicit",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProjectProcessJobProfileValidationCode {
    InvalidManifestJson,
    UnknownProfile,
    UnsupportedManifestVersion,
    AmbiguousManifestSource,
    DisallowedBackend,
    MalformedCommandShape,
    DisallowedEnvironmentKey,
    ResourceLimitExceeded,
    DisallowedCwd,
    DisallowedWritablePath,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectProcessJobProfileValidationError {
    pub code: ProjectProcessJobProfileValidationCode,
    pub profile: String,
    pub reason: String,
}

impl std::fmt::Display for ProjectProcessJobProfileValidationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "process job profile {} validation failed ({:?}): {}",
            self.profile, self.code, self.reason
        )
    }
}

impl ProjectProcessJobProfileValidationError {
    #[must_use]
    pub fn new(
        code: ProjectProcessJobProfileValidationCode,
        profile: impl Into<String>,
        reason: impl Into<String>,
    ) -> Self {
        Self {
            code,
            profile: profile.into(),
            reason: reason.into(),
        }
    }
}

pub const PROCESS_JOB_PROFILE_SCHEMA_VERSION: u32 = 1;

fn default_process_job_profile_schema_version() -> u32 {
    PROCESS_JOB_PROFILE_SCHEMA_VERSION
}

/// Named project process/job profile collection. Parsing this type is pure and
/// never dispatches to native, pueue, systemd, or storage adapters.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectProcessJobProfiles {
    #[serde(default = "default_process_job_profile_schema_version")]
    pub schema_version: u32,
    #[serde(default)]
    pub profiles: BTreeMap<String, ProjectProcessJobProfile>,
}

impl Default for ProjectProcessJobProfiles {
    fn default() -> Self {
        Self {
            schema_version: PROCESS_JOB_PROFILE_SCHEMA_VERSION,
            profiles: BTreeMap::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct ProjectProcessJobProfile {
    pub backend: Option<ProcessJobBackendKind>,
    pub command: Option<String>,
    pub program: Option<String>,
    #[serde(default)]
    pub args: Vec<String>,
    pub cwd: Option<PathBuf>,
    #[serde(default)]
    pub writable_paths: Vec<PathBuf>,
    #[serde(default)]
    pub env: BTreeMap<String, String>,
    #[serde(default)]
    pub resource_policy: ProcessJobResourcePolicy,
    #[serde(default)]
    pub notification_policy: ProcessJobNotificationPolicy,
    #[serde(default)]
    pub metadata: BTreeMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectProcessJobProfileResolution {
    pub name: String,
    pub request: ProcessJobSpec,
    pub evidence: ProjectProcessJobProfileResolutionEvidence,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectProcessJobProfileResolutionEvidence {
    pub profile_name: String,
    pub manifest_schema_version: u32,
    pub profile_source: String,
    pub policy_source: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectProcessJobProfileManifestSource {
    pub precedence: ProjectProcessJobProfileSourcePrecedence,
    pub label: String,
    pub path: Option<PathBuf>,
    pub manifest: ProjectProcessJobProfiles,
}

impl ProjectProcessJobProfileManifestSource {
    #[must_use]
    pub fn safe_label(&self) -> String {
        self.path.as_ref().map_or_else(|| self.label.clone(), |path| path.to_string_lossy().into_owned())
    }
}

impl ProjectProcessJobProfiles {
    pub fn from_json_str(input: &str) -> Result<Self, ProjectProcessJobProfileValidationError> {
        serde_json::from_str(input).map_err(|err| {
            ProjectProcessJobProfileValidationError::new(
                ProjectProcessJobProfileValidationCode::InvalidManifestJson,
                "<manifest>",
                format!("invalid process job profiles config: {err}"),
            )
        })
    }

    pub fn resolve(
        &self,
        name: &str,
        owner: ProcessJobOwnerScope,
        policy: &ProjectProcessJobProfilePolicy,
    ) -> Result<ProjectProcessJobProfileResolution, ProjectProcessJobProfileValidationError> {
        self.resolve_with_evidence(name, owner, policy, ProjectProcessJobProfileResolutionEvidence {
            profile_name: name.to_string(),
            manifest_schema_version: self.schema_version,
            profile_source: "inline".to_string(),
            policy_source: policy.policy_source.clone(),
        })
    }

    pub fn resolve_with_evidence(
        &self,
        name: &str,
        owner: ProcessJobOwnerScope,
        policy: &ProjectProcessJobProfilePolicy,
        evidence: ProjectProcessJobProfileResolutionEvidence,
    ) -> Result<ProjectProcessJobProfileResolution, ProjectProcessJobProfileValidationError> {
        validate_profile_manifest_version(name, self.schema_version)?;
        let profile = self.profiles.get(name).ok_or_else(|| {
            ProjectProcessJobProfileValidationError::new(
                ProjectProcessJobProfileValidationCode::UnknownProfile,
                name,
                format!("unknown process job profile: {name}"),
            )
        })?;
        profile.resolve_named(name, owner, policy, evidence)
    }

    pub fn resolve_from_sources(
        sources: &[ProjectProcessJobProfileManifestSource],
        name: &str,
        owner: ProcessJobOwnerScope,
        policy: &ProjectProcessJobProfilePolicy,
    ) -> Result<ProjectProcessJobProfileResolution, ProjectProcessJobProfileValidationError> {
        let selected = select_profile_manifest_source(sources, name)?;
        selected
            .manifest
            .resolve_with_evidence(name, owner, policy, ProjectProcessJobProfileResolutionEvidence {
                profile_name: name.to_string(),
                manifest_schema_version: selected.manifest.schema_version,
                profile_source: selected.safe_label(),
                policy_source: policy.policy_source.clone(),
            })
    }
}

fn select_profile_manifest_source<'a>(
    sources: &'a [ProjectProcessJobProfileManifestSource],
    name: &str,
) -> Result<&'a ProjectProcessJobProfileManifestSource, ProjectProcessJobProfileValidationError> {
    let mut matches: BTreeMap<ProjectProcessJobProfileSourcePrecedence, Vec<&ProjectProcessJobProfileManifestSource>> =
        BTreeMap::new();
    for source in sources {
        if source.manifest.profiles.contains_key(name) {
            matches.entry(source.precedence).or_default().push(source);
        }
    }
    let Some((_, selected_at_level)) = matches.iter().next_back() else {
        return Err(ProjectProcessJobProfileValidationError::new(
            ProjectProcessJobProfileValidationCode::UnknownProfile,
            name,
            format!("unknown process job profile: {name}"),
        ));
    };
    if selected_at_level.len() != 1 {
        let labels = selected_at_level.iter().map(|source| source.safe_label()).collect::<Vec<_>>().join(", ");
        return Err(ProjectProcessJobProfileValidationError::new(
            ProjectProcessJobProfileValidationCode::AmbiguousManifestSource,
            name,
            format!("ambiguous duplicate profile at same precedence: {labels}"),
        ));
    }
    Ok(selected_at_level[0])
}

impl ProjectProcessJobProfile {
    fn resolve_named(
        &self,
        name: &str,
        owner: ProcessJobOwnerScope,
        policy: &ProjectProcessJobProfilePolicy,
        evidence: ProjectProcessJobProfileResolutionEvidence,
    ) -> Result<ProjectProcessJobProfileResolution, ProjectProcessJobProfileValidationError> {
        let backend = self.backend.unwrap_or(policy.default_backend);
        validate_profile_backend(name, backend, policy)?;
        validate_profile_command_shape(name, self)?;
        validate_profile_environment(name, &self.env, policy)?;
        validate_profile_resources(name, &self.resource_policy, policy)?;
        validate_profile_paths(name, self, policy)?;

        let cwd = self.cwd.clone().map_or(ProcessJobCwd::Inherited, ProcessJobCwd::Explicit);
        let mut metadata = self.metadata.clone();
        metadata.insert(PROCESS_JOB_PROFILE_METADATA_NAME.to_string(), name.to_string());
        metadata.insert(
            PROCESS_JOB_PROFILE_METADATA_SCHEMA_VERSION.to_string(),
            evidence.manifest_schema_version.to_string(),
        );
        metadata.insert(PROCESS_JOB_PROFILE_METADATA_SOURCE.to_string(), evidence.profile_source.clone());
        metadata.insert(PROCESS_JOB_PROFILE_METADATA_POLICY.to_string(), evidence.policy_source.clone());
        for (key, value) in &self.env {
            metadata.insert(format!("env:{key}"), value.clone());
        }
        let command_preview = self.command.clone().unwrap_or_else(|| {
            std::iter::once(self.program.clone().unwrap_or_default())
                .chain(self.args.clone())
                .collect::<Vec<_>>()
                .join(" ")
        });

        Ok(ProjectProcessJobProfileResolution {
            name: name.to_string(),
            request: StartProcessJobRequest {
                backend,
                command_preview,
                program: self.program.clone(),
                args: self.args.clone(),
                shell_command: self.command.clone(),
                cwd,
                owner,
                resource_policy: self.resource_policy.clone(),
                notification_policy: self.notification_policy.clone(),
                metadata,
            },
            evidence,
        })
    }
}

fn validate_profile_manifest_version(
    name: &str,
    schema_version: u32,
) -> Result<(), ProjectProcessJobProfileValidationError> {
    if schema_version != PROCESS_JOB_PROFILE_SCHEMA_VERSION {
        return Err(ProjectProcessJobProfileValidationError::new(
            ProjectProcessJobProfileValidationCode::UnsupportedManifestVersion,
            name,
            format!("unsupported process job profile manifest schema_version {schema_version}"),
        ));
    }
    Ok(())
}

fn validate_profile_backend(
    name: &str,
    backend: ProcessJobBackendKind,
    policy: &ProjectProcessJobProfilePolicy,
) -> Result<(), ProjectProcessJobProfileValidationError> {
    if backend == ProcessJobBackendKind::Unknown || !policy.allowed_backends.contains(&backend) {
        return Err(ProjectProcessJobProfileValidationError::new(
            ProjectProcessJobProfileValidationCode::DisallowedBackend,
            name,
            format!("uses disallowed backend {backend:?}"),
        ));
    }
    Ok(())
}

fn validate_profile_command_shape(
    name: &str,
    profile: &ProjectProcessJobProfile,
) -> Result<(), ProjectProcessJobProfileValidationError> {
    let has_command = profile.command.as_ref().is_some_and(|value| !value.trim().is_empty());
    let has_program = profile.program.as_ref().is_some_and(|value| !value.trim().is_empty());
    if has_command == has_program {
        return Err(ProjectProcessJobProfileValidationError::new(
            ProjectProcessJobProfileValidationCode::MalformedCommandShape,
            name,
            "must set exactly one of command or program",
        ));
    }
    if !has_program && !profile.args.is_empty() {
        return Err(ProjectProcessJobProfileValidationError::new(
            ProjectProcessJobProfileValidationCode::MalformedCommandShape,
            name,
            "cannot set args without program",
        ));
    }
    Ok(())
}

fn validate_profile_environment(
    name: &str,
    env: &BTreeMap<String, String>,
    policy: &ProjectProcessJobProfilePolicy,
) -> Result<(), ProjectProcessJobProfileValidationError> {
    for key in env.keys() {
        let allowed = key.chars().all(|ch| ch.is_ascii_uppercase() || ch.is_ascii_digit() || ch == '_')
            && !is_sensitive_process_job_key(key)
            && policy.allowed_env_prefixes.iter().any(|prefix| key.starts_with(prefix));
        if !allowed {
            return Err(ProjectProcessJobProfileValidationError::new(
                ProjectProcessJobProfileValidationCode::DisallowedEnvironmentKey,
                name,
                format!("has disallowed environment key {key}"),
            ));
        }
    }
    Ok(())
}

fn validate_profile_resources(
    name: &str,
    resources: &ProcessJobResourcePolicy,
    policy: &ProjectProcessJobProfilePolicy,
) -> Result<(), ProjectProcessJobProfileValidationError> {
    if let (Some(actual), Some(maximum)) = (resources.timeout, policy.max_timeout)
        && actual > maximum
    {
        return Err(ProjectProcessJobProfileValidationError::new(
            ProjectProcessJobProfileValidationCode::ResourceLimitExceeded,
            name,
            "timeout exceeds policy",
        ));
    }
    if let (Some(actual), Some(maximum)) = (resources.memory_max_bytes, policy.max_memory_bytes)
        && actual > maximum
    {
        return Err(ProjectProcessJobProfileValidationError::new(
            ProjectProcessJobProfileValidationCode::ResourceLimitExceeded,
            name,
            "memory exceeds policy",
        ));
    }
    if let (Some(actual), Some(maximum)) = (resources.cpu_quota_percent, policy.max_cpu_quota_percent)
        && actual > maximum
    {
        return Err(ProjectProcessJobProfileValidationError::new(
            ProjectProcessJobProfileValidationCode::ResourceLimitExceeded,
            name,
            "cpu quota exceeds policy",
        ));
    }
    if let (Some(actual), Some(maximum)) = (resources.max_log_bytes, policy.max_log_bytes)
        && actual > maximum
    {
        return Err(ProjectProcessJobProfileValidationError::new(
            ProjectProcessJobProfileValidationCode::ResourceLimitExceeded,
            name,
            "log bytes exceeds policy",
        ));
    }
    Ok(())
}

fn validate_profile_paths(
    name: &str,
    profile: &ProjectProcessJobProfile,
    policy: &ProjectProcessJobProfilePolicy,
) -> Result<(), ProjectProcessJobProfileValidationError> {
    if let Some(cwd) = &profile.cwd
        && !path_allowed(cwd, &policy.allowed_cwd_prefixes)
    {
        return Err(ProjectProcessJobProfileValidationError::new(
            ProjectProcessJobProfileValidationCode::DisallowedCwd,
            name,
            format!("cwd is outside allowed prefixes: {}", cwd.to_string_lossy()),
        ));
    }
    for writable_path in &profile.writable_paths {
        if !path_allowed(writable_path, &policy.allowed_writable_path_prefixes) {
            return Err(ProjectProcessJobProfileValidationError::new(
                ProjectProcessJobProfileValidationCode::DisallowedWritablePath,
                name,
                format!("writable path is outside allowed prefixes: {}", writable_path.to_string_lossy()),
            ));
        }
    }
    Ok(())
}

fn path_allowed(path: &std::path::Path, allowed_prefixes: &[PathBuf]) -> bool {
    allowed_prefixes.is_empty() || allowed_prefixes.iter().any(|prefix| path.starts_with(prefix))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProcessJobRetentionClass {
    Active,
    RecentCompleted,
    Failed,
    Adopted,
    Notification,
    Tombstone,
}

impl ProcessJobRetentionClass {
    #[must_use]
    pub fn protects_active_state(self) -> bool {
        matches!(self, Self::Active | Self::Adopted)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProcessJobLogOverflowPolicy {
    pub max_line_bytes: u64,
    pub max_chunk_bytes: u64,
    pub max_file_bytes: u64,
    pub max_total_bytes: u64,
}

impl Default for ProcessJobLogOverflowPolicy {
    fn default() -> Self {
        Self {
            max_line_bytes: DEFAULT_PROCESS_JOB_LOG_LINE_BYTES,
            max_chunk_bytes: DEFAULT_PROCESS_JOB_LOG_CHUNK_BYTES,
            max_file_bytes: DEFAULT_PROCESS_JOB_LOG_FILE_BYTES,
            max_total_bytes: DEFAULT_PROCESS_JOB_LOG_TOTAL_BYTES,
        }
    }
}

impl ProcessJobLogOverflowPolicy {
    #[must_use]
    pub fn classify_write(&self, line_bytes: u64, chunk_bytes: u64, total_bytes: u64) -> ProcessJobLogWriteDisposition {
        if line_bytes > self.max_line_bytes {
            ProcessJobLogWriteDisposition::TruncateLine {
                dropped_bytes: line_bytes.saturating_sub(self.max_line_bytes),
            }
        } else if chunk_bytes > self.max_chunk_bytes {
            ProcessJobLogWriteDisposition::TruncateChunk {
                dropped_bytes: chunk_bytes.saturating_sub(self.max_chunk_bytes),
            }
        } else if total_bytes > self.max_total_bytes || total_bytes > self.max_file_bytes {
            ProcessJobLogWriteDisposition::DegradeDiskFull
        } else {
            ProcessJobLogWriteDisposition::Accept
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind")]
pub enum ProcessJobLogWriteDisposition {
    Accept,
    TruncateLine { dropped_bytes: u64 },
    TruncateChunk { dropped_bytes: u64 },
    DegradeDiskFull,
}

/// Backend-neutral native-process admission decision.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProcessJobNativeAdmissionDecision {
    pub accepted: bool,
    pub active: usize,
    pub limit: usize,
}

/// Named input for native-process admission checks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProcessJobNativeAdmissionInput {
    pub active: usize,
    pub limit: usize,
}

impl ProcessJobNativeAdmissionDecision {
    #[must_use]
    pub fn summary(&self) -> String {
        format!(
            "native process admission denied: active process limit reached ({active}/{limit})",
            active = self.active,
            limit = self.limit,
        )
    }
}

#[must_use]
pub fn native_process_job_admission_decision(
    input: ProcessJobNativeAdmissionInput,
) -> ProcessJobNativeAdmissionDecision {
    ProcessJobNativeAdmissionDecision {
        accepted: input.active < input.limit,
        active: input.active,
        limit: input.limit,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn process_job_identity_and_id_helpers_are_stable() {
        let request = StartProcessJobRequest {
            backend: ProcessJobBackendKind::Native,
            command_preview: "run --token raw-token".to_string(),
            program: Some("run".to_string()),
            args: vec!["--token".to_string(), "raw-token".to_string()],
            shell_command: None,
            cwd: ProcessJobCwd::Inherited,
            owner: ProcessJobOwnerScope::Session("sess".to_string()),
            resource_policy: ProcessJobResourcePolicy::default(),
            notification_policy: ProcessJobNotificationPolicy::default(),
            metadata: BTreeMap::from([("identity.token".to_string(), "raw-token".to_string())]),
        };
        let envelope = ProcessJobIdentityEnvelope::for_start_request(&request, "nonce");
        let id = ProcessJobId::from_identity_envelope(&envelope);
        let canonical = String::from_utf8(envelope.canonical_bytes()).expect("canonical identity is utf8");

        assert!(id.is_blake3_native());
        assert!(!ProcessJobId::legacy("native_pid_42").is_blake3_native());
        assert!(canonical.contains(PROCESS_JOB_REDACTED));
        assert!(!canonical.contains("raw-token"));
    }

    #[test]
    fn tool_request_maps_to_operation_vocabulary() {
        let request = ProcessJobToolRequest::WriteStdin(WriteProcessJobStdinRequest {
            id: ProcessJobId::legacy("proc_1"),
            data: b"hello".to_vec(),
            newline: true,
        });
        assert_eq!(request.operation(), ProcessJobOperation::WriteStdin);

        let gc = ProcessJobToolRequest::GarbageCollect(GarbageCollectProcessJobsRequest {
            filter: ProcessJobFilter {
                owner: Some(ProcessJobOwnerScope::DaemonGlobal),
                backend: Some(ProcessJobBackendKind::Pueue),
                include_terminal: true,
            },
        });
        assert_eq!(gc.operation(), ProcessJobOperation::GarbageCollect);
    }

    #[test]
    fn receipt_errors_and_tool_result_envelopes_are_backend_neutral() {
        let id = ProcessJobId::legacy("proc_1");
        let receipt = ProcessJobReceipt::unsupported_with_detail(ProcessJobUnsupportedDetail {
            operation: ProcessJobOperation::WriteStdin,
            id: Some(id.clone()),
            backend: ProcessJobBackendKind::Pueue,
            action: "write_stdin".to_string(),
            capability_detail: Some("stdin requires stdin support".to_string()),
            message: "cannot write stdin".to_string(),
        });
        let tool_receipt = receipt.clone().into_tool_receipt();

        assert_eq!(
            receipt.error.as_ref().expect("unsupported receipt has error").code,
            ProcessJobErrorCode::UnsupportedActionForBackend
        );
        assert_eq!(tool_receipt.common.id, Some(id));
        assert_eq!(tool_receipt.common.summary, "cannot write stdin");
        assert_eq!(tool_receipt.payload, receipt.state_payload());

        let list_receipt = ProcessJobToolResult::List(Vec::new()).into_receipt();
        assert_eq!(list_receipt.common.operation, ProcessJobOperation::List);
        assert_eq!(list_receipt.common.summary, "Listed 0 process jobs");
    }

    #[test]
    fn retention_receipts_and_notification_events_are_backend_neutral() {
        let now = ProcessJobTimestamp::from_unix_seconds(1_000);
        let id = ProcessJobId::legacy("proc_retained");
        let log_ref = ProcessJobLogRef {
            stream: ProcessJobStream::Combined,
            reference: "native:proc_retained/combined.log".to_string(),
            retained_until: None,
            max_bytes: Some(1024),
        };
        let summary = ProcessJobSummary {
            id: id.clone(),
            backend: ProcessJobBackendKind::Native,
            backend_ref: None,
            owner: ProcessJobOwnerScope::DaemonGlobal,
            status: ProcessJobStatus::Succeeded { exit_code: Some(0) },
            command_preview: "done".to_string(),
            cwd: ProcessJobCwd::Inherited,
            started_at: Some(now),
            updated_at: now,
            completed_at: Some(now),
            log_refs: vec![log_ref.clone()],
            profile: None,
        };
        let retention = ProcessJobRetentionPolicy {
            max_age: Some(Duration::from_secs(1)),
            max_records: Some(1),
            max_log_bytes: Some(1024),
        };

        let kept = retention.eligibility_for_summary(&summary, now, Some("policy".to_string()));
        assert!(matches!(kept, ProcessJobRetentionEligibility::KeepUntil { .. }));
        let eligible = retention.eligibility_for_summary(
            &summary,
            ProcessJobTimestamp::from_unix_seconds(1_002),
            Some("policy".to_string()),
        );
        assert_eq!(eligible, ProcessJobRetentionEligibility::Eligible {
            id: id.clone(),
            class: ProcessJobRetentionClass::RecentCompleted,
            log_refs: vec![log_ref.clone()],
        });

        let mut gc = ProcessJobGarbageCollectionReceipt::empty();
        gc.removed_records.push(id.clone());
        gc.released_log_refs.push(ProcessJobReleasedLogRef {
            id: id.clone(),
            backend: ProcessJobBackendKind::Native,
            reference: log_ref.reference,
            bytes: 7,
        });
        gc.removed_log_bytes = 7;
        gc.refresh_summary();
        assert!(gc.summary.contains("removed 1 metadata records"));
        assert!(gc.summary.contains("reclaimed 7 log bytes"));

        let event = ProcessJobNotificationEvent {
            event_id: ProcessJobEventId("evt".to_string()),
            id,
            backend: ProcessJobBackendKind::Native,
            owner: ProcessJobOwnerScope::DaemonGlobal,
            kind: ProcessJobNotificationKind::WatchPattern {
                pattern_index: 0,
                pattern: "token=secret".to_string(),
            },
            status: ProcessJobStatus::Running,
            created_at: now,
            summary: "token=secret".to_string(),
            log_excerpt: Some("bearer secret".to_string()),
            log_refs: Vec::new(),
        };
        let redacted = ProcessJobRedactionPolicy::default().safe_notification_event(event);
        assert_eq!(redacted.summary, PROCESS_JOB_REDACTED);
        assert_eq!(redacted.log_excerpt.as_deref(), Some(PROCESS_JOB_REDACTED));
        assert!(
            matches!(redacted.kind, ProcessJobNotificationKind::WatchPattern { ref pattern, .. } if pattern == PROCESS_JOB_REDACTED)
        );
    }

    #[test]
    fn native_admission_accepts_below_limit_and_denies_at_limit() {
        let accepted = native_process_job_admission_decision(ProcessJobNativeAdmissionInput { active: 1, limit: 2 });
        assert!(accepted.accepted);
        assert_eq!(accepted.active, 1);
        assert_eq!(accepted.limit, 2);

        let denied = native_process_job_admission_decision(ProcessJobNativeAdmissionInput { active: 2, limit: 2 });
        assert!(!denied.accepted);
        assert_eq!(denied.summary(), "native process admission denied: active process limit reached (2/2)");
    }

    #[test]
    fn backend_kind_and_operation_labels_are_stable() {
        assert_eq!(ProcessJobBackendKind::Native.label(), "native");
        assert_eq!(ProcessJobBackendKind::Pueue.label(), "pueue");
        assert_eq!(ProcessJobBackendKind::Systemd.label(), "systemd");
        assert_eq!(ProcessJobBackendKind::Unknown.label(), "unknown");
        assert_eq!(ProcessJobOperation::Start.action_name(), "start");
        assert_eq!(ProcessJobOperation::GarbageCollect.action_name(), "garbage_collect");
    }

    #[test]
    fn process_job_list_projection_splits_sorts_and_truncates_lifecycles() {
        let summary = |id: &str, status: ProcessJobStatus, updated_at: i64| ProcessJobSummary {
            id: ProcessJobId(id.to_string()),
            backend: ProcessJobBackendKind::Native,
            backend_ref: None,
            owner: ProcessJobOwnerScope::DaemonGlobal,
            status,
            command_preview: format!("cmd {id}"),
            cwd: ProcessJobCwd::Inherited,
            started_at: Some(ProcessJobTimestamp::from_unix_seconds(updated_at - 1)),
            updated_at: ProcessJobTimestamp::from_unix_seconds(updated_at),
            completed_at: None,
            log_refs: Vec::new(),
            profile: None,
        };
        let projection = project_process_job_list(
            vec![
                summary("active-old", ProcessJobStatus::Running, 10),
                summary("active-new", ProcessJobStatus::Waiting, 20),
                summary("done", ProcessJobStatus::Succeeded { exit_code: Some(0) }, 30),
            ],
            ProcessJobProjectionBounds {
                max_active: 1,
                max_completed: 4,
            },
        );

        assert_eq!(projection.total_active, 2);
        assert_eq!(projection.active.len(), 1);
        assert!(projection.truncated_active);
        assert_eq!(projection.active[0].id.0, "active-new");
        assert_eq!(projection.active[0].lifecycle, ProcessJobLifecycleBucket::Active);
        assert_eq!(projection.active[0].backend_label, "native");
        assert_eq!(projection.completed.len(), 1);
        assert_eq!(projection.completed[0].lifecycle, ProcessJobLifecycleBucket::Completed);
        assert!(!projection.truncated_completed);
    }

    #[test]
    fn reconciliation_report_counts_observations_and_backend_unavailability() {
        let mut report = ProcessJobReconciliationReport::default();
        report.record_observation(ProcessJobReconciliationState::Running);
        report.record_observation(ProcessJobReconciliationState::BackendUnavailable);
        report.skipped_terminal += 1;

        assert_eq!(report.checked, 2);
        assert_eq!(report.updated, 1);
        assert_eq!(report.unavailable, 1);
        assert_eq!(report.skipped_terminal, 1);
    }

    #[test]
    fn process_job_reconciliation_state_classifies_adopted_and_fail_closed() {
        assert!(ProcessJobReconciliationState::Running.is_adopted());
        assert!(ProcessJobReconciliationState::ReattachedLogIncomplete.is_adopted());
        assert!(!ProcessJobReconciliationState::LostAfterRestart.is_adopted());
        assert!(ProcessJobReconciliationState::IdentityMismatch.is_fail_closed());
        assert!(ProcessJobReconciliationState::BackendUnavailable.is_fail_closed());
    }

    #[test]
    fn process_job_timestamp_projects_chrono_seconds() {
        let timestamp = DateTime::parse_from_rfc3339("2026-05-18T00:00:07Z")
            .expect("timestamp parses")
            .with_timezone(&Utc);
        assert_eq!(process_job_timestamp(timestamp), ProcessJobTimestamp::from_unix_seconds(1_779_062_407));
    }

    #[test]
    fn external_backend_reconciliation_maps_matching_backend_facts_to_outcome() {
        let outcome = reconcile_external_backend_reference(ExternalProcessJobReconciliationFacts {
            id: ProcessJobId("proc".to_string()),
            backend: ProcessJobBackendKind::Pueue,
            expected_backend_ref: BackendRef("pueue:7".to_string()),
            observed_backend_ref: Some(BackendRef("pueue:7".to_string())),
            state: ExternalProcessJobBackendState::Succeeded { exit_code: Some(0) },
            log_refs: Vec::new(),
        });
        assert_eq!(outcome.state, ProcessJobReconciliationState::Exited);
        assert_eq!(outcome.log_state, ProcessJobLogReconciliationState::BackendReferenced);
        assert_eq!(outcome.backend_ref, Some(BackendRef("pueue:7".to_string())));
        assert!(matches!(outcome.status, ProcessJobStatus::Succeeded { exit_code: Some(0) }));
    }

    #[test]
    fn external_reconciliation_facts_roundtrip_preserves_backend_state() {
        let facts = ExternalProcessJobReconciliationFacts {
            id: ProcessJobId("proc".to_string()),
            backend: ProcessJobBackendKind::Pueue,
            expected_backend_ref: BackendRef("pueue:7".to_string()),
            observed_backend_ref: Some(BackendRef("pueue:7".to_string())),
            state: ExternalProcessJobBackendState::Succeeded { exit_code: Some(0) },
            log_refs: Vec::new(),
        };
        let json = serde_json::to_string(&facts).expect("facts should serialize");
        assert!(json.contains("succeeded"));
        let parsed: ExternalProcessJobReconciliationFacts = serde_json::from_str(&json)
            .expect("facts should deserialize");
        assert_eq!(parsed, facts);
    }

    #[test]
    fn native_process_identity_conservatively_verifies_observations() {
        let identity = NativeProcessJobIdentity {
            pid: 42,
            process_group: Some(42),
            start_time_ticks: Some(1000),
            command_fingerprint: Some("cmd".to_string()),
            cwd_fingerprint: Some("cwd".to_string()),
        };
        let matching = NativeProcessJobObservation {
            pid: 42,
            process_group: Some(42),
            start_time_ticks: Some(1000),
            command_fingerprint: Some("cmd".to_string()),
            cwd_fingerprint: Some("cwd".to_string()),
        };
        let reused_pid = NativeProcessJobObservation {
            start_time_ticks: Some(2000),
            ..matching.clone()
        };
        let ambiguous = NativeProcessJobIdentity {
            start_time_ticks: None,
            command_fingerprint: None,
            cwd_fingerprint: None,
            ..identity.clone()
        };

        assert_eq!(identity.verify_observation(Some(&matching)), ProcessJobReconciliationState::ReattachedLogIncomplete);
        assert_eq!(identity.verify_observation(Some(&reused_pid)), ProcessJobReconciliationState::IdentityMismatch);
        assert_eq!(ambiguous.verify_observation(Some(&matching)), ProcessJobReconciliationState::IdentityMismatch);
        assert_eq!(identity.verify_observation(None), ProcessJobReconciliationState::LostAfterRestart);
    }

    #[test]
    fn process_job_status_terminal_and_labels_are_stable() {
        assert!(!ProcessJobStatus::Running.is_terminal());
        assert!(ProcessJobStatus::Succeeded { exit_code: Some(0) }.is_terminal());
        assert_eq!(ProcessJobStatus::Waiting.label(), "waiting");
        assert_eq!(
            ProcessJobStatus::BackendUnavailable {
                reason: "missing".to_string(),
            }
            .label(),
            "backend-unavailable(missing)"
        );
    }

    #[test]
    fn backend_capabilities_advertise_supported_operations() {
        let native = ProcessJobBackendCapabilities::native();
        assert!(native.supports_operation(ProcessJobOperation::Start));
        assert!(native.supports_operation(ProcessJobOperation::WriteStdin));
        assert_eq!(native.unsupported_detail(ProcessJobOperation::Start), None);

        let pueue = ProcessJobBackendCapabilities::pueue();
        assert!(pueue.supports_operation(ProcessJobOperation::Log));
        assert!(!pueue.supports_operation(ProcessJobOperation::WriteStdin));
        assert_eq!(pueue.unsupported_detail(ProcessJobOperation::WriteStdin), Some("stdin requires stdin support"));

        let unavailable = ProcessJobBackendCapabilities::unavailable(ProcessJobBackendKind::Systemd, "missing");
        assert!(!unavailable.supports_operation(ProcessJobOperation::List));
    }

    #[test]
    fn safe_capability_hints_project_non_sensitive_booleans() {
        let hints = ProcessJobSafeCapabilityHints::for_backend(ProcessJobBackendKind::Pueue);
        assert!(hints.supports_kill);
        assert!(hints.supports_restart);
        assert!(!hints.supports_stdin);
        assert!(hints.supports_logs);
        assert!(!hints.supports_resource_limits);
    }

    #[test]
    fn caller_scope_and_capabilities_authorize_by_owner_and_operation() {
        let caller = ProcessJobCallerScope {
            session_id: Some("sess".to_string()),
            capabilities: ProcessJobCapabilitySet::full_control(),
            ..ProcessJobCallerScope::default()
        };
        let owner = ProcessJobOwnerScope::Session("sess".to_string());
        assert!(caller.matches_owner(&owner));
        assert!(caller.can_access(&owner, ProcessJobOperation::Start, ProcessJobBackendKind::Native));
        assert!(!caller.can_access(
            &ProcessJobOwnerScope::Session("other".to_string()),
            ProcessJobOperation::Start,
            ProcessJobBackendKind::Native,
        ));

        let observer = ProcessJobCapabilitySet::observe_only();
        assert!(observer.allows_operation(ProcessJobOperation::List, ProcessJobBackendKind::Native));
        assert!(!observer.allows_operation(ProcessJobOperation::Kill, ProcessJobBackendKind::Native));
    }

    #[test]
    fn cwd_policy_is_plain_backend_neutral_data() {
        assert!(matches!(ProcessJobCwd::Inherited, ProcessJobCwd::Inherited));
        assert!(matches!(
            ProcessJobCwd::Explicit(PathBuf::from("/repo")),
            ProcessJobCwd::Explicit(ref path) if path == &PathBuf::from("/repo")
        ));
    }

    #[test]
    fn retention_class_identifies_active_state() {
        assert!(ProcessJobRetentionClass::Active.protects_active_state());
        assert!(ProcessJobRetentionClass::Adopted.protects_active_state());
        assert!(!ProcessJobRetentionClass::RecentCompleted.protects_active_state());
        assert!(!ProcessJobRetentionClass::Failed.protects_active_state());
    }

    #[test]
    fn log_overflow_policy_classifies_truncation_and_disk_pressure() {
        let overflow = ProcessJobLogOverflowPolicy {
            max_line_bytes: 10,
            max_chunk_bytes: 20,
            max_file_bytes: 30,
            max_total_bytes: 40,
        };

        assert_eq!(overflow.classify_write(8, 20, 30), ProcessJobLogWriteDisposition::Accept);
        assert_eq!(overflow.classify_write(12, 20, 30), ProcessJobLogWriteDisposition::TruncateLine {
            dropped_bytes: 2
        });
        assert_eq!(overflow.classify_write(8, 25, 29), ProcessJobLogWriteDisposition::TruncateChunk {
            dropped_bytes: 5
        });
        assert_eq!(overflow.classify_write(8, 19, 31), ProcessJobLogWriteDisposition::DegradeDiskFull);
        assert_eq!(overflow.classify_write(8, 19, 41), ProcessJobLogWriteDisposition::DegradeDiskFull);
    }

    #[test]
    fn backend_status_contract_preserves_backend_ref_status_and_logs() {
        let updated_at = DateTime::parse_from_rfc3339("2026-05-18T00:00:00Z")
            .expect("timestamp parses")
            .with_timezone(&Utc);
        let status = ProcessJobBackendStatus {
            backend_ref: BackendRef("native:42".to_string()),
            status: ProcessJobStatus::Running,
            updated_at,
            log_refs: vec![ProcessJobLogRef {
                stream: ProcessJobStream::Stdout,
                reference: "native:job/stdout.log".to_string(),
                retained_until: None,
                max_bytes: Some(1024),
            }],
        };
        let json = serde_json::to_string(&status).expect("backend status should serialize");
        assert!(json.contains("native:42"));
        let parsed: ProcessJobBackendStatus = serde_json::from_str(&json).expect("backend status should deserialize");
        assert_eq!(parsed, status);
    }

    #[test]
    fn native_log_layout_sanitizes_references_without_host_io() {
        let layout = NativeProcessJobLogLayout::for_stream(
            ProcessJobId("../job with spaces".to_string()),
            ProcessJobStream::Combined,
        );
        assert_eq!(layout.reference, "native:.._job_with_spaces/combined.log");
        assert_eq!(layout.relative_path, PathBuf::from(".._job_with_spaces").join("combined.log"));

        let log_ref = layout.into_log_ref(1024);
        assert_eq!(log_ref.reference, "native:.._job_with_spaces/combined.log");
        assert_eq!(log_ref.max_bytes, Some(1024));
    }

    #[test]
    fn log_reference_cursor_and_range_are_plain_backend_neutral_data() {
        let reference = ProcessJobLogRef {
            stream: ProcessJobStream::Combined,
            reference: "native:proc/stdout.log".to_string(),
            retained_until: None,
            max_bytes: Some(4096),
        };
        assert_eq!(reference.stream, ProcessJobStream::Combined);
        assert_eq!(reference.max_bytes, Some(4096));

        let cursor = ProcessJobLogCursor {
            stream: ProcessJobStream::Stdout,
            offset: 128,
        };
        assert_eq!(cursor.offset, 128);

        let range = ProcessJobLogRange {
            stream: ProcessJobStream::Stderr,
            offset: Some(64),
            limit_bytes: 1024,
        };
        assert_eq!(range.stream, ProcessJobStream::Stderr);
        assert_eq!(range.offset, Some(64));
        assert_eq!(range.limit_bytes, 1024);
    }

    #[test]
    fn redaction_policy_bounds_and_redacts_sensitive_contract_fields() {
        let redaction = ProcessJobRedactionPolicy {
            max_preview_chars: 12,
            max_excerpt_chars: 16,
            max_metadata_value_chars: 8,
        };
        let mut metadata = BTreeMap::new();
        metadata.insert("profile".to_string(), "verification".to_string());
        metadata.insert("identity.team".to_string(), "runtime".to_string());
        metadata.insert("identity.token".to_string(), "raw-token".to_string());
        metadata.insert("headers.Authorization".to_string(), "Bearer raw".to_string());
        let projected = redaction.safe_identity_metadata(&metadata);

        assert_eq!(redaction.safe_command_preview("cargo nextest run"), "cargo nextes…");
        assert_eq!(redaction.safe_command_preview("Authorization: Bearer raw-token"), PROCESS_JOB_REDACTED);
        assert_eq!(redaction.safe_log_excerpt("ready with password=hunter2"), PROCESS_JOB_REDACTED);
        assert_eq!(projected.get("profile").map(String::as_str), Some("verifica…"));
        assert_eq!(projected.get("identity.team").map(String::as_str), Some("runtime"));
        assert_eq!(projected.get("identity.token").map(String::as_str), Some(PROCESS_JOB_REDACTED));
        assert!(!projected.contains_key("headers.Authorization"));
    }

    #[test]
    fn notification_decision_and_observation_are_backend_neutral_data() {
        let decision = ProcessJobNotificationDecision {
            kind: ProcessJobNotificationKind::Completion,
            summary: "complete".to_string(),
            log_excerpt: Some("done".to_string()),
        };
        assert!(matches!(decision.kind, ProcessJobNotificationKind::Completion));
        assert_eq!(decision.summary, "complete");

        let observation = ProcessJobNotificationObservation {
            status: ProcessJobStatus::Running,
            line: Some("ready".to_string()),
            tick: 4,
        };
        assert!(!observation.status.is_terminal());
        assert_eq!(observation.line.as_deref(), Some("ready"));
        assert_eq!(observation.tick, 4);
    }

    #[test]
    fn notification_policy_bounds_watch_patterns_without_dispatch() {
        let policy = ProcessJobNotificationPolicy {
            notify_on_complete: true,
            watch_patterns: vec![
                "".to_string(),
                " ready ".to_string(),
                "x".repeat(MAX_PROCESS_JOB_WATCH_PATTERN_LEN + 4),
            ],
        };
        let bounded = policy.bounded_watch_patterns();
        assert_eq!(bounded.len(), 2);
        assert_eq!(bounded[0], "ready");
        assert_eq!(bounded[1].chars().count(), MAX_PROCESS_JOB_WATCH_PATTERN_LEN);
        assert!(matches!(ProcessJobNotificationKind::Completion, ProcessJobNotificationKind::Completion));
    }

    #[test]
    fn resource_policy_is_plain_backend_neutral_data() {
        let policy = ProcessJobResourcePolicy {
            timeout: Some(Duration::from_secs(30)),
            memory_max_bytes: Some(1024),
            cpu_quota_percent: Some(50),
            max_log_bytes: Some(4096),
        };

        assert_eq!(policy.timeout, Some(Duration::from_secs(30)));
        assert_eq!(policy.memory_max_bytes, Some(1024));
        assert_eq!(policy.cpu_quota_percent, Some(50));
        assert_eq!(policy.max_log_bytes, Some(4096));
    }

    #[test]
    fn project_process_job_profile_source_precedence_orders_by_specificity() {
        assert!(ProjectProcessJobProfileSourcePrecedence::Global < ProjectProcessJobProfileSourcePrecedence::Workspace);
        assert!(ProjectProcessJobProfileSourcePrecedence::Workspace < ProjectProcessJobProfileSourcePrecedence::Explicit);
        assert_eq!(ProjectProcessJobProfileSourcePrecedence::Explicit.label(), "explicit");
        let json = serde_json::to_string(&ProjectProcessJobProfileSourcePrecedence::Workspace)
            .expect("precedence should serialize");
        assert_eq!(json, r#""workspace""#);
    }

    #[test]
    fn project_profile_resolution_produces_backend_neutral_start_spec() {
        let profiles = ProjectProcessJobProfiles {
            schema_version: PROCESS_JOB_PROFILE_SCHEMA_VERSION,
            profiles: BTreeMap::from([("verify".to_string(), ProjectProcessJobProfile {
                backend: Some(ProcessJobBackendKind::Pueue),
                program: Some("cargo".to_string()),
                args: vec!["nextest".to_string(), "run".to_string()],
                cwd: Some(PathBuf::from("/repo")),
                env: BTreeMap::from([("APP_MODE".to_string(), "ci".to_string())]),
                ..ProjectProcessJobProfile::default()
            })]),
        };
        let policy = ProjectProcessJobProfilePolicy {
            allowed_backends: vec![ProcessJobBackendKind::Native, ProcessJobBackendKind::Pueue],
            allowed_env_prefixes: vec!["APP_".to_string()],
            allowed_cwd_prefixes: vec![PathBuf::from("/repo")],
            policy_source: "test-policy".to_string(),
            ..ProjectProcessJobProfilePolicy::default()
        };

        let resolved = profiles
            .resolve("verify", ProcessJobOwnerScope::Workspace("repo".to_string()), &policy)
            .expect("profile should resolve");

        assert_eq!(resolved.request.backend, ProcessJobBackendKind::Pueue);
        assert_eq!(resolved.request.command_preview, "cargo nextest run");
        assert_eq!(resolved.request.program.as_deref(), Some("cargo"));
        assert_eq!(resolved.request.metadata.get(PROCESS_JOB_PROFILE_METADATA_NAME).map(String::as_str), Some("verify"));
        assert_eq!(resolved.request.metadata.get("env:APP_MODE").map(String::as_str), Some("ci"));
        assert_eq!(resolved.evidence.policy_source, "test-policy");
    }

    #[test]
    fn project_profile_validation_error_message_is_stable() {
        let error = ProjectProcessJobProfileValidationError::new(
            ProjectProcessJobProfileValidationCode::DisallowedBackend,
            "build",
            "uses disallowed backend",
        );
        assert_eq!(
            error.to_string(),
            "process job profile build validation failed (DisallowedBackend): uses disallowed backend"
        );
    }

    #[test]
    fn project_process_job_profile_policy_defaults_to_native_only() {
        let policy = ProjectProcessJobProfilePolicy::default();
        assert_eq!(policy.default_backend, ProcessJobBackendKind::Native);
        assert_eq!(policy.allowed_backends, vec![ProcessJobBackendKind::Native]);
        assert_eq!(policy.policy_source, "default");
        assert!(policy.allowed_env_prefixes.is_empty());
    }

    #[test]
    fn profile_receipt_metadata_projects_from_safe_metadata() {
        let mut metadata = BTreeMap::new();
        metadata.insert(PROCESS_JOB_PROFILE_METADATA_NAME.to_string(), "quick-check".to_string());
        metadata.insert(PROCESS_JOB_PROFILE_METADATA_SCHEMA_VERSION.to_string(), "1".to_string());
        metadata.insert(PROCESS_JOB_PROFILE_METADATA_SOURCE.to_string(), "inline".to_string());
        metadata.insert(PROCESS_JOB_PROFILE_METADATA_POLICY.to_string(), "test-policy".to_string());

        let receipt = ProcessJobProfileReceiptMetadata::from_metadata(&metadata)
            .expect("safe profile receipt metadata should project from metadata");
        assert_eq!(receipt.profile_name, "quick-check");
        assert_eq!(receipt.manifest_schema_version, 1);
        assert_eq!(receipt.profile_source, "inline");
        assert_eq!(receipt.policy_source, "test-policy");
    }
}
