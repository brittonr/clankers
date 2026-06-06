//! Backend-neutral process/job contracts shared by tool adapters.
//!
//! This module owns pure process-job request/decision DTOs that do not depend
//! on the Clankers runtime facade, daemon actors, TUI state, procmon handles,
//! filesystem storage, or backend command execution.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::time::Duration;

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
