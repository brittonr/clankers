//! Backend-neutral process/job contracts.
//!
//! These types are the stable seam between the agent-visible `process` tool,
//! service orchestration, storage, log backends, notification delivery, and UI
//! projections. Concrete native, pueue, systemd, redb, TUI, and daemon adapters
//! should depend on these DTOs rather than on each other.

use std::collections::BTreeMap;
use std::collections::BTreeSet;
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

pub const PROCESS_JOB_ID_PREFIX: &str = "proc_b3_";
pub const PROCESS_JOB_IDENTITY_DOMAIN: &str = "clankers.process-job.identity";
pub const PROCESS_JOB_IDENTITY_VERSION: u8 = 1;

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
    #[serde(default)]
    pub metadata: BTreeMap<String, String>,
}

impl ProcessJobIdentityEnvelope {
    #[must_use]
    pub fn for_start_request(request: &StartProcessJobRequest, request_nonce: impl Into<String>) -> Self {
        Self {
            version: PROCESS_JOB_IDENTITY_VERSION,
            domain: PROCESS_JOB_IDENTITY_DOMAIN.to_string(),
            backend: request.backend,
            owner: request.owner.clone(),
            command_preview: request.command_preview.clone(),
            cwd: request.cwd.clone(),
            profile: request.metadata.get("profile").cloned(),
            request_nonce: request_nonce.into(),
            metadata: public_identity_metadata(&request.metadata),
        }
    }

    #[must_use]
    pub fn canonical_bytes(&self) -> Vec<u8> {
        let mut fields = Vec::new();
        fields.push(("version".to_string(), self.version.to_string()));
        fields.push(("domain".to_string(), self.domain.clone()));
        fields.push(("backend".to_string(), self.backend.label().to_string()));
        fields.push(("owner.kind".to_string(), owner_kind(&self.owner).to_string()));
        fields.push(("owner.value".to_string(), owner_value(&self.owner).unwrap_or_default()));
        fields.push(("command_preview".to_string(), self.command_preview.clone()));
        fields.push(("cwd.kind".to_string(), cwd_kind(&self.cwd).to_string()));
        fields.push(("cwd.path".to_string(), cwd_path(&self.cwd).unwrap_or_default()));
        fields.push(("profile".to_string(), self.profile.clone().unwrap_or_default()));
        fields.push(("request_nonce".to_string(), self.request_nonce.clone()));
        for (key, value) in &self.metadata {
            fields.push((format!("metadata.{key}"), value.clone()));
        }
        fields.sort_by(|left, right| left.0.cmp(&right.0));

        let mut canonical = Vec::new();
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

fn public_identity_metadata(metadata: &BTreeMap<String, String>) -> BTreeMap<String, String> {
    metadata
        .iter()
        .filter(|(key, _)| key.starts_with("identity.") || key.as_str() == "profile")
        .map(|(key, value)| (key.clone(), value.clone()))
        .collect()
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
            max_age: Some(Duration::from_secs(14 * 24 * 60 * 60)),
            max_records: Some(1000),
            max_log_bytes: Some(1024 * 1024 * 1024),
        }
    }
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
    pub removed_records: Vec<ProcessJobId>,
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
            removed_records: Vec::new(),
            removed_log_bytes: 0,
            skipped_active_jobs: Vec::new(),
            released_log_refs: Vec::new(),
            failures: Vec::new(),
            summary: "process job GC removed 0 records, 0 log bytes, skipped 0 active jobs, 0 failures".to_string(),
        }
    }

    pub fn refresh_summary(&mut self) {
        self.summary = format!(
            "process job GC removed {} records, {} log bytes, skipped {} active jobs, {} failures",
            self.removed_records.len(),
            self.removed_log_bytes,
            self.skipped_active_jobs.len(),
            self.failures.len()
        );
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

/// Policy bounds for resolving project-defined process/job profiles.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectProcessJobProfilePolicy {
    pub default_backend: ProcessJobBackendKind,
    pub allowed_backends: Vec<ProcessJobBackendKind>,
    pub max_timeout: Option<Duration>,
    pub max_memory_bytes: Option<u64>,
    pub max_cpu_quota_percent: Option<u32>,
    pub allowed_env_prefixes: Vec<String>,
}

impl Default for ProjectProcessJobProfilePolicy {
    fn default() -> Self {
        Self {
            default_backend: ProcessJobBackendKind::Native,
            allowed_backends: vec![ProcessJobBackendKind::Native],
            max_timeout: None,
            max_memory_bytes: None,
            max_cpu_quota_percent: None,
            allowed_env_prefixes: Vec::new(),
        }
    }
}

/// Named project process/job profile collection. Parsing this type is pure and
/// never dispatches to native, pueue, systemd, or storage adapters.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct ProjectProcessJobProfiles {
    #[serde(default)]
    pub profiles: BTreeMap<String, ProjectProcessJobProfile>,
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
}

impl ProjectProcessJobProfiles {
    pub fn from_json_str(input: &str) -> Result<Self, RuntimeError> {
        serde_json::from_str(input)
            .map_err(|err| RuntimeError::InvalidTool(format!("invalid process job profiles config: {err}")))
    }

    pub fn resolve(
        &self,
        name: &str,
        owner: ProcessJobOwnerScope,
        policy: &ProjectProcessJobProfilePolicy,
    ) -> Result<ProjectProcessJobProfileResolution, RuntimeError> {
        let profile = self
            .profiles
            .get(name)
            .ok_or_else(|| RuntimeError::InvalidTool(format!("unknown process job profile: {name}")))?;
        profile.resolve_named(name, owner, policy)
    }
}

impl ProjectProcessJobProfile {
    fn resolve_named(
        &self,
        name: &str,
        owner: ProcessJobOwnerScope,
        policy: &ProjectProcessJobProfilePolicy,
    ) -> Result<ProjectProcessJobProfileResolution, RuntimeError> {
        let backend = self.backend.unwrap_or(policy.default_backend);
        validate_profile_backend(name, backend, policy)?;
        validate_profile_command_shape(name, self)?;
        validate_profile_environment(name, &self.env, policy)?;
        validate_profile_resources(name, &self.resource_policy, policy)?;

        let cwd = self.cwd.clone().map_or(ProcessJobCwd::Inherited, ProcessJobCwd::Explicit);
        let mut metadata = self.metadata.clone();
        metadata.insert("profile".to_string(), name.to_string());
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
        })
    }
}

fn validate_profile_backend(
    name: &str,
    backend: ProcessJobBackendKind,
    policy: &ProjectProcessJobProfilePolicy,
) -> Result<(), RuntimeError> {
    if backend == ProcessJobBackendKind::Unknown || !policy.allowed_backends.contains(&backend) {
        return Err(RuntimeError::InvalidTool(format!(
            "process job profile {name} uses disallowed backend {backend:?}"
        )));
    }
    Ok(())
}

fn validate_profile_command_shape(name: &str, profile: &ProjectProcessJobProfile) -> Result<(), RuntimeError> {
    let has_command = profile.command.as_ref().is_some_and(|value| !value.trim().is_empty());
    let has_program = profile.program.as_ref().is_some_and(|value| !value.trim().is_empty());
    if has_command == has_program {
        return Err(RuntimeError::InvalidTool(format!(
            "process job profile {name} must set exactly one of command or program"
        )));
    }
    if !has_program && !profile.args.is_empty() {
        return Err(RuntimeError::InvalidTool(format!("process job profile {name} cannot set args without program")));
    }
    Ok(())
}

fn validate_profile_environment(
    name: &str,
    env: &BTreeMap<String, String>,
    policy: &ProjectProcessJobProfilePolicy,
) -> Result<(), RuntimeError> {
    for key in env.keys() {
        let allowed = key.chars().all(|ch| ch.is_ascii_uppercase() || ch.is_ascii_digit() || ch == '_')
            && !key.contains("SECRET")
            && !key.contains("TOKEN")
            && !key.contains("KEY")
            && policy.allowed_env_prefixes.iter().any(|prefix| key.starts_with(prefix));
        if !allowed {
            return Err(RuntimeError::InvalidTool(format!(
                "process job profile {name} has disallowed environment key {key}"
            )));
        }
    }
    Ok(())
}

fn validate_profile_resources(
    name: &str,
    resources: &ProcessJobResourcePolicy,
    policy: &ProjectProcessJobProfilePolicy,
) -> Result<(), RuntimeError> {
    if let (Some(actual), Some(maximum)) = (resources.timeout, policy.max_timeout) {
        if actual > maximum {
            return Err(RuntimeError::InvalidTool(format!("process job profile {name} timeout exceeds policy")));
        }
    }
    if let (Some(actual), Some(maximum)) = (resources.memory_max_bytes, policy.max_memory_bytes) {
        if actual > maximum {
            return Err(RuntimeError::InvalidTool(format!("process job profile {name} memory exceeds policy")));
        }
    }
    if let (Some(actual), Some(maximum)) = (resources.cpu_quota_percent, policy.max_cpu_quota_percent) {
        if actual > maximum {
            return Err(RuntimeError::InvalidTool(format!("process job profile {name} cpu quota exceeds policy")));
        }
    }
    Ok(())
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

impl ProcessJobBackendKind {
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::Native => "native",
            Self::Pueue => "pueue",
            Self::Systemd => "systemd",
            Self::Unknown => "unknown",
        }
    }
}

impl ProcessJobStatus {
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
    pub lifecycle: ProcessJobLifecycleBucket,
    pub status: ProcessJobStatus,
    pub status_label: String,
    pub command_preview: String,
    pub cwd: ProcessJobCwd,
    pub started_at: Option<DateTime<Utc>>,
    pub updated_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
    pub log_refs: Vec<ProcessJobLogRef>,
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
            lifecycle: lifecycle.clone(),
            status_label: summary.status.label(),
            status: summary.status,
            command_preview: summary.command_preview,
            cwd: summary.cwd,
            started_at: summary.started_at,
            updated_at: summary.updated_at,
            completed_at: summary.completed_at,
            log_refs: summary.log_refs,
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

/// Fields every process/job receipt surface carries, independent of operation payload.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProcessJobReceiptCommon {
    pub operation: ProcessJobOperation,
    pub id: Option<ProcessJobId>,
    pub backend: Option<ProcessJobBackendKind>,
    pub status: Option<ProcessJobStatus>,
    pub backend_ref: Option<BackendRef>,
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
    pub summary: String,
    pub error: Option<ProcessJobError>,
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
            summary: reason.clone(),
            error: Some(ProcessJobError {
                code: ProcessJobErrorCode::BackendUnavailable,
                operation,
                id: None,
                backend: Some(backend),
                action: Some(operation.action_name().to_string()),
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
                    summary: receipt.summary.clone(),
                    error: None,
                },
                payload: ProcessJobReceiptPayload::GarbageCollect { receipt },
            },
        }
    }
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
    let events = store.list_notifications(caller, after).await?;
    Ok(deduplicate_notification_events(events))
}

fn deduplicate_notification_events(events: Vec<ProcessJobNotificationEvent>) -> Vec<ProcessJobNotificationEvent> {
    let mut seen = BTreeSet::new();
    events.into_iter().filter(|event| seen.insert(event.event_id.clone())).collect()
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

    fn profile_policy() -> ProjectProcessJobProfilePolicy {
        ProjectProcessJobProfilePolicy {
            default_backend: ProcessJobBackendKind::Native,
            allowed_backends: vec![ProcessJobBackendKind::Native, ProcessJobBackendKind::Pueue],
            max_timeout: Some(Duration::from_secs(600)),
            max_memory_bytes: Some(1024 * 1024 * 1024),
            max_cpu_quota_percent: Some(100),
            allowed_env_prefixes: vec!["APP_".to_string()],
        }
    }

    #[test]
    fn blake3_process_job_identity_fixture_is_canonical_and_backend_ref_free() {
        let request = StartProcessJobRequest {
            backend: ProcessJobBackendKind::Native,
            command_preview: "cargo nextest run".to_string(),
            program: Some("cargo".to_string()),
            args: vec!["nextest".to_string(), "run".to_string()],
            shell_command: None,
            cwd: ProcessJobCwd::Explicit(PathBuf::from("/repo")),
            owner: ProcessJobOwnerScope::Workspace("repo".to_string()),
            resource_policy: ProcessJobResourcePolicy::default(),
            notification_policy: ProcessJobNotificationPolicy::default(),
            metadata: BTreeMap::from([
                ("profile".to_string(), "verify".to_string()),
                ("identity.intent".to_string(), "ci".to_string()),
                ("env:APP_SECRET".to_string(), "must-not-enter-id".to_string()),
            ]),
        };
        let envelope = ProcessJobIdentityEnvelope::for_start_request(&request, "native:42");
        let canonical = String::from_utf8(envelope.canonical_bytes()).expect("canonical bytes are utf8 fixture");

        assert!(canonical.starts_with("clankers-process-job-identity-v1\n"));
        assert!(canonical.contains("7:backend=6:native\n"));
        assert!(canonical.contains("8:cwd.path=5:/repo\n"));
        assert!(canonical.contains("16:metadata.profile=6:verify\n"));
        assert!(canonical.contains("24:metadata.identity.intent=2:ci\n"));
        assert!(!canonical.contains("pid:"));
        assert!(!canonical.contains("pueue:"));
        assert!(!canonical.contains("systemd:"));
        assert!(!canonical.contains("must-not-enter-id"));

        let id = envelope.derive_id();
        assert_eq!(id.0, "proc_b3_115e5d8781a631cd008255939c0446e4d96d6661b5435a093a534672c17b4f40");
        assert!(id.is_blake3_native());
        assert!(!ProcessJobId::legacy("proc_1").is_blake3_native());
    }

    #[test]
    fn process_job_tool_request_maps_to_operation_vocabulary() {
        let request = ProcessJobToolRequest::WriteStdin(WriteProcessJobStdinRequest {
            id: ProcessJobId::legacy("proc_1"),
            data: b"hello".to_vec(),
            newline: true,
        });

        assert_eq!(request.operation(), ProcessJobOperation::WriteStdin);
    }

    #[test]
    fn process_job_tool_receipt_envelope_keeps_common_fields_and_payloads_separate() {
        let receipt = ProcessJobReceipt {
            operation: ProcessJobOperation::Start,
            id: Some(ProcessJobId::legacy("proc_1")),
            backend: Some(ProcessJobBackendKind::Native),
            status: Some(ProcessJobStatus::Running),
            backend_ref: Some(BackendRef("pid:123".to_string())),
            log_refs: vec![ProcessJobLogRef {
                stream: ProcessJobStream::Combined,
                reference: "native:proc_1/combined.log".to_string(),
                retained_until: None,
                max_bytes: Some(1024),
            }],
            summary: "started".to_string(),
            error: None,
        };

        let envelope = ProcessJobToolResult::Start(receipt).into_receipt();
        assert_eq!(envelope.common.operation, ProcessJobOperation::Start);
        assert_eq!(envelope.common.backend_ref, Some(BackendRef("pid:123".to_string())));
        match envelope.payload {
            ProcessJobReceiptPayload::State { log_refs } => {
                assert_eq!(log_refs.len(), 1);
                assert_eq!(log_refs[0].reference, "native:proc_1/combined.log");
            }
            other => panic!("unexpected payload: {other:?}"),
        }

        let serialized = serde_json::to_value(ProcessJobToolResult::List(Vec::new()).into_receipt())
            .expect("receipt envelope serializes");
        assert_eq!(serialized["common"]["operation"], "list");
        assert_eq!(serialized["payload"]["kind"], "list");
        assert!(serialized["payload"]["data"]["jobs"].as_array().is_some());
    }

    #[test]
    fn project_job_profile_resolves_to_backend_neutral_start_spec() {
        let profiles = ProjectProcessJobProfiles::from_json_str(
            r#"{
              "profiles": {
                "verify": {
                  "backend": "pueue",
                  "program": "cargo",
                  "args": ["nextest", "run"],
                  "cwd": "/repo",
                  "env": {"APP_MODE": "ci"},
                  "notification_policy": {"notify_on_complete": true},
                  "metadata": {"intent": "verify"}
                }
              }
            }"#,
        )
        .expect("profile config parses");

        let resolved = profiles
            .resolve("verify", ProcessJobOwnerScope::Workspace("repo".to_string()), &profile_policy())
            .expect("profile resolves");

        assert_eq!(resolved.name, "verify");
        assert_eq!(resolved.request.backend, ProcessJobBackendKind::Pueue);
        assert_eq!(resolved.request.program.as_deref(), Some("cargo"));
        assert_eq!(resolved.request.args, vec!["nextest", "run"]);
        assert_eq!(resolved.request.shell_command, None);
        assert_eq!(resolved.request.command_preview, "cargo nextest run");
        assert!(matches!(resolved.request.cwd, ProcessJobCwd::Explicit(ref path) if path == &PathBuf::from("/repo")));
        assert_eq!(resolved.request.metadata.get("profile").map(String::as_str), Some("verify"));
        assert_eq!(resolved.request.metadata.get("env:APP_MODE").map(String::as_str), Some("ci"));
        assert!(resolved.request.notification_policy.notify_on_complete);
    }

    #[test]
    fn project_job_profile_rejects_invalid_config_before_backend_dispatch() {
        let mut profiles = ProjectProcessJobProfiles::default();
        profiles.profiles.insert("bad".to_string(), ProjectProcessJobProfile {
            backend: Some(ProcessJobBackendKind::Systemd),
            command: Some("run secret thing".to_string()),
            env: BTreeMap::from([("APP_SECRET".to_string(), "nope".to_string())]),
            resource_policy: ProcessJobResourcePolicy {
                timeout: Some(Duration::from_secs(1200)),
                ..ProcessJobResourcePolicy::default()
            },
            ..ProjectProcessJobProfile::default()
        });

        let err = profiles
            .resolve("bad", ProcessJobOwnerScope::Workspace("repo".to_string()), &profile_policy())
            .expect_err("invalid profile rejects before dispatch");
        assert!(err.to_string().contains("disallowed backend"));
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
    async fn persisted_completion_events_replay_independently_for_multiple_reattached_clients() {
        let store = FakeStore::default();
        let owner = ProcessJobOwnerScope::Session("sess".to_string());
        let first = notification_event("evt_notify_1", owner.clone());
        let second = notification_event("evt_notify_2", owner);
        store.record_notification(first.clone()).await.expect("first notification persists");
        store.record_notification(second.clone()).await.expect("second notification persists");

        let client_a = ProcessJobCallerScope {
            session_id: Some("sess".to_string()),
            capabilities: ProcessJobCapabilitySet::observe_only(),
            ..ProcessJobCallerScope::default()
        };
        let client_b = client_a.clone();

        let replay_a = replay_authorized_notifications(&store, client_a, None)
            .await
            .expect("first reattached client replays notifications");
        let replay_b = replay_authorized_notifications(&store, client_b, Some(first.event_id.clone()))
            .await
            .expect("second reattached client uses its own event cursor");

        assert_eq!(replay_a.iter().map(|event| event.event_id.clone()).collect::<Vec<_>>(), vec![
            first.event_id.clone(),
            second.event_id.clone(),
        ]);
        assert_eq!(replay_b, vec![second]);
    }

    #[tokio::test]
    async fn replay_deduplicates_persisted_completion_events_by_event_id() {
        let store = FakeStore::default();
        let event = notification_event("evt_notify_dedupe", ProcessJobOwnerScope::DaemonGlobal);
        store.record_notification(event.clone()).await.expect("original notification persists");
        store.record_notification(event.clone()).await.expect("duplicate notification persists");

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
        .expect("deduplicated replay succeeds");

        assert_eq!(replayed, vec![event]);
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

        let unavailable = ProcessJobReceipt::backend_unavailable(
            ProcessJobOperation::GarbageCollect,
            ProcessJobBackendKind::Systemd,
            "systemd not enabled",
        );
        let unsupported = ProcessJobReceipt::unsupported(
            ProcessJobOperation::Restart,
            Some(ProcessJobId("proc_1".to_string())),
            ProcessJobBackendKind::Pueue,
            "restart",
            "restart unsupported",
        );

        let unavailable_error = unavailable.error.expect("backend unavailable");
        assert_eq!(unavailable_error.code, ProcessJobErrorCode::BackendUnavailable);
        assert_eq!(unavailable_error.action.as_deref(), Some("garbage_collect"));
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
    fn process_job_tool_request_serialization_golden_fixtures() {
        let id = ProcessJobId("proc_b3_115e5d8781a631cd008255939c0446e4d96d6661b5435a093a534672c17b4f40".to_string());
        let mut metadata = BTreeMap::new();
        metadata.insert("purpose".to_string(), "golden".to_string());
        let start = ProcessJobToolRequest::Start(StartProcessJobRequest {
            backend: ProcessJobBackendKind::Native,
            command_preview: "printf ok".to_string(),
            program: Some("printf".to_string()),
            args: vec!["ok".to_string()],
            shell_command: None,
            cwd: ProcessJobCwd::Inherited,
            owner: ProcessJobOwnerScope::Session("sess-golden".to_string()),
            resource_policy: ProcessJobResourcePolicy {
                timeout: None,
                memory_max_bytes: Some(268_435_456),
                cpu_quota_percent: Some(50),
                max_log_bytes: Some(4096),
            },
            notification_policy: ProcessJobNotificationPolicy {
                notify_on_complete: true,
                watch_patterns: vec!["READY".to_string()],
            },
            metadata,
        });
        let log = ProcessJobToolRequest::Log(ReadProcessJobLogRequest {
            id: id.clone(),
            range: ProcessJobLogRange {
                stream: ProcessJobStream::Combined,
                offset: Some(7),
                limit_bytes: 1024,
            },
        });
        let write = ProcessJobToolRequest::WriteStdin(WriteProcessJobStdinRequest {
            id: id.clone(),
            data: b"hello".to_vec(),
            newline: true,
        });
        let gc = ProcessJobToolRequest::GarbageCollect(GarbageCollectProcessJobsRequest {
            filter: ProcessJobFilter {
                owner: Some(ProcessJobOwnerScope::DaemonGlobal),
                backend: Some(ProcessJobBackendKind::Pueue),
                include_terminal: true,
            },
        });

        let cases = [
            (
                start,
                serde_json::json!({
                    "action": "start",
                    "request": {
                        "backend": "native",
                        "command_preview": "printf ok",
                        "program": "printf",
                        "args": ["ok"],
                        "shell_command": null,
                        "cwd": {"kind": "inherited"},
                        "owner": {"kind": "session", "value": "sess-golden"},
                        "resource_policy": {
                            "timeout": null,
                            "memory_max_bytes": 268435456,
                            "cpu_quota_percent": 50,
                            "max_log_bytes": 4096
                        },
                        "notification_policy": {
                            "notify_on_complete": true,
                            "watch_patterns": ["READY"]
                        },
                        "metadata": {"purpose": "golden"}
                    }
                }),
            ),
            (
                log,
                serde_json::json!({
                    "action": "log",
                    "request": {
                        "id": "proc_b3_115e5d8781a631cd008255939c0446e4d96d6661b5435a093a534672c17b4f40",
                        "range": {"stream": "combined", "offset": 7, "limit_bytes": 1024}
                    }
                }),
            ),
            (
                write,
                serde_json::json!({
                    "action": "write_stdin",
                    "request": {
                        "id": "proc_b3_115e5d8781a631cd008255939c0446e4d96d6661b5435a093a534672c17b4f40",
                        "data": [104, 101, 108, 108, 111],
                        "newline": true
                    }
                }),
            ),
            (
                gc,
                serde_json::json!({
                    "action": "garbage_collect",
                    "request": {
                        "filter": {
                            "owner": {"kind": "daemon_global"},
                            "backend": "pueue",
                            "include_terminal": true
                        }
                    }
                }),
            ),
        ];

        for (request, expected) in cases {
            let actual = serde_json::to_value(&request).expect("request serializes");
            assert_eq!(actual, expected);
            let roundtrip: ProcessJobToolRequest = serde_json::from_value(actual).expect("request deserializes");
            assert_eq!(roundtrip, request);
        }
    }

    #[test]
    fn process_job_tool_receipt_serialization_golden_fixtures() {
        let id = ProcessJobId("proc_b3_115e5d8781a631cd008255939c0446e4d96d6661b5435a093a534672c17b4f40".to_string());
        let log_ref = ProcessJobLogRef {
            stream: ProcessJobStream::Combined,
            reference: "native:proc_b3_115/combined.log".to_string(),
            retained_until: None,
            max_bytes: Some(4096),
        };
        let start_receipt = ProcessJobToolResult::Start(ProcessJobReceipt {
            operation: ProcessJobOperation::Start,
            id: Some(id.clone()),
            backend: Some(ProcessJobBackendKind::Native),
            status: Some(ProcessJobStatus::Running),
            backend_ref: Some(BackendRef("pid:123".to_string())),
            log_refs: vec![log_ref.clone()],
            summary: "started process job".to_string(),
            error: None,
        })
        .into_receipt();
        let log_receipt = ProcessJobToolResult::Log(ProcessJobLogChunk {
            id: id.clone(),
            backend: ProcessJobBackendKind::Native,
            stream: ProcessJobStream::Combined,
            cursor: ProcessJobLogCursor {
                stream: ProcessJobStream::Combined,
                offset: 0,
            },
            next_cursor: Some(ProcessJobLogCursor {
                stream: ProcessJobStream::Combined,
                offset: 2,
            }),
            text: "ok".to_string(),
            truncated: false,
        })
        .into_receipt();
        let error_receipt = ProcessJobReceipt::unsupported(
            ProcessJobOperation::WriteStdin,
            Some(id),
            ProcessJobBackendKind::Pueue,
            "write_stdin",
            "stdin is not supported by pueue backend",
        )
        .into_tool_receipt();

        let cases = [
            (
                start_receipt,
                serde_json::json!({
                    "common": {
                        "operation": "start",
                        "id": "proc_b3_115e5d8781a631cd008255939c0446e4d96d6661b5435a093a534672c17b4f40",
                        "backend": "native",
                        "status": {"state": "running"},
                        "backend_ref": "pid:123",
                        "summary": "started process job",
                        "error": null
                    },
                    "payload": {
                        "kind": "state",
                        "data": {
                            "log_refs": [{
                                "stream": "combined",
                                "reference": "native:proc_b3_115/combined.log",
                                "retained_until": null,
                                "max_bytes": 4096
                            }]
                        }
                    }
                }),
            ),
            (
                log_receipt,
                serde_json::json!({
                    "common": {
                        "operation": "log",
                        "id": "proc_b3_115e5d8781a631cd008255939c0446e4d96d6661b5435a093a534672c17b4f40",
                        "backend": null,
                        "status": null,
                        "backend_ref": null,
                        "summary": "Read 2 bytes of process job log",
                        "error": null
                    },
                    "payload": {
                        "kind": "log",
                        "data": {
                            "chunk": {
                                "id": "proc_b3_115e5d8781a631cd008255939c0446e4d96d6661b5435a093a534672c17b4f40",
                                "backend": "native",
                                "stream": "combined",
                                "cursor": {"stream": "combined", "offset": 0},
                                "next_cursor": {"stream": "combined", "offset": 2},
                                "text": "ok",
                                "truncated": false
                            }
                        }
                    }
                }),
            ),
            (
                error_receipt,
                serde_json::json!({
                    "common": {
                        "operation": "write_stdin",
                        "id": "proc_b3_115e5d8781a631cd008255939c0446e4d96d6661b5435a093a534672c17b4f40",
                        "backend": "pueue",
                        "status": null,
                        "backend_ref": null,
                        "summary": "stdin is not supported by pueue backend",
                        "error": {
                            "code": "unsupported_action_for_backend",
                            "operation": "write_stdin",
                            "id": "proc_b3_115e5d8781a631cd008255939c0446e4d96d6661b5435a093a534672c17b4f40",
                            "backend": "pueue",
                            "action": "write_stdin",
                            "message": "stdin is not supported by pueue backend"
                        }
                    },
                    "payload": {"kind": "state", "data": {"log_refs": []}}
                }),
            ),
        ];

        for (receipt, expected) in cases {
            let actual = serde_json::to_value(&receipt).expect("receipt serializes");
            assert_eq!(actual, expected);
            let roundtrip: ProcessJobToolReceipt = serde_json::from_value(actual).expect("receipt deserializes");
            assert_eq!(roundtrip, receipt);
        }
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
            ProcessJobToolResult::Adopt(receipt),
            ProcessJobToolResult::GarbageCollect(ProcessJobGarbageCollectionReceipt::empty()),
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
    #[test]
    fn process_job_projection_unifies_backends_and_bounds_active_completed_views() {
        let base = Utc::now();
        let summaries = vec![
            ProcessJobSummary {
                id: ProcessJobId("native_active".to_string()),
                backend: ProcessJobBackendKind::Native,
                backend_ref: Some(BackendRef("pid:101".to_string())),
                owner: ProcessJobOwnerScope::DaemonGlobal,
                status: ProcessJobStatus::Running,
                command_preview: "native watcher".to_string(),
                cwd: ProcessJobCwd::Inherited,
                started_at: Some(base),
                updated_at: base,
                completed_at: None,
                log_refs: Vec::new(),
            },
            ProcessJobSummary {
                id: ProcessJobId("pueue_done".to_string()),
                backend: ProcessJobBackendKind::Pueue,
                backend_ref: Some(BackendRef("pueue:7".to_string())),
                owner: ProcessJobOwnerScope::DaemonGlobal,
                status: ProcessJobStatus::Succeeded { exit_code: Some(0) },
                command_preview: "pueue build".to_string(),
                cwd: ProcessJobCwd::Inherited,
                started_at: Some(base),
                updated_at: base + chrono::Duration::seconds(1),
                completed_at: Some(base + chrono::Duration::seconds(1)),
                log_refs: Vec::new(),
            },
            ProcessJobSummary {
                id: ProcessJobId("systemd_active".to_string()),
                backend: ProcessJobBackendKind::Systemd,
                backend_ref: Some(BackendRef("systemd:clankers-job.scope".to_string())),
                owner: ProcessJobOwnerScope::DaemonGlobal,
                status: ProcessJobStatus::Waiting,
                command_preview: "systemd run".to_string(),
                cwd: ProcessJobCwd::Inherited,
                started_at: Some(base),
                updated_at: base + chrono::Duration::seconds(2),
                completed_at: None,
                log_refs: Vec::new(),
            },
        ];

        let projection = project_process_job_list(summaries, ProcessJobProjectionBounds {
            max_active: 1,
            max_completed: 8,
        });

        assert_eq!(projection.total_active, 2);
        assert_eq!(projection.total_completed, 1);
        assert!(projection.truncated_active);
        assert!(!projection.truncated_completed);
        assert_eq!(projection.active[0].id.0, "systemd_active");
        assert_eq!(projection.active[0].backend_label, "systemd");
        assert_eq!(projection.completed[0].backend_label, "pueue");
        assert_eq!(projection.completed[0].status_label, "succeeded(0)");
    }
}
