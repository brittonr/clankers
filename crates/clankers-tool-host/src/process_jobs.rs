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
    pub retained_until: Option<DateTime<Utc>>,
    pub max_bytes: Option<u64>,
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
            max_line_bytes: 64 * 1024,
            max_chunk_bytes: 1024 * 1024,
            max_file_bytes: 64 * 1024 * 1024,
            max_total_bytes: 1024 * 1024 * 1024,
        }
    }
}

impl ProcessJobLogOverflowPolicy {
    #[must_use]
    pub fn classify_write(&self, line_bytes: u64, chunk_bytes: u64, total_bytes: u64) -> ProcessJobLogWriteDisposition {
        if line_bytes > self.max_line_bytes {
            ProcessJobLogWriteDisposition::TruncateLine {
                dropped_bytes: line_bytes - self.max_line_bytes,
            }
        } else if chunk_bytes > self.max_chunk_bytes {
            ProcessJobLogWriteDisposition::TruncateChunk {
                dropped_bytes: chunk_bytes - self.max_chunk_bytes,
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
