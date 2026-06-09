//! Backend-neutral process/job contracts.
//!
//! These types are the stable seam between the agent-visible `process` tool,
//! service orchestration, storage, log backends, notification delivery, and UI
//! projections. Concrete native, pueue, systemd, redb, TUI, and daemon adapters
//! should depend on these DTOs rather than on each other.

#[cfg(test)]
use std::collections::BTreeMap;
use std::collections::BTreeSet;
#[cfg(test)]
use std::path::PathBuf;
use std::time::Duration;

use async_trait::async_trait;
use chrono::DateTime;
use chrono::Utc;
pub use clankers_tool_host::process_jobs::AdoptProcessJobRequest;
pub use clankers_tool_host::process_jobs::BackendCapabilities;
pub use clankers_tool_host::process_jobs::BackendRef;
pub use clankers_tool_host::process_jobs::ExternalProcessJobBackendState;
pub use clankers_tool_host::process_jobs::ExternalProcessJobReconciliationFacts;
pub use clankers_tool_host::process_jobs::GarbageCollectProcessJobsRequest;
pub use clankers_tool_host::process_jobs::ListProcessJobsRequest;
pub use clankers_tool_host::process_jobs::MAX_PROCESS_JOB_WATCH_PATTERN_LEN;
pub use clankers_tool_host::process_jobs::MAX_PROCESS_JOB_WATCH_PATTERNS;
pub use clankers_tool_host::process_jobs::MutateProcessJobRequest;
pub use clankers_tool_host::process_jobs::NativeProcessJobIdentity;
pub use clankers_tool_host::process_jobs::NativeProcessJobLogLayout;
pub use clankers_tool_host::process_jobs::NativeProcessJobObservation;
pub use clankers_tool_host::process_jobs::PROCESS_JOB_ID_PREFIX;
pub use clankers_tool_host::process_jobs::PROCESS_JOB_IDENTITY_DOMAIN;
pub use clankers_tool_host::process_jobs::PROCESS_JOB_IDENTITY_VERSION;
pub use clankers_tool_host::process_jobs::PROCESS_JOB_MAX_SAFE_EXCERPT_CHARS;
pub use clankers_tool_host::process_jobs::PROCESS_JOB_MAX_SAFE_METADATA_VALUE_CHARS;
pub use clankers_tool_host::process_jobs::PROCESS_JOB_MAX_SAFE_PREVIEW_CHARS;
pub use clankers_tool_host::process_jobs::PROCESS_JOB_PROFILE_METADATA_NAME;
pub use clankers_tool_host::process_jobs::PROCESS_JOB_PROFILE_METADATA_POLICY;
pub use clankers_tool_host::process_jobs::PROCESS_JOB_PROFILE_METADATA_SCHEMA_VERSION;
pub use clankers_tool_host::process_jobs::PROCESS_JOB_PROFILE_METADATA_SOURCE;
pub use clankers_tool_host::process_jobs::PROCESS_JOB_PROFILE_SCHEMA_VERSION;
pub use clankers_tool_host::process_jobs::PROCESS_JOB_REDACTED;
pub use clankers_tool_host::process_jobs::PROCESS_JOB_WATCH_RATE_LIMIT_TICKS;
pub use clankers_tool_host::process_jobs::PROCESS_JOB_WATCH_SUPPRESSION_LIMIT;
pub use clankers_tool_host::process_jobs::PollProcessJobRequest;
pub use clankers_tool_host::process_jobs::ProcessJobBackendCapabilities;
pub use clankers_tool_host::process_jobs::ProcessJobBackendKind;
pub use clankers_tool_host::process_jobs::ProcessJobBackendStart;
pub use clankers_tool_host::process_jobs::ProcessJobBackendStatus;
pub use clankers_tool_host::process_jobs::ProcessJobCallerScope;
pub use clankers_tool_host::process_jobs::ProcessJobCapabilitySet;
pub use clankers_tool_host::process_jobs::ProcessJobCwd;
pub use clankers_tool_host::process_jobs::ProcessJobError;
pub use clankers_tool_host::process_jobs::ProcessJobErrorCode;
pub use clankers_tool_host::process_jobs::ProcessJobEventId;
pub use clankers_tool_host::process_jobs::ProcessJobFilter;
pub use clankers_tool_host::process_jobs::ProcessJobGarbageCollectionFailure;
pub use clankers_tool_host::process_jobs::ProcessJobGarbageCollectionReceipt;
pub use clankers_tool_host::process_jobs::ProcessJobId;
pub use clankers_tool_host::process_jobs::ProcessJobIdentityEnvelope;
pub use clankers_tool_host::process_jobs::ProcessJobLogChunk;
pub use clankers_tool_host::process_jobs::ProcessJobLogCursor;
pub use clankers_tool_host::process_jobs::ProcessJobLogOverflowPolicy;
pub use clankers_tool_host::process_jobs::ProcessJobLogRange;
pub use clankers_tool_host::process_jobs::ProcessJobLogRef;
pub use clankers_tool_host::process_jobs::ProcessJobLifecycleBucket;
pub use clankers_tool_host::process_jobs::ProcessJobListProjection;
pub use clankers_tool_host::process_jobs::ProcessJobLogReconciliationState;
pub use clankers_tool_host::process_jobs::ProcessJobLogWriteDisposition;
pub use clankers_tool_host::process_jobs::ProcessJobNativeAdmissionDecision;
pub use clankers_tool_host::process_jobs::ProcessJobNativeAdmissionInput;
pub use clankers_tool_host::process_jobs::ProcessJobNotificationDecision;
pub use clankers_tool_host::process_jobs::ProcessJobNotificationEvent;
pub use clankers_tool_host::process_jobs::ProcessJobNotificationKind;
pub use clankers_tool_host::process_jobs::ProcessJobNotificationObservation;
pub use clankers_tool_host::process_jobs::ProcessJobNotificationPolicy;
pub use clankers_tool_host::process_jobs::ProcessJobNotificationRedactionTarget;
pub use clankers_tool_host::process_jobs::ProcessJobOperation;
pub use clankers_tool_host::process_jobs::ProcessJobOwnerScope;
pub use clankers_tool_host::process_jobs::ProcessJobProfileReceiptMetadata;
pub use clankers_tool_host::process_jobs::ProjectProcessJobProfile;
pub use clankers_tool_host::process_jobs::ProjectProcessJobProfileManifestSource;
pub use clankers_tool_host::process_jobs::ProjectProcessJobProfilePolicy;
pub use clankers_tool_host::process_jobs::ProjectProcessJobProfileResolution;
pub use clankers_tool_host::process_jobs::ProjectProcessJobProfileResolutionEvidence;
pub use clankers_tool_host::process_jobs::ProjectProcessJobProfileSourcePrecedence;
pub use clankers_tool_host::process_jobs::ProjectProcessJobProfiles;
pub use clankers_tool_host::process_jobs::ProjectProcessJobProfileValidationCode;
pub use clankers_tool_host::process_jobs::ProjectProcessJobProfileValidationError;
pub use clankers_tool_host::process_jobs::ProcessJobProjectionBounds;
pub use clankers_tool_host::process_jobs::ProcessJobProjectionItem;
pub use clankers_tool_host::process_jobs::ProcessJobReconciliationOutcome;
pub use clankers_tool_host::process_jobs::ProcessJobReceipt;
pub use clankers_tool_host::process_jobs::ProcessJobReceiptCommon;
pub use clankers_tool_host::process_jobs::ProcessJobReceiptPayload;
pub use clankers_tool_host::process_jobs::ProcessJobReconciliationReport;
pub use clankers_tool_host::process_jobs::ProcessJobReconciliationState;
pub use clankers_tool_host::process_jobs::ProcessJobRedactionPolicy;
pub use clankers_tool_host::process_jobs::ProcessJobReleasedLogRef;
pub use clankers_tool_host::process_jobs::ProcessJobResourcePolicy;
pub use clankers_tool_host::process_jobs::ProcessJobRetentionClass;
pub use clankers_tool_host::process_jobs::ProcessJobRetentionEligibility;
pub use clankers_tool_host::process_jobs::ProcessJobRetentionMetadata;
pub use clankers_tool_host::process_jobs::ProcessJobRetentionPolicy;
pub use clankers_tool_host::process_jobs::ProcessJobSafeCapabilityHints;
pub use clankers_tool_host::process_jobs::ProcessJobSpec;
pub use clankers_tool_host::process_jobs::ProcessJobStatus;
pub use clankers_tool_host::process_jobs::ProcessJobStream;
pub use clankers_tool_host::process_jobs::ProcessJobSummary;
pub use clankers_tool_host::process_jobs::ProcessJobTimestamp;
pub use clankers_tool_host::process_jobs::ProcessJobToolReceipt;
pub use clankers_tool_host::process_jobs::ProcessJobToolRequest;
pub use clankers_tool_host::process_jobs::ProcessJobToolResult;
pub use clankers_tool_host::process_jobs::ProcessJobUnsupportedDetail;
pub use clankers_tool_host::process_jobs::ReadProcessJobLogRequest;
pub use clankers_tool_host::process_jobs::StartProcessJobProfileRequest;
pub use clankers_tool_host::process_jobs::StartProcessJobRequest;
pub use clankers_tool_host::process_jobs::WaitProcessJobRequest;
pub use clankers_tool_host::process_jobs::WriteProcessJobStdinRequest;
pub use clankers_tool_host::process_jobs::native_process_job_admission_decision;
pub use clankers_tool_host::process_jobs::process_job_timestamp;
pub use clankers_tool_host::process_jobs::project_process_job_list;
pub use clankers_tool_host::process_jobs::reconcile_external_backend_reference;
use serde::Deserialize;
use serde::Serialize;

use crate::RuntimeError;

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
        let retained_until = self
            .max_age
            .and_then(|age| chrono::Duration::from_std(age).ok())
            .map(|age| process_job_timestamp(now + age));
        ProcessJobLogRef {
            stream,
            reference: layout.reference,
            retained_until,
            max_bytes: Some(self.max_bytes_per_job),
        }
    }
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
            decisions.push(ProcessJobRedactionPolicy::default().safe_notification_decision(
                ProcessJobNotificationDecision {
                    kind: ProcessJobNotificationKind::Completion,
                    summary: format!("process job reached terminal status: {:?}", observation.status),
                    log_excerpt: observation.line.clone(),
                },
            ));
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
            decisions.push(ProcessJobRedactionPolicy::default().safe_notification_decision(
                ProcessJobNotificationDecision {
                    kind: ProcessJobNotificationKind::WatchPattern {
                        pattern_index,
                        pattern: pattern.clone(),
                    },
                    summary: format!("process job matched readiness pattern {pattern_index}: {pattern}"),
                    log_excerpt: Some(line.clone()),
                },
            ));
        }
        decisions
    }
}

/// Runtime receipt projection retained as a compatibility extension over backend capability DTOs.
pub trait ProcessJobBackendCapabilitiesReceiptExt {
    #[must_use]
    fn unsupported_receipt(
        &self,
        operation: ProcessJobOperation,
        id: Option<ProcessJobId>,
        message: impl Into<String>,
    ) -> ProcessJobReceipt;
}

impl ProcessJobBackendCapabilitiesReceiptExt for ProcessJobBackendCapabilities {
    fn unsupported_receipt(
        &self,
        operation: ProcessJobOperation,
        id: Option<ProcessJobId>,
        message: impl Into<String>,
    ) -> ProcessJobReceipt {
        let backend = self.backend.unwrap_or(ProcessJobBackendKind::Unknown);
        ProcessJobReceipt::unsupported_with_detail(ProcessJobUnsupportedDetail {
            operation,
            id,
            backend,
            action: operation.action_name().to_string(),
            capability_detail: Some(
                self.unsupported_detail(operation)
                    .map(std::string::ToString::to_string)
                    .unwrap_or_else(|| "capability unsupported".to_string()),
            ),
            message: message.into(),
        })
    }
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
    async fn garbage_collect(
        &self,
        filter: ProcessJobFilter,
    ) -> Result<ProcessJobGarbageCollectionReceipt, RuntimeError>;
    async fn reconcile_startup(&self) -> Result<ProcessJobReconciliationReport, RuntimeError> {
        Ok(ProcessJobReconciliationReport::default())
    }
}

/// Backend adapter boundary. Backends expose facts and capabilities; they do not own UI/storage
/// policy.
#[async_trait]
pub trait ProcessJobBackend: Send + Sync {
    fn kind(&self) -> ProcessJobBackendKind;
    fn capabilities(&self) -> ProcessJobBackendCapabilities;
    async fn start(&self, request: StartProcessJobRequest) -> Result<ProcessJobBackendStart, RuntimeError>;
    async fn observe(&self, backend_ref: BackendRef) -> Result<ProcessJobBackendStatus, RuntimeError>;
    async fn reconcile(&self, summary: ProcessJobSummary) -> Result<ProcessJobReconciliationOutcome, RuntimeError>;
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

pub async fn reconcile_persisted_process_jobs(
    store: &dyn ProcessJobStore,
    backends: &[&dyn ProcessJobBackend],
) -> Result<ProcessJobReconciliationReport, RuntimeError> {
    let mut report = ProcessJobReconciliationReport::default();
    let summaries = store
        .list(ProcessJobFilter {
            backend: None,
            owner: None,
            include_terminal: true,
        })
        .await?;
    for summary in summaries {
        if summary.status.is_terminal() {
            report.skipped_terminal += 1;
            continue;
        }
        let Some(backend) = backends.iter().copied().find(|backend| backend.kind() == summary.backend) else {
            let reason = format!("{} backend is unavailable during startup reconciliation", summary.backend.label());
            let updated = ProcessJobReconciliationOutcome {
                id: summary.id.clone(),
                backend: summary.backend,
                backend_ref: summary.backend_ref.clone(),
                state: ProcessJobReconciliationState::BackendUnavailable,
                log_state: ProcessJobLogReconciliationState::Unavailable { reason: reason.clone() },
                status: ProcessJobStatus::BackendUnavailable { reason },
                log_refs: summary.log_refs.clone(),
                reason: None,
            }
            .into_summary_update(summary, Utc::now());
            store.upsert(updated).await?;
            report.record_observation(ProcessJobReconciliationState::BackendUnavailable);
            continue;
        };
        let outcome = backend.reconcile(summary.clone()).await?;
        let state = outcome.state;
        store.upsert(outcome.into_summary_update(summary, Utc::now())).await?;
        report.record_observation(state);
    }
    Ok(report)
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
    let event = ProcessJobRedactionPolicy::default().safe_notification_event(event);
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
        reconciliation_state: Option<ProcessJobReconciliationState>,
    }

    #[async_trait]
    impl ProcessJobBackend for FakeBackend {
        fn kind(&self) -> ProcessJobBackendKind {
            ProcessJobBackendKind::Native
        }

        fn capabilities(&self) -> ProcessJobBackendCapabilities {
            ProcessJobBackendCapabilities::native()
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

        async fn reconcile(&self, summary: ProcessJobSummary) -> Result<ProcessJobReconciliationOutcome, RuntimeError> {
            self.calls.lock().expect("fake backend calls lock poisoned").push("reconcile");
            let state = self.reconciliation_state.unwrap_or(ProcessJobReconciliationState::ReattachedLogIncomplete);
            let (log_state, status, reason) = match state {
                ProcessJobReconciliationState::Running | ProcessJobReconciliationState::Reattached => {
                    (ProcessJobLogReconciliationState::Complete, ProcessJobStatus::Running, None)
                }
                ProcessJobReconciliationState::ReattachedLogIncomplete => (
                    ProcessJobLogReconciliationState::Incomplete,
                    ProcessJobStatus::ReattachedLogIncomplete,
                    Some("fake backend reattached status but log pipes were not recoverable".to_string()),
                ),
                ProcessJobReconciliationState::Exited => (
                    ProcessJobLogReconciliationState::Complete,
                    ProcessJobStatus::Succeeded { exit_code: Some(0) },
                    None,
                ),
                ProcessJobReconciliationState::LostAfterRestart => (
                    ProcessJobLogReconciliationState::Unavailable {
                        reason: "fake lost after restart".to_string(),
                    },
                    ProcessJobStatus::LostAfterRestart,
                    Some("fake lost after restart".to_string()),
                ),
                ProcessJobReconciliationState::BackendUnavailable => (
                    ProcessJobLogReconciliationState::Unavailable {
                        reason: "fake backend unavailable".to_string(),
                    },
                    ProcessJobStatus::BackendUnavailable {
                        reason: "fake backend unavailable".to_string(),
                    },
                    Some("fake backend unavailable".to_string()),
                ),
                ProcessJobReconciliationState::Orphaned | ProcessJobReconciliationState::IdentityMismatch => (
                    ProcessJobLogReconciliationState::Unavailable {
                        reason: "fake fail-closed reconciliation".to_string(),
                    },
                    ProcessJobStatus::LostAfterRestart,
                    Some("fake fail-closed reconciliation".to_string()),
                ),
            };
            Ok(ProcessJobReconciliationOutcome {
                id: summary.id,
                backend: summary.backend,
                backend_ref: summary.backend_ref,
                state,
                log_state,
                status,
                log_refs: summary.log_refs,
                reason,
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
                profile: None,
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
                profile: None,
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
                profile: None,
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
            max_log_bytes: Some(1024 * 1024),
            allowed_env_prefixes: vec!["APP_".to_string()],
            allowed_cwd_prefixes: vec![PathBuf::from("/repo")],
            allowed_writable_path_prefixes: vec![PathBuf::from("/repo/target")],
            policy_source: "test-policy".to_string(),
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
    }

    #[test]
    fn redaction_policy_bounds_previews_and_redacts_sensitive_metadata() {
        let redaction = ProcessJobRedactionPolicy {
            max_preview_chars: 12,
            max_excerpt_chars: 16,
            max_metadata_value_chars: 8,
        };
        let safe = "cargo nextest run --workspace";
        let secret_command = "curl -H 'Authorization: Bearer shh' https://example.invalid";
        let metadata = BTreeMap::from([
            ("profile".to_string(), "verification-profile".to_string()),
            ("identity.intent".to_string(), "ci".to_string()),
            ("identity.token".to_string(), "raw-token".to_string()),
            ("headers.Authorization".to_string(), "Bearer raw-token".to_string()),
        ]);

        let projected = redaction.safe_identity_metadata(&metadata);

        assert_eq!(redaction.safe_command_preview(safe), "cargo nextes…");
        assert_eq!(redaction.safe_command_preview(secret_command), PROCESS_JOB_REDACTED);
        assert_eq!(redaction.safe_log_excerpt("ready with password=hunter2"), PROCESS_JOB_REDACTED);
        assert_eq!(projected.get("profile").map(String::as_str), Some("verifica…"));
        assert_eq!(projected.get("identity.intent").map(String::as_str), Some("ci"));
        assert_eq!(projected.get("identity.token").map(String::as_str), Some(PROCESS_JOB_REDACTED));
        assert!(!projected.contains_key("headers.Authorization"));
    }

    #[test]
    fn identity_envelope_redacts_command_preview_before_canonicalization() {
        let request = StartProcessJobRequest {
            backend: ProcessJobBackendKind::Native,
            command_preview: "run --token raw-token".to_string(),
            program: Some("run".to_string()),
            args: vec!["--token".to_string(), "raw-token".to_string()],
            shell_command: None,
            cwd: ProcessJobCwd::Inherited,
            owner: ProcessJobOwnerScope::DaemonGlobal,
            resource_policy: ProcessJobResourcePolicy::default(),
            notification_policy: ProcessJobNotificationPolicy::default(),
            metadata: BTreeMap::from([("identity.token".to_string(), "raw-token".to_string())]),
        };

        let canonical =
            String::from_utf8(ProcessJobIdentityEnvelope::for_start_request(&request, "nonce").canonical_bytes())
                .expect("canonical bytes are utf8 fixture");

        assert!(!canonical.contains("raw-token"));
        assert!(canonical.contains(PROCESS_JOB_REDACTED));
    }

    #[test]
    fn blake3_process_job_identity_fixtures_cover_backend_kinds_and_legacy_ids() {
        let request_for_backend = |backend| StartProcessJobRequest {
            backend,
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
        let fixtures = [
            (
                ProcessJobBackendKind::Native,
                "native:42",
                "proc_b3_115e5d8781a631cd008255939c0446e4d96d6661b5435a093a534672c17b4f40",
            ),
            (
                ProcessJobBackendKind::Pueue,
                "start-seq:42",
                "proc_b3_f5e9b858d40fe65de880e52b9adb20aa7cd2b2a08bcaeb1709cd71c86037a668",
            ),
            (
                ProcessJobBackendKind::Systemd,
                "start-seq:42",
                "proc_b3_870a94dc2c0343c549a9d14bc258b37fdb372c1e17def29a4f1946eb2f0c2406",
            ),
        ];

        for (backend, nonce, expected_id) in fixtures {
            let request = request_for_backend(backend);
            let envelope = ProcessJobIdentityEnvelope::for_start_request(&request, nonce);
            let canonical = String::from_utf8(envelope.canonical_bytes()).expect("canonical bytes are utf8 fixture");
            let id = envelope.derive_id();

            assert_eq!(id.0, expected_id, "{backend:?} fixture drifted");
            assert!(id.is_blake3_native(), "{backend:?} id must be BLAKE3-native");
            assert!(canonical.contains(&format!("7:backend={}:{}\n", backend.label().len(), backend.label())));
            assert!(!canonical.contains("pid:"), "backend PID locator must stay out of public identity");
            assert!(!canonical.contains("pueue:"), "pueue task locator must stay out of public identity");
            assert!(!canonical.contains("systemd:"), "systemd unit locator must stay out of public identity");
            assert!(!canonical.contains("must-not-enter-id"), "secret metadata must stay out of public identity");
        }

        for legacy_id in [
            "proc_1",
            "pueue_42",
            "systemd_clankers-build.service",
            "native_pid_1234",
        ] {
            assert!(!ProcessJobId::legacy(legacy_id).is_blake3_native(), "{legacy_id} must remain a legacy projection");
        }
        assert!(!ProcessJobId::legacy("proc_b3_not-a-64-byte-digest").is_blake3_native());
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
            profile: Some(ProcessJobProfileReceiptMetadata {
                profile_name: "quick-check".to_string(),
                manifest_schema_version: PROCESS_JOB_PROFILE_SCHEMA_VERSION,
                profile_source: "/repo/.clankers/process-jobs.json".to_string(),
                policy_source: "workspace-policy".to_string(),
            }),
            summary: "started".to_string(),
            error: None,
        };

        let envelope = ProcessJobToolResult::Start(receipt).into_receipt();
        assert_eq!(envelope.common.operation, ProcessJobOperation::Start);
        assert_eq!(envelope.common.backend_ref, Some(BackendRef("pid:123".to_string())));
        assert_eq!(envelope.common.profile.as_ref().map(|profile| profile.profile_name.as_str()), Some("quick-check"));
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

    #[test]
    fn profile_manifest_sources_resolve_by_deterministic_precedence() {
        let make_source = |precedence, label: &str, command: &str| ProjectProcessJobProfileManifestSource {
            precedence,
            label: label.to_string(),
            path: Some(PathBuf::from(format!("/repo/{label}.json"))),
            manifest: ProjectProcessJobProfiles {
                schema_version: PROCESS_JOB_PROFILE_SCHEMA_VERSION,
                profiles: BTreeMap::from([("verify".to_string(), ProjectProcessJobProfile {
                    command: Some(command.to_string()),
                    cwd: Some(PathBuf::from("/repo")),
                    ..ProjectProcessJobProfile::default()
                })]),
            },
        };
        let sources = vec![
            make_source(ProjectProcessJobProfileSourcePrecedence::Global, "global", "cargo check"),
            make_source(ProjectProcessJobProfileSourcePrecedence::Workspace, "workspace", "cargo nextest run"),
            make_source(ProjectProcessJobProfileSourcePrecedence::Explicit, "explicit", "nix flake check"),
        ];

        let resolved = ProjectProcessJobProfiles::resolve_from_sources(
            &sources,
            "verify",
            ProcessJobOwnerScope::Workspace("repo".to_string()),
            &profile_policy(),
        )
        .expect("highest-precedence profile resolves");

        assert_eq!(resolved.request.shell_command.as_deref(), Some("nix flake check"));
        assert_eq!(resolved.evidence.profile_source, "/repo/explicit.json");
        assert_eq!(resolved.evidence.policy_source, "test-policy");
        assert_eq!(
            resolved.request.metadata.get(PROCESS_JOB_PROFILE_METADATA_SOURCE).map(String::as_str),
            Some("/repo/explicit.json")
        );
    }

    #[test]
    fn profile_manifest_sources_fail_closed_on_same_precedence_duplicates() {
        let duplicate = |label: &str| ProjectProcessJobProfileManifestSource {
            precedence: ProjectProcessJobProfileSourcePrecedence::Workspace,
            label: label.to_string(),
            path: None,
            manifest: ProjectProcessJobProfiles {
                schema_version: PROCESS_JOB_PROFILE_SCHEMA_VERSION,
                profiles: BTreeMap::from([("verify".to_string(), ProjectProcessJobProfile {
                    command: Some("cargo check".to_string()),
                    cwd: Some(PathBuf::from("/repo")),
                    ..ProjectProcessJobProfile::default()
                })]),
            },
        };

        let err = ProjectProcessJobProfiles::resolve_from_sources(
            &[duplicate("workspace-a"), duplicate("workspace-b")],
            "verify",
            ProcessJobOwnerScope::Workspace("repo".to_string()),
            &profile_policy(),
        )
        .expect_err("same-precedence duplicates fail closed");

        assert!(err.to_string().contains("AmbiguousManifestSource"));
        assert!(err.to_string().contains("workspace-a"));
        assert!(err.to_string().contains("workspace-b"));
    }

    #[test]
    fn profile_policy_rejects_paths_resources_and_unsupported_manifest_versions() {
        let too_big = ProjectProcessJobProfiles {
            schema_version: PROCESS_JOB_PROFILE_SCHEMA_VERSION,
            profiles: BTreeMap::from([("heavy".to_string(), ProjectProcessJobProfile {
                command: Some("cargo nextest run".to_string()),
                cwd: Some(PathBuf::from("/repo")),
                writable_paths: vec![PathBuf::from("/repo/target/nextest")],
                resource_policy: ProcessJobResourcePolicy {
                    max_log_bytes: Some(2 * 1024 * 1024),
                    ..ProcessJobResourcePolicy::default()
                },
                ..ProjectProcessJobProfile::default()
            })]),
        };
        let err = too_big
            .resolve("heavy", ProcessJobOwnerScope::Workspace("repo".to_string()), &profile_policy())
            .expect_err("over-policy log cap rejects");
        assert!(err.to_string().contains("ResourceLimitExceeded"));

        let bad_path = ProjectProcessJobProfiles {
            schema_version: PROCESS_JOB_PROFILE_SCHEMA_VERSION,
            profiles: BTreeMap::from([("leaky".to_string(), ProjectProcessJobProfile {
                command: Some("touch /tmp/leak".to_string()),
                cwd: Some(PathBuf::from("/tmp")),
                writable_paths: vec![PathBuf::from("/tmp")],
                ..ProjectProcessJobProfile::default()
            })]),
        };
        let err = bad_path
            .resolve("leaky", ProcessJobOwnerScope::Workspace("repo".to_string()), &profile_policy())
            .expect_err("outside cwd rejects");
        assert!(err.to_string().contains("DisallowedCwd"));

        let unsupported = ProjectProcessJobProfiles {
            schema_version: PROCESS_JOB_PROFILE_SCHEMA_VERSION + 1,
            profiles: BTreeMap::from([("future".to_string(), ProjectProcessJobProfile {
                command: Some("cargo check".to_string()),
                cwd: Some(PathBuf::from("/repo")),
                ..ProjectProcessJobProfile::default()
            })]),
        };
        let err = unsupported
            .resolve("future", ProcessJobOwnerScope::Workspace("repo".to_string()), &profile_policy())
            .expect_err("future schema rejects");
        assert!(err.to_string().contains("UnsupportedManifestVersion"));
    }

    #[test]
    fn process_job_profile_kit_validates_manifest_policy_identity_and_redaction() {
        let profiles = ProjectProcessJobProfiles::from_json_str(
            r#"{
              "profiles": {
                "quick-check": {
                  "command": "cargo check --tests",
                  "cwd": "/repo",
                  "env": {"APP_MODE": "ci"},
                  "resource_policy": {"timeout": {"secs": 300, "nanos": 0}},
                  "notification_policy": {"notify_on_complete": true},
                  "metadata": {"intent": "developer-smoke", "identity.team": "runtime"}
                }
              }
            }"#,
        )
        .expect("profile manifest parses without contacting a backend");

        let resolved = profiles
            .resolve("quick-check", ProcessJobOwnerScope::Workspace("repo".to_string()), &profile_policy())
            .expect("valid profile resolves to backend-neutral start spec");
        assert_eq!(resolved.request.backend, ProcessJobBackendKind::Native);
        assert_eq!(resolved.request.shell_command.as_deref(), Some("cargo check --tests"));
        assert_eq!(resolved.request.program, None);
        assert!(resolved.request.notification_policy.notify_on_complete);
        assert_eq!(resolved.request.metadata.get("profile").map(String::as_str), Some("quick-check"));
        assert_eq!(resolved.request.metadata.get("env:APP_MODE").map(String::as_str), Some("ci"));
        let profile_receipt = ProcessJobProfileReceiptMetadata::from_metadata(&resolved.request.metadata)
            .expect("safe profile receipt metadata projects from resolved request");
        assert_eq!(profile_receipt.profile_name, "quick-check");
        assert_eq!(profile_receipt.manifest_schema_version, PROCESS_JOB_PROFILE_SCHEMA_VERSION);
        assert_eq!(profile_receipt.profile_source, "inline");
        assert_eq!(profile_receipt.policy_source, "test-policy");
        assert_eq!(
            serde_json::to_value(&profile_receipt).expect("profile receipt serializes")["profile_name"],
            "quick-check"
        );

        let envelope = ProcessJobIdentityEnvelope::for_start_request(&resolved.request, "nonce-1");
        let stable_id = envelope.derive_id();
        assert!(stable_id.is_blake3_native());
        assert_eq!(envelope.profile.as_deref(), Some("quick-check"));
        assert_eq!(envelope.metadata.get("identity.team").map(String::as_str), Some("runtime"));
        assert!(!envelope.metadata.contains_key("env:APP_MODE"));

        let mut secret_profiles = ProjectProcessJobProfiles::default();
        secret_profiles.profiles.insert("leaky".to_string(), ProjectProcessJobProfile {
            command: Some("echo ok".to_string()),
            env: BTreeMap::from([("APP_TOKEN".to_string(), "SECRET_TOKEN".to_string())]),
            ..ProjectProcessJobProfile::default()
        });
        let err = secret_profiles
            .resolve("leaky", ProcessJobOwnerScope::Workspace("repo".to_string()), &profile_policy())
            .expect_err("secret env keys reject before backend dispatch");
        assert!(err.to_string().contains("disallowed environment key APP_TOKEN"));

        let redaction = ProcessJobRedactionPolicy::default();
        assert_eq!(redaction.safe_command_preview("Authorization: Bearer raw-token"), PROCESS_JOB_REDACTED);
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
            started_at: Some(process_job_timestamp(Utc::now())),
            updated_at: process_job_timestamp(Utc::now()),
            completed_at: None,
            log_refs: backend_start.log_refs,
            profile: None,
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
            created_at: process_job_timestamp(Utc::now()),
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
            created_at: process_job_timestamp(Utc::now()),
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
    async fn notification_decisions_and_persistence_redact_secret_excerpts() {
        let engine = DefaultProcessJobNotificationPolicyEngine;
        let policy = ProcessJobNotificationPolicy {
            notify_on_complete: true,
            watch_patterns: vec!["Authorization".to_string()],
        };
        let mut state = ProcessJobNotificationPolicyState::default();
        let completion = engine
            .evaluate(&policy, &mut state, ProcessJobNotificationObservation {
                status: ProcessJobStatus::Succeeded { exit_code: Some(0) },
                line: Some("finished with token=raw-token".to_string()),
                tick: 1,
            })
            .await;
        let watch = engine
            .evaluate(&policy, &mut state, ProcessJobNotificationObservation {
                status: ProcessJobStatus::Running,
                line: Some("Authorization: Bearer raw-token".to_string()),
                tick: PROCESS_JOB_WATCH_RATE_LIMIT_TICKS + 2,
            })
            .await;

        assert_eq!(completion[0].log_excerpt.as_deref(), Some(PROCESS_JOB_REDACTED));
        assert_eq!(watch[0].log_excerpt.as_deref(), Some(PROCESS_JOB_REDACTED));
        assert!(matches!(
            &watch[0].kind,
            ProcessJobNotificationKind::WatchPattern { pattern, .. } if pattern == PROCESS_JOB_REDACTED
        ));

        let store = FakeStore::default();
        let sink = FakeSink::default();
        let mut event = notification_event("evt_secret", ProcessJobOwnerScope::DaemonGlobal);
        event.summary = "done token=raw-token".to_string();
        event.log_excerpt = Some("password=hunter2".to_string());
        persist_and_deliver_notification(&store, &sink, event)
            .await
            .expect("redacted event persists and delivers");

        let persisted = store
            .notifications
            .lock()
            .expect("fake notification lock poisoned")
            .first()
            .cloned()
            .expect("notification persisted");
        assert_eq!(persisted.summary, PROCESS_JOB_REDACTED);
        assert_eq!(persisted.log_excerpt.as_deref(), Some(PROCESS_JOB_REDACTED));
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
        assert_eq!(sink.delivered.lock().expect("sink lock").as_slice(), std::slice::from_ref(&event.event_id));

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
            started_at: Some(process_job_timestamp(observed.updated_at)),
            updated_at: process_job_timestamp(observed.updated_at),
            completed_at: None,
            log_refs: observed.log_refs,
            profile: None,
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
    fn backend_capability_defaults_cover_native_pueue_and_systemd_contracts() {
        let native = ProcessJobBackendCapabilities::native();
        assert_eq!(native.backend, Some(ProcessJobBackendKind::Native));
        assert!(native.supports_shell);
        assert!(native.supports_direct_exec);
        assert!(native.supports_stdin);
        assert!(native.supports_kill);
        assert!(native.supports_kill_tree);
        assert!(native.supports_control_group);
        assert!(native.supports_log_cursor);
        assert!(native.supports_log_range);
        assert!(native.supports_live_status);
        assert!(native.supports_completion_notifications);
        assert!(native.supports_readiness_watch);
        assert!(native.supports_adopt);
        assert!(native.supports_restart);
        assert!(native.supports_garbage_collect);
        assert!(native.supports_operation(ProcessJobOperation::GarbageCollect));
        assert!(!native.supports_queueing);
        assert!(!native.supports_dependencies);
        assert!(!native.durable_across_daemon_restart);

        let pueue = ProcessJobBackendCapabilities::pueue();
        assert_eq!(pueue.backend, Some(ProcessJobBackendKind::Pueue));
        assert!(pueue.supports_queueing);
        assert!(pueue.supports_priority);
        assert!(pueue.supports_dependencies);
        assert!(pueue.durable_across_daemon_restart);
        assert!(pueue.supports_live_status);
        assert!(pueue.supports_completion_notifications);
        assert!(pueue.supports_restart);
        assert!(pueue.supports_adopt);
        assert!(!pueue.supports_stdin);
        assert!(!pueue.supports_operation(ProcessJobOperation::GarbageCollect));
        assert!(!pueue.supports_resource_limits);
        assert!(!pueue.supports_readiness_watch);

        let systemd = ProcessJobBackendCapabilities::systemd();
        assert_eq!(systemd.backend, Some(ProcessJobBackendKind::Systemd));
        assert!(systemd.supports_restart);
        assert!(systemd.supports_kill_tree);
        assert!(systemd.supports_control_group);
        assert!(systemd.supports_resource_limits);
        assert!(systemd.supports_adopt);
        assert!(systemd.durable_across_daemon_restart);
        assert!(!systemd.supports_stdin);
        assert!(!systemd.supports_operation(ProcessJobOperation::GarbageCollect));
        assert!(!systemd.supports_queueing);
        assert!(!systemd.supports_dependencies);
    }

    #[test]
    fn fake_backend_capability_matrix_and_unavailable_receipts_are_explicit() {
        let capabilities = FakeBackend::default().capabilities();
        assert_eq!(capabilities, ProcessJobBackendCapabilities::native());
        assert!(capabilities.supports_operation(ProcessJobOperation::WriteStdin));
        assert!(capabilities.supports_operation(ProcessJobOperation::Restart));
        assert_eq!(capabilities.unsupported_detail(ProcessJobOperation::Restart), None);

        let unavailable = ProcessJobReceipt::backend_unavailable(
            ProcessJobOperation::GarbageCollect,
            ProcessJobBackendKind::Systemd,
            "systemd not enabled",
        );
        let unsupported = ProcessJobBackendCapabilities::pueue().unsupported_receipt(
            ProcessJobOperation::WriteStdin,
            Some(ProcessJobId("proc_1".to_string())),
            "stdin is not supported by pueue backend",
        );

        let unavailable_error = unavailable.error.expect("backend unavailable");
        assert_eq!(unavailable_error.code, ProcessJobErrorCode::BackendUnavailable);
        assert_eq!(unavailable_error.action.as_deref(), Some("garbage_collect"));
        let unsupported_error = unsupported.error.expect("stdin unsupported");
        assert_eq!(unsupported_error.code, ProcessJobErrorCode::UnsupportedActionForBackend);
        assert_eq!(unsupported_error.backend, Some(ProcessJobBackendKind::Pueue));
        assert_eq!(unsupported_error.action.as_deref(), Some("write_stdin"));
        assert_eq!(unsupported_error.capability_detail.as_deref(), Some("stdin requires stdin support"));
    }

    #[test]
    fn service_validation_can_fail_closed_before_backend_mutation() {
        let fake = FakeBackend::default();
        let capabilities = ProcessJobBackendCapabilities {
            supports_restart: false,
            ..fake.capabilities()
        };
        let receipt = capabilities.unsupported_receipt(
            ProcessJobOperation::Restart,
            Some(ProcessJobId("proc_1".to_string())),
            "restart unsupported by native backend",
        );

        assert!(fake.calls.lock().expect("fake calls lock poisoned").is_empty());
        let error = receipt.error.expect("unsupported restart receipt");
        assert_eq!(error.code, ProcessJobErrorCode::UnsupportedActionForBackend);
        assert_eq!(error.capability_detail.as_deref(), Some("restart requires restart support"));
    }

    #[test]
    fn list_projection_includes_safe_capability_hints_only() {
        let now = DateTime::parse_from_rfc3339("2026-05-18T00:00:00Z").expect("timestamp parses").with_timezone(&Utc);
        let projection = project_process_job_list(
            [ProcessJobSummary {
                id: ProcessJobId("proc_1".to_string()),
                backend: ProcessJobBackendKind::Systemd,
                backend_ref: Some(BackendRef("systemd:clankers-job.scope".to_string())),
                owner: ProcessJobOwnerScope::DaemonGlobal,
                status: ProcessJobStatus::Running,
                command_preview: "systemd-run true".to_string(),
                cwd: ProcessJobCwd::Inherited,
                started_at: Some(process_job_timestamp(now)),
                updated_at: process_job_timestamp(now),
                completed_at: None,
                log_refs: Vec::new(),
                profile: None,
            }],
            ProcessJobProjectionBounds::default(),
        );

        let hints = &projection.active[0].capability_hints;
        assert!(hints.supports_kill);
        assert!(hints.supports_restart);
        assert!(!hints.supports_stdin);
        assert!(hints.supports_logs);
        assert!(hints.supports_resource_limits);
        let json = serde_json::to_value(&projection.active[0]).expect("projection serializes");
        assert_eq!(json["capability_hints"]["supports_resource_limits"], true);
        assert!(json.get("unavailable_reason").is_none());
    }

    #[test]
    fn status_terminal_classification_is_explicit() {
        assert!(!ProcessJobStatus::Running.is_terminal());
        assert!(ProcessJobStatus::Succeeded { exit_code: Some(0) }.is_terminal());
        assert!(ProcessJobStatus::LostAfterRestart.is_terminal());
    }

    fn persisted_summary(id: &str, backend: ProcessJobBackendKind, status: ProcessJobStatus) -> ProcessJobSummary {
        let now = DateTime::parse_from_rfc3339("2026-05-18T00:00:00Z").expect("timestamp parses").with_timezone(&Utc);
        ProcessJobSummary {
            id: ProcessJobId(id.to_string()),
            backend,
            backend_ref: Some(BackendRef(format!("{}:{id}", backend.label()))),
            owner: ProcessJobOwnerScope::DaemonGlobal,
            status,
            command_preview: "sleep 60".to_string(),
            cwd: ProcessJobCwd::Inherited,
            started_at: Some(process_job_timestamp(now)),
            updated_at: process_job_timestamp(now),
            completed_at: None,
            log_refs: Vec::new(),
            profile: None,
        }
    }

    #[tokio::test]
    async fn startup_reconciliation_updates_nonterminal_jobs_and_skips_terminal_records() {
        let store = FakeStore::default();
        store.summaries.lock().expect("fake store lock poisoned").extend([
            persisted_summary("proc_running", ProcessJobBackendKind::Native, ProcessJobStatus::Running),
            persisted_summary("proc_done", ProcessJobBackendKind::Native, ProcessJobStatus::Succeeded {
                exit_code: Some(0),
            }),
            persisted_summary("pueue_missing", ProcessJobBackendKind::Pueue, ProcessJobStatus::Pending),
        ]);
        let backend = FakeBackend::default();

        let report = reconcile_persisted_process_jobs(&store, &[&backend]).await.expect("reconciliation succeeds");

        assert_eq!(report.checked, 2);
        assert_eq!(report.updated, 1);
        assert_eq!(report.unavailable, 1);
        assert_eq!(report.skipped_terminal, 1);
        assert_eq!(backend.calls.lock().expect("fake backend calls lock poisoned").as_slice(), ["reconcile"]);
        let summaries = store.summaries.lock().expect("fake store lock poisoned");
        let reattached = summaries
            .iter()
            .rev()
            .find(|summary| summary.id.0 == "proc_running")
            .expect("reattached summary persisted");
        assert_eq!(reattached.status, ProcessJobStatus::ReattachedLogIncomplete);
        let unavailable = summaries
            .iter()
            .rev()
            .find(|summary| summary.id.0 == "pueue_missing")
            .expect("unavailable summary persisted");
        assert!(matches!(unavailable.status, ProcessJobStatus::BackendUnavailable { .. }));
    }

    #[test]
    fn retention_policy_classifies_metadata_lifetimes_and_active_protection() {
        let now = DateTime::parse_from_rfc3339("2026-05-18T00:00:00Z").expect("timestamp parses").with_timezone(&Utc);
        let policy = ProcessJobRetentionPolicy {
            max_age: Some(Duration::from_secs(60)),
            max_records: Some(10),
            max_log_bytes: Some(1024),
        };

        let active_summary = persisted_summary("proc_active", ProcessJobBackendKind::Native, ProcessJobStatus::Running);
        let active = policy.classify_summary(&active_summary, process_job_timestamp(now), Some("default".to_string()));
        assert_eq!(active.class, ProcessJobRetentionClass::Active);
        assert!(active.class.protects_active_state());
        assert_eq!(active.metadata_retained_until, Some(process_job_timestamp(now + chrono::Duration::seconds(60))));
        assert_eq!(active.log_retained_until, active.metadata_retained_until);
        assert_eq!(active.event_retained_until, active.metadata_retained_until);
        assert_eq!(active.policy_ref.as_deref(), Some("default"));

        let failed_summary =
            persisted_summary("proc_failed", ProcessJobBackendKind::Native, ProcessJobStatus::Failed {
                exit_code: Some(1),
                reason: "boom".to_string(),
            });
        let failed = policy.classify_summary(&failed_summary, process_job_timestamp(now), None);
        assert_eq!(failed.class, ProcessJobRetentionClass::Failed);
        assert!(!failed.class.protects_active_state());
        assert!(matches!(
            policy.eligibility_for_summary(
                &active_summary,
                process_job_timestamp(now + chrono::Duration::seconds(3600)),
                None
            ),
            ProcessJobRetentionEligibility::ProtectActive { .. }
        ));
        assert!(matches!(
            policy.eligibility_for_summary(
                &failed_summary,
                process_job_timestamp(now + chrono::Duration::seconds(30)),
                None
            ),
            ProcessJobRetentionEligibility::KeepUntil { .. }
        ));
        assert!(matches!(
            policy.eligibility_for_summary(
                &failed_summary,
                process_job_timestamp(now + chrono::Duration::seconds(120)),
                None
            ),
            ProcessJobRetentionEligibility::Eligible { .. }
        ));
    }

    #[test]
    fn log_overflow_policy_fixtures_cover_truncation_and_disk_pressure() {
        let overflow = ProcessJobLogOverflowPolicy {
            max_line_bytes: 10,
            max_chunk_bytes: 20,
            max_file_bytes: 30,
            max_total_bytes: 40,
        };
        let fixtures = [
            (
                "under every limit is accepted",
                10,
                20,
                30,
                ProcessJobLogWriteDisposition::Accept,
                serde_json::json!({ "kind": "accept" }),
            ),
            (
                "line overflow reports dropped bytes before chunk/file pressure",
                12,
                25,
                35,
                ProcessJobLogWriteDisposition::TruncateLine { dropped_bytes: 2 },
                serde_json::json!({ "kind": "truncate_line", "dropped_bytes": 2 }),
            ),
            (
                "chunk overflow reports dropped bytes when line is bounded",
                9,
                25,
                29,
                ProcessJobLogWriteDisposition::TruncateChunk { dropped_bytes: 5 },
                serde_json::json!({ "kind": "truncate_chunk", "dropped_bytes": 5 }),
            ),
            (
                "per-file overflow degrades as disk-full without corrupting counters",
                9,
                19,
                31,
                ProcessJobLogWriteDisposition::DegradeDiskFull,
                serde_json::json!({ "kind": "degrade_disk_full" }),
            ),
            (
                "total overflow degrades as disk-full even below per-file cap",
                9,
                19,
                41,
                ProcessJobLogWriteDisposition::DegradeDiskFull,
                serde_json::json!({ "kind": "degrade_disk_full" }),
            ),
        ];

        for (name, line_bytes, chunk_bytes, total_bytes, expected, expected_json) in fixtures {
            let actual = overflow.classify_write(line_bytes, chunk_bytes, total_bytes);
            assert_eq!(actual, expected, "{name}");
            assert_eq!(serde_json::to_value(&actual).expect("disposition serializes"), expected_json, "{name}");
            assert_eq!(
                serde_json::from_value::<ProcessJobLogWriteDisposition>(expected_json)
                    .expect("disposition deserializes"),
                expected,
                "{name}"
            );
        }
    }

    #[test]
    fn log_overflow_policy_fixture_serialization_is_stable() {
        let policy = ProcessJobLogOverflowPolicy {
            max_line_bytes: 10,
            max_chunk_bytes: 20,
            max_file_bytes: 30,
            max_total_bytes: 40,
        };
        let json = serde_json::to_value(&policy).expect("policy serializes");
        assert_eq!(
            json,
            serde_json::json!({
                "max_line_bytes": 10,
                "max_chunk_bytes": 20,
                "max_file_bytes": 30,
                "max_total_bytes": 40,
            })
        );
        let roundtrip: ProcessJobLogOverflowPolicy = serde_json::from_value(json).expect("policy deserializes");
        assert_eq!(roundtrip, policy);
    }

    #[tokio::test]
    async fn fake_backend_reconciliation_covers_every_outcome_state() {
        let states = [
            ProcessJobReconciliationState::Running,
            ProcessJobReconciliationState::Reattached,
            ProcessJobReconciliationState::ReattachedLogIncomplete,
            ProcessJobReconciliationState::Exited,
            ProcessJobReconciliationState::LostAfterRestart,
            ProcessJobReconciliationState::BackendUnavailable,
            ProcessJobReconciliationState::Orphaned,
            ProcessJobReconciliationState::IdentityMismatch,
        ];

        for state in states {
            let backend = FakeBackend {
                reconciliation_state: Some(state),
                ..FakeBackend::default()
            };
            let summary = persisted_summary("proc_state", ProcessJobBackendKind::Native, ProcessJobStatus::Running);
            let outcome = backend.reconcile(summary.clone()).await.expect("fake backend reconciles");
            assert_eq!(outcome.state, state);
            let updated = outcome.into_summary_update(summary, Utc::now());
            assert_eq!(updated.id, ProcessJobId("proc_state".to_string()));
            assert_eq!(updated.backend_ref, Some(BackendRef("native:proc_state".to_string())));
            match state {
                ProcessJobReconciliationState::Running | ProcessJobReconciliationState::Reattached => {
                    assert_eq!(updated.status, ProcessJobStatus::Running);
                }
                ProcessJobReconciliationState::ReattachedLogIncomplete => {
                    assert_eq!(updated.status, ProcessJobStatus::ReattachedLogIncomplete);
                }
                ProcessJobReconciliationState::Exited => {
                    assert_eq!(updated.status, ProcessJobStatus::Succeeded { exit_code: Some(0) });
                    assert!(updated.completed_at.is_some());
                }
                ProcessJobReconciliationState::BackendUnavailable => {
                    assert!(matches!(updated.status, ProcessJobStatus::BackendUnavailable { .. }));
                    assert!(updated.completed_at.is_some());
                }
                ProcessJobReconciliationState::LostAfterRestart
                | ProcessJobReconciliationState::Orphaned
                | ProcessJobReconciliationState::IdentityMismatch => {
                    assert_eq!(updated.status, ProcessJobStatus::LostAfterRestart);
                    assert!(updated.completed_at.is_some());
                }
            }
        }
    }

    #[test]
    fn reconciliation_state_vocabulary_serializes_and_classifies_fail_closed_states() {
        let states = vec![
            ProcessJobReconciliationState::Running,
            ProcessJobReconciliationState::Reattached,
            ProcessJobReconciliationState::ReattachedLogIncomplete,
            ProcessJobReconciliationState::Exited,
            ProcessJobReconciliationState::LostAfterRestart,
            ProcessJobReconciliationState::BackendUnavailable,
            ProcessJobReconciliationState::Orphaned,
            ProcessJobReconciliationState::IdentityMismatch,
        ];
        let serialized = serde_json::to_value(&states).expect("states serialize");

        assert_eq!(
            serialized,
            serde_json::json!([
                "running",
                "reattached",
                "reattached_log_incomplete",
                "exited",
                "lost_after_restart",
                "backend_unavailable",
                "orphaned",
                "identity_mismatch"
            ])
        );
        assert!(ProcessJobReconciliationState::ReattachedLogIncomplete.is_adopted());
        assert!(ProcessJobReconciliationState::IdentityMismatch.is_fail_closed());
        assert!(!ProcessJobReconciliationState::LostAfterRestart.is_adopted());
    }

    #[test]
    fn native_identity_reconciliation_fails_closed_on_pid_reuse_or_ambiguous_identity() {
        let persisted = NativeProcessJobIdentity {
            pid: 4242,
            process_group: Some(4242),
            start_time_ticks: Some(100),
            command_fingerprint: Some("cmd:a".to_string()),
            cwd_fingerprint: Some("cwd:repo".to_string()),
        };
        let matching = NativeProcessJobObservation {
            pid: 4242,
            process_group: Some(4242),
            start_time_ticks: Some(100),
            command_fingerprint: Some("cmd:a".to_string()),
            cwd_fingerprint: Some("cwd:repo".to_string()),
        };
        let reused_pid = NativeProcessJobObservation {
            start_time_ticks: Some(200),
            ..matching.clone()
        };
        let ambiguous = NativeProcessJobIdentity {
            start_time_ticks: None,
            command_fingerprint: None,
            cwd_fingerprint: None,
            ..persisted.clone()
        };

        assert_eq!(
            persisted.verify_observation(Some(&matching)),
            ProcessJobReconciliationState::ReattachedLogIncomplete
        );
        assert_eq!(persisted.verify_observation(Some(&reused_pid)), ProcessJobReconciliationState::IdentityMismatch);
        assert_eq!(ambiguous.verify_observation(Some(&matching)), ProcessJobReconciliationState::IdentityMismatch);
        assert_eq!(persisted.verify_observation(None), ProcessJobReconciliationState::LostAfterRestart);
    }

    #[test]
    fn reconciliation_outcome_updates_summary_without_changing_stable_id() {
        let now = DateTime::parse_from_rfc3339("2026-05-18T01:00:00Z").expect("timestamp parses").with_timezone(&Utc);
        let summary = ProcessJobSummary {
            id: ProcessJobId("proc_stable".to_string()),
            backend: ProcessJobBackendKind::Pueue,
            backend_ref: Some(BackendRef("pueue:7".to_string())),
            owner: ProcessJobOwnerScope::DaemonGlobal,
            status: ProcessJobStatus::Running,
            command_preview: "build".to_string(),
            cwd: ProcessJobCwd::Inherited,
            started_at: Some(process_job_timestamp(now)),
            updated_at: process_job_timestamp(now),
            completed_at: None,
            log_refs: Vec::new(),
            profile: None,
        };
        let outcome = ProcessJobReconciliationOutcome {
            id: summary.id.clone(),
            backend: ProcessJobBackendKind::Pueue,
            backend_ref: Some(BackendRef("pueue:7".to_string())),
            state: ProcessJobReconciliationState::BackendUnavailable,
            log_state: ProcessJobLogReconciliationState::Unavailable {
                reason: "pueue daemon unavailable".to_string(),
            },
            status: ProcessJobStatus::BackendUnavailable {
                reason: "pueue daemon unavailable".to_string(),
            },
            log_refs: Vec::new(),
            reason: Some("pueue daemon unavailable".to_string()),
        };

        let updated = outcome.into_summary_update(summary, now + chrono::Duration::seconds(5));

        assert_eq!(updated.id, ProcessJobId("proc_stable".to_string()));
        assert!(matches!(updated.status, ProcessJobStatus::BackendUnavailable { .. }));
        assert_eq!(updated.completed_at, Some(process_job_timestamp(now + chrono::Duration::seconds(5))));
    }

    #[test]
    fn external_backend_reconciliation_maps_refs_into_common_outcomes() {
        let facts = ExternalProcessJobReconciliationFacts {
            id: ProcessJobId("proc_pueue".to_string()),
            backend: ProcessJobBackendKind::Pueue,
            expected_backend_ref: BackendRef("pueue:7".to_string()),
            observed_backend_ref: Some(BackendRef("pueue:7".to_string())),
            state: ExternalProcessJobBackendState::Succeeded { exit_code: Some(0) },
            log_refs: vec![ProcessJobLogRef {
                stream: ProcessJobStream::Combined,
                reference: "pueue:7".to_string(),
                retained_until: None,
                max_bytes: Some(4096),
            }],
        };

        let outcome = reconcile_external_backend_reference(facts);

        assert_eq!(outcome.id, ProcessJobId("proc_pueue".to_string()));
        assert_eq!(outcome.backend_ref, Some(BackendRef("pueue:7".to_string())));
        assert_eq!(outcome.state, ProcessJobReconciliationState::Exited);
        assert_eq!(outcome.log_state, ProcessJobLogReconciliationState::BackendReferenced);
        assert_eq!(outcome.status, ProcessJobStatus::Succeeded { exit_code: Some(0) });
    }

    #[test]
    fn external_backend_reconciliation_fails_closed_for_unavailable_missing_or_mismatched_refs() {
        let make_facts = |state, observed_backend_ref| ExternalProcessJobReconciliationFacts {
            id: ProcessJobId("proc_systemd".to_string()),
            backend: ProcessJobBackendKind::Systemd,
            expected_backend_ref: BackendRef("systemd:clankers-job.scope".to_string()),
            observed_backend_ref,
            state,
            log_refs: Vec::new(),
        };

        let unavailable = reconcile_external_backend_reference(make_facts(
            ExternalProcessJobBackendState::BackendUnavailable {
                reason: "systemd unavailable".to_string(),
            },
            None,
        ));
        let missing = reconcile_external_backend_reference(make_facts(ExternalProcessJobBackendState::Missing, None));
        let mismatch = reconcile_external_backend_reference(make_facts(
            ExternalProcessJobBackendState::Running,
            Some(BackendRef("systemd:other.scope".to_string())),
        ));

        assert_eq!(unavailable.state, ProcessJobReconciliationState::BackendUnavailable);
        assert_eq!(missing.state, ProcessJobReconciliationState::Orphaned);
        assert_eq!(mismatch.state, ProcessJobReconciliationState::IdentityMismatch);
        assert_eq!(mismatch.backend_ref, None);
        assert!(matches!(unavailable.status, ProcessJobStatus::BackendUnavailable { .. }));
    }

    #[test]
    fn native_admission_decision_is_owned_by_process_job_contracts() {
        let accepted = native_process_job_admission_decision(ProcessJobNativeAdmissionInput { active: 31, limit: 32 });
        let rejected = native_process_job_admission_decision(ProcessJobNativeAdmissionInput { active: 32, limit: 32 });

        assert!(accepted.accepted);
        assert_eq!(accepted.active, 31);
        assert_eq!(accepted.limit, 32);
        assert!(!rejected.accepted);
        assert_eq!(rejected.summary(), "native process admission denied: active process limit reached (32/32)");
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
        assert_eq!(log_ref.retained_until, Some(process_job_timestamp(now + chrono::Duration::seconds(60))));
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
        assert!(!observer.can_access(&owner, ProcessJobOperation::Log, ProcessJobBackendKind::Native));
        assert!(!observer.capabilities.allows_log_access(false));
        assert!(!observer.capabilities.allows_log_access(true));
        let bounded_log_reader = ProcessJobCallerScope {
            session_id: Some("sess-a".to_string()),
            capabilities: ProcessJobCapabilitySet::bounded_log_reader(),
            ..ProcessJobCallerScope::default()
        };
        assert!(bounded_log_reader.can_access(&owner, ProcessJobOperation::Log, ProcessJobBackendKind::Native));
        assert!(bounded_log_reader.capabilities.allows_log_access(false));
        assert!(!bounded_log_reader.capabilities.allows_log_access(true));
        assert!(ProcessJobCapabilitySet::raw_log_reader().allows_log_access(true));
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
            raw: false,
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
                        "range": {"stream": "combined", "offset": 7, "limit_bytes": 1024},
                        "raw": false
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
            profile: None,
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
        let gc_receipt = ProcessJobToolResult::GarbageCollect(ProcessJobGarbageCollectionReceipt {
            operation: ProcessJobOperation::GarbageCollect,
            removed_metadata_count: 1,
            removed_records: vec![id.clone()],
            tombstoned_records: Vec::new(),
            deleted_native_log_files: 1,
            removed_log_bytes: 2,
            released_log_refs: vec![ProcessJobReleasedLogRef {
                id: id.clone(),
                backend: ProcessJobBackendKind::Native,
                reference: "native:proc_b3_115/combined.log".to_string(),
                bytes: 2,
            }],
            skipped_active_jobs: Vec::new(),
            failures: Vec::new(),
            summary: "process job GC removed 1 metadata records, tombstoned 0 records, deleted 1 native log files, released 1 backend log refs, reclaimed 2 log bytes, skipped 0 active jobs, 0 failures".to_string(),
        })
        .into_receipt();
        let error_receipt = ProcessJobReceipt::unsupported(
            ProcessJobOperation::WriteStdin,
            Some(id.clone()),
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
            (
                gc_receipt,
                serde_json::json!({
                    "common": {
                        "operation": "garbage_collect",
                        "id": null,
                        "backend": null,
                        "status": null,
                        "backend_ref": null,
                        "summary": "process job GC removed 1 metadata records, tombstoned 0 records, deleted 1 native log files, released 1 backend log refs, reclaimed 2 log bytes, skipped 0 active jobs, 0 failures",
                        "error": null
                    },
                    "payload": {
                        "kind": "garbage_collect",
                        "data": {
                            "receipt": {
                                "operation": "garbage_collect",
                                "removed_metadata_count": 1,
                                "removed_records": ["proc_b3_115e5d8781a631cd008255939c0446e4d96d6661b5435a093a534672c17b4f40"],
                                "tombstoned_records": [],
                                "deleted_native_log_files": 1,
                                "removed_log_bytes": 2,
                                "released_log_refs": [{
                                    "id": "proc_b3_115e5d8781a631cd008255939c0446e4d96d6661b5435a093a534672c17b4f40",
                                    "backend": "native",
                                    "reference": "native:proc_b3_115/combined.log",
                                    "bytes": 2
                                }],
                                "skipped_active_jobs": [],
                                "failures": [],
                                "summary": "process job GC removed 1 metadata records, tombstoned 0 records, deleted 1 native log files, released 1 backend log refs, reclaimed 2 log bytes, skipped 0 active jobs, 0 failures"
                            }
                        }
                    }
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
            profile: None,
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
                started_at: Some(process_job_timestamp(base)),
                updated_at: process_job_timestamp(base),
                completed_at: None,
                log_refs: Vec::new(),
                profile: None,
            },
            ProcessJobSummary {
                id: ProcessJobId("pueue_done".to_string()),
                backend: ProcessJobBackendKind::Pueue,
                backend_ref: Some(BackendRef("pueue:7".to_string())),
                owner: ProcessJobOwnerScope::DaemonGlobal,
                status: ProcessJobStatus::Succeeded { exit_code: Some(0) },
                command_preview: "pueue build".to_string(),
                cwd: ProcessJobCwd::Inherited,
                started_at: Some(process_job_timestamp(base)),
                updated_at: process_job_timestamp(base + chrono::Duration::seconds(1)),
                completed_at: Some(process_job_timestamp(base + chrono::Duration::seconds(1))),
                log_refs: Vec::new(),
                profile: None,
            },
            ProcessJobSummary {
                id: ProcessJobId("systemd_active".to_string()),
                backend: ProcessJobBackendKind::Systemd,
                backend_ref: Some(BackendRef("systemd:clankers-job.scope".to_string())),
                owner: ProcessJobOwnerScope::DaemonGlobal,
                status: ProcessJobStatus::Waiting,
                command_preview: "systemd run".to_string(),
                cwd: ProcessJobCwd::Inherited,
                started_at: Some(process_job_timestamp(base)),
                updated_at: process_job_timestamp(base + chrono::Duration::seconds(2)),
                completed_at: None,
                log_refs: Vec::new(),
                profile: None,
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
