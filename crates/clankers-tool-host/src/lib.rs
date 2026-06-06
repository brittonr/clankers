//! Reusable tool execution contracts for engine-host runners.
//!
//! This crate owns plain tool-host outcomes and result accumulation. It does
//! not supervise Clankers plugins, discover built-in tools, or interpret engine
//! reducer policy.

use std::collections::BTreeMap;
use std::sync::Arc;

use clanker_message::Content;
use clankers_engine::EngineCorrelationId;
use clankers_engine::EngineToolCall;
use serde::Deserialize;
use serde::Serialize;
use serde_json::Value;
use thiserror::Error;

pub mod process_jobs;

pub const DEFAULT_TOOL_MAX_BYTES: usize = 200_000;
pub const DEFAULT_TOOL_MAX_LINES: usize = 10_000;

pub type ToolHostFuture<'a, T> = core::pin::Pin<Box<dyn core::future::Future<Output = T> + Send + 'a>>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolDescriptor {
    pub name: String,
    pub description: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolHostServiceKind {
    Storage,
    Search,
    Hooks,
    Process,
    Progress,
    Capability,
    Cancellation,
    RuntimePolicy,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum ToolHostServiceStatus {
    Available,
    Unavailable { reason: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolHostServiceHandle {
    pub kind: ToolHostServiceKind,
    pub status: ToolHostServiceStatus,
    pub metadata: BTreeMap<String, String>,
}

impl ToolHostServiceHandle {
    #[must_use]
    pub fn available(kind: ToolHostServiceKind) -> Self {
        Self {
            kind,
            status: ToolHostServiceStatus::Available,
            metadata: BTreeMap::new(),
        }
    }

    #[must_use]
    pub fn unavailable(kind: ToolHostServiceKind, reason: impl Into<String>) -> Self {
        Self {
            kind,
            status: ToolHostServiceStatus::Unavailable { reason: reason.into() },
            metadata: BTreeMap::new(),
        }
    }

    #[must_use]
    pub fn with_metadata(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.metadata.insert(safe_metadata_key(key.into()), safe_metadata(value.into()));
        self
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolHostServices {
    services: BTreeMap<ToolHostServiceKind, ToolHostServiceHandle>,
}

impl ToolHostServices {
    #[must_use]
    pub fn empty() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn with_service(mut self, handle: ToolHostServiceHandle) -> Self {
        self.services.insert(handle.kind, handle);
        self
    }

    #[must_use]
    pub fn get(&self, kind: ToolHostServiceKind) -> Option<&ToolHostServiceHandle> {
        self.services.get(&kind)
    }

    #[must_use]
    pub fn is_available(&self, kind: ToolHostServiceKind) -> bool {
        matches!(self.get(kind).map(|handle| &handle.status), Some(ToolHostServiceStatus::Available))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CapabilityDecision {
    Allowed,
    Denied { reason: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolStorageKey {
    pub namespace: String,
    pub key: String,
}

impl ToolStorageKey {
    #[must_use]
    pub fn new(namespace: impl Into<String>, key: impl Into<String>) -> Self {
        Self {
            namespace: safe_metadata_key(namespace.into()),
            key: safe_metadata_key(key.into()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolStorageValue {
    pub bytes: Vec<u8>,
    pub content_type: Option<String>,
    pub metadata: BTreeMap<String, String>,
}

impl ToolStorageValue {
    #[must_use]
    pub fn new(bytes: Vec<u8>) -> Self {
        Self {
            bytes,
            content_type: None,
            metadata: BTreeMap::new(),
        }
    }

    #[must_use]
    pub fn with_content_type(mut self, content_type: impl Into<String>) -> Self {
        self.content_type = Some(safe_metadata(content_type.into()));
        self
    }

    #[must_use]
    pub fn with_metadata(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.metadata.insert(safe_metadata_key(key.into()), safe_metadata(value.into()));
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolStorageReadRequest {
    pub key: ToolStorageKey,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolStorageReadResult {
    pub value: Option<ToolStorageValue>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolStorageWriteRequest {
    pub key: ToolStorageKey,
    pub value: ToolStorageValue,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolStorageWriteResult {
    pub stored: bool,
    pub metadata: BTreeMap<String, String>,
}

pub trait ToolStorageService: Send + Sync {
    fn read(&self, request: ToolStorageReadRequest) -> Result<ToolStorageReadResult, ToolHostError>;
    fn write(&self, request: ToolStorageWriteRequest) -> Result<ToolStorageWriteResult, ToolHostError>;
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolSearchRequest {
    pub query: String,
    pub limit: u32,
    pub metadata: BTreeMap<String, String>,
}

impl ToolSearchRequest {
    #[must_use]
    pub fn new(query: impl Into<String>, limit: u32) -> Self {
        Self {
            query: safe_metadata(query.into()),
            limit,
            metadata: BTreeMap::new(),
        }
    }

    #[must_use]
    pub fn with_metadata(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.metadata.insert(safe_metadata_key(key.into()), safe_metadata(value.into()));
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolSearchHit {
    pub title: String,
    pub snippet: String,
    pub rank: u32,
    pub metadata: BTreeMap<String, String>,
}

impl ToolSearchHit {
    #[must_use]
    pub fn new(title: impl Into<String>, snippet: impl Into<String>, rank: u32) -> Self {
        Self {
            title: safe_metadata(title.into()),
            snippet: safe_metadata(snippet.into()),
            rank,
            metadata: BTreeMap::new(),
        }
    }

    #[must_use]
    pub fn with_metadata(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.metadata.insert(safe_metadata_key(key.into()), safe_metadata(value.into()));
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolSearchResult {
    pub hits: Vec<ToolSearchHit>,
}

pub trait ToolSearchService: Send + Sync {
    fn search(&self, request: ToolSearchRequest) -> Result<ToolSearchResult, ToolHostError>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolHookPhase {
    Before,
    After,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolHookRequest {
    pub phase: ToolHookPhase,
    pub call_id: String,
    pub tool_name: String,
    pub input: Value,
    pub metadata: BTreeMap<String, String>,
}

impl ToolHookRequest {
    #[must_use]
    pub fn before(call_id: impl Into<String>, tool_name: impl Into<String>, input: Value) -> Self {
        Self::new(ToolHookPhase::Before, call_id, tool_name, input)
    }

    #[must_use]
    pub fn after(call_id: impl Into<String>, tool_name: impl Into<String>, input: Value) -> Self {
        Self::new(ToolHookPhase::After, call_id, tool_name, input)
    }

    #[must_use]
    pub fn with_metadata(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.metadata.insert(safe_metadata_key(key.into()), safe_metadata(value.into()));
        self
    }

    fn new(phase: ToolHookPhase, call_id: impl Into<String>, tool_name: impl Into<String>, input: Value) -> Self {
        Self {
            phase,
            call_id: safe_metadata_key(call_id.into()),
            tool_name: safe_metadata_key(tool_name.into()),
            input,
            metadata: BTreeMap::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "decision", rename_all = "snake_case")]
pub enum ToolHookDecision {
    Continue,
    Modify { input: Value },
    Deny { reason: String },
}

pub trait ToolHookService: Send + Sync {
    fn decide(&self, request: ToolHookRequest) -> ToolHostFuture<'_, Result<ToolHookDecision, ToolHostError>>;
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolCapabilityRequest {
    pub call_id: String,
    pub tool_name: String,
    pub input: Value,
    pub metadata: BTreeMap<String, String>,
}

impl ToolCapabilityRequest {
    #[must_use]
    pub fn new(call_id: impl Into<String>, tool_name: impl Into<String>, input: Value) -> Self {
        Self {
            call_id: safe_metadata_key(call_id.into()),
            tool_name: safe_metadata_key(tool_name.into()),
            input,
            metadata: BTreeMap::new(),
        }
    }

    #[must_use]
    pub fn with_metadata(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.metadata.insert(safe_metadata_key(key.into()), safe_metadata(value.into()));
        self
    }
}

pub trait ToolCapabilityService: Send + Sync {
    fn check(&self, request: ToolCapabilityRequest) -> Result<CapabilityDecision, ToolHostError>;
}

pub trait ToolCancellationService: Send + Sync {
    fn cancellation_state(&self, call_id: &str) -> ToolInvocationCancellation;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolRuntimePolicyKind {
    Steel,
    Process,
    Network,
    FileSystem,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolRuntimePolicyRequest {
    pub call_id: String,
    pub tool_name: String,
    pub kind: ToolRuntimePolicyKind,
    pub metadata: BTreeMap<String, String>,
}

impl ToolRuntimePolicyRequest {
    #[must_use]
    pub fn new(call_id: impl Into<String>, tool_name: impl Into<String>, kind: ToolRuntimePolicyKind) -> Self {
        Self {
            call_id: safe_metadata_key(call_id.into()),
            tool_name: safe_metadata_key(tool_name.into()),
            kind,
            metadata: BTreeMap::new(),
        }
    }

    #[must_use]
    pub fn with_metadata(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.metadata.insert(safe_metadata_key(key.into()), safe_metadata(value.into()));
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "decision", rename_all = "snake_case")]
pub enum ToolRuntimePolicyDecision {
    Allowed,
    Denied { reason: String },
}

pub trait ToolRuntimePolicyService: Send + Sync {
    fn authorize(&self, request: ToolRuntimePolicyRequest) -> Result<ToolRuntimePolicyDecision, ToolHostError>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolTruncationLimits {
    pub max_bytes: usize,
    pub max_lines: usize,
}

impl Default for ToolTruncationLimits {
    fn default() -> Self {
        Self {
            max_bytes: DEFAULT_TOOL_MAX_BYTES,
            max_lines: DEFAULT_TOOL_MAX_LINES,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolTruncationMetadata {
    pub original_bytes: usize,
    pub original_lines: usize,
    pub truncated_bytes: usize,
    pub truncated_lines: usize,
}

#[derive(Debug, Clone)]
pub enum ToolHostOutcome {
    Succeeded {
        content: Vec<Content>,
        details: Value,
    },
    ToolError {
        content: Vec<Content>,
        details: Value,
        message: String,
    },
    MissingTool {
        name: String,
    },
    CapabilityDenied {
        name: String,
        reason: String,
    },
    Cancelled {
        name: String,
    },
    Truncated {
        content: Vec<Content>,
        metadata: ToolTruncationMetadata,
    },
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum ToolHostError {
    #[error("tool host failed: {message}")]
    HostFailed { message: String },
}

pub trait ToolCatalog {
    fn describe_tools(&self) -> Vec<ToolDescriptor>;
    fn contains_tool(&self, name: &str) -> bool;
}

pub trait CapabilityChecker {
    fn check_capability(&mut self, call: &EngineToolCall) -> CapabilityDecision;
}

pub trait ToolHook {
    fn before_tool(&mut self, call: &EngineToolCall) -> Result<(), ToolHostError>;
    fn after_tool(&mut self, call: &EngineToolCall, outcome: &ToolHostOutcome) -> Result<(), ToolHostError>;
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolProgressKind {
    Started,
    Progress,
    Completed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolProgressEvent {
    pub call_id: String,
    pub kind: ToolProgressKind,
    pub message: String,
    pub metadata: BTreeMap<String, String>,
}

impl ToolProgressEvent {
    #[must_use]
    pub fn new(call_id: impl Into<String>, kind: ToolProgressKind, message: impl Into<String>) -> Self {
        Self {
            call_id: call_id.into(),
            kind,
            message: safe_metadata(message.into()),
            metadata: BTreeMap::new(),
        }
    }

    #[must_use]
    pub fn with_metadata(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.metadata.insert(safe_metadata_key(key.into()), safe_metadata(value.into()));
        self
    }
}

pub trait ToolProgressSink: Send + Sync {
    fn emit(&self, event: ToolProgressEvent) -> Result<(), ToolHostError>;
}

pub struct NullToolProgressSink;

impl ToolProgressSink for NullToolProgressSink {
    fn emit(&self, event: ToolProgressEvent) -> Result<(), ToolHostError> {
        let _ = event;
        Ok(())
    }
}

#[derive(Clone)]
pub struct ToolInvocationContext {
    pub call_id: String,
    pub capability: CapabilityDecision,
    pub services: ToolHostServices,
    pub cancellation: ToolInvocationCancellation,
    pub progress: Arc<dyn ToolProgressSink>,
    pub storage: Option<Arc<dyn ToolStorageService>>,
    pub search: Option<Arc<dyn ToolSearchService>>,
    pub hooks: Option<Arc<dyn ToolHookService>>,
    pub capability_service: Option<Arc<dyn ToolCapabilityService>>,
    pub cancellation_service: Option<Arc<dyn ToolCancellationService>>,
    pub runtime_policy: Option<Arc<dyn ToolRuntimePolicyService>>,
    pub metadata: BTreeMap<String, String>,
}

impl ToolInvocationContext {
    #[must_use]
    pub fn new(call_id: impl Into<String>) -> Self {
        Self {
            call_id: call_id.into(),
            capability: CapabilityDecision::Allowed,
            services: ToolHostServices::empty(),
            cancellation: ToolInvocationCancellation::default(),
            progress: Arc::new(NullToolProgressSink),
            storage: None,
            search: None,
            hooks: None,
            capability_service: None,
            cancellation_service: None,
            runtime_policy: None,
            metadata: BTreeMap::new(),
        }
    }

    #[must_use]
    pub fn with_capability(mut self, capability: CapabilityDecision) -> Self {
        self.capability = capability;
        self
    }

    #[must_use]
    pub fn with_services(mut self, services: ToolHostServices) -> Self {
        self.services = services;
        self
    }

    #[must_use]
    pub fn with_cancellation(mut self, cancellation: ToolInvocationCancellation) -> Self {
        self.cancellation = cancellation;
        self
    }

    #[must_use]
    pub fn with_progress_sink(mut self, progress: Arc<dyn ToolProgressSink>) -> Self {
        self.progress = progress;
        self
    }

    #[must_use]
    pub fn with_storage_service(mut self, storage: Arc<dyn ToolStorageService>) -> Self {
        self.storage = Some(storage);
        self
    }

    #[must_use]
    pub fn with_search_service(mut self, search: Arc<dyn ToolSearchService>) -> Self {
        self.search = Some(search);
        self
    }

    #[must_use]
    pub fn with_hook_service(mut self, hooks: Arc<dyn ToolHookService>) -> Self {
        self.hooks = Some(hooks);
        self
    }

    #[must_use]
    pub fn with_capability_service(mut self, capability_service: Arc<dyn ToolCapabilityService>) -> Self {
        self.capability_service = Some(capability_service);
        self
    }

    #[must_use]
    pub fn with_cancellation_service(mut self, cancellation_service: Arc<dyn ToolCancellationService>) -> Self {
        self.cancellation_service = Some(cancellation_service);
        self
    }

    #[must_use]
    pub fn with_runtime_policy(mut self, runtime_policy: Arc<dyn ToolRuntimePolicyService>) -> Self {
        self.runtime_policy = Some(runtime_policy);
        self
    }

    #[must_use]
    pub fn with_metadata(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.metadata.insert(safe_metadata_key(key.into()), safe_metadata(value.into()));
        self
    }

    pub fn ensure_allowed(&self, tool_name: &str) -> Result<(), ToolHostOutcome> {
        match &self.capability {
            CapabilityDecision::Allowed => Ok(()),
            CapabilityDecision::Denied { reason } => Err(ToolHostOutcome::CapabilityDenied {
                name: tool_name.to_string(),
                reason: reason.clone(),
            }),
        }
    }

    pub fn ensure_not_cancelled(&self, tool_name: &str) -> Result<(), ToolHostOutcome> {
        if self.cancellation.cancelled {
            return Err(ToolHostOutcome::Cancelled {
                name: tool_name.to_string(),
            });
        }
        Ok(())
    }

    pub fn require_service(&self, tool_name: &str, kind: ToolHostServiceKind) -> Result<(), ToolHostOutcome> {
        if self.services.is_available(kind) {
            return Ok(());
        }
        let message = match self.services.get(kind).map(|handle| &handle.status) {
            Some(ToolHostServiceStatus::Unavailable { reason }) => reason.clone(),
            Some(ToolHostServiceStatus::Available) => String::new(),
            None => format!("required service {kind:?} unavailable"),
        };
        Err(ToolHostOutcome::ToolError {
            content: vec![Content::Text { text: message.clone() }],
            details: serde_json::json!({
                "tool": tool_name,
                "missing_service": format!("{kind:?}").to_ascii_lowercase(),
            }),
            message,
        })
    }

    pub fn emit_progress(&self, kind: ToolProgressKind, message: impl Into<String>) -> Result<(), ToolHostError> {
        self.progress.emit(ToolProgressEvent::new(self.call_id.clone(), kind, message))
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolInvocationCancellation {
    pub cancelled: bool,
    pub reason: Option<String>,
}

impl ToolInvocationCancellation {
    #[must_use]
    pub fn active() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn cancelled(reason: impl Into<String>) -> Self {
        Self {
            cancelled: true,
            reason: Some(reason.into()),
        }
    }
}

pub trait ToolExecutor {
    fn execute_tool(&mut self, call: EngineToolCall) -> impl core::future::Future<Output = ToolHostOutcome> + Send;
}

pub trait NeutralToolExecutor {
    fn execute_tool_with_context(
        &mut self,
        call: EngineToolCall,
        context: ToolInvocationContext,
    ) -> impl core::future::Future<Output = ToolHostOutcome> + Send;
}

#[derive(Debug, Clone)]
pub struct ToolOutputAccumulator {
    limits: ToolTruncationLimits,
    chunks: Vec<String>,
}

impl ToolOutputAccumulator {
    #[must_use]
    pub fn new(limits: ToolTruncationLimits) -> Self {
        assert!(limits.max_bytes > 0, "tool truncation max_bytes must be positive");
        assert!(limits.max_lines > 0, "tool truncation max_lines must be positive");
        Self {
            limits,
            chunks: Vec::new(),
        }
    }

    pub fn push(&mut self, chunk: impl Into<String>) {
        self.chunks.push(chunk.into());
    }

    #[must_use]
    pub fn finish(self) -> ToolHostOutcome {
        let combined = self.chunks.concat();
        let original_bytes = combined.len();
        let original_lines = count_lines(&combined);
        let truncated = truncate_utf8_by_bytes_and_lines(&combined, &self.limits);
        let truncated_bytes = truncated.len();
        let truncated_lines = count_lines(&truncated);
        let content = vec![Content::Text { text: truncated }];
        if original_bytes > truncated_bytes || original_lines > truncated_lines {
            return ToolHostOutcome::Truncated {
                content,
                metadata: ToolTruncationMetadata {
                    original_bytes,
                    original_lines,
                    truncated_bytes,
                    truncated_lines,
                },
            };
        }
        ToolHostOutcome::Succeeded {
            content,
            details: serde_json::json!({ "truncated": false }),
        }
    }
}

#[must_use]
pub fn tool_call_id(call: &EngineToolCall) -> &EngineCorrelationId {
    &call.call_id
}

#[must_use]
fn safe_metadata_key(value: String) -> String {
    value.chars().take(160).collect()
}

#[must_use]
fn safe_metadata(value: String) -> String {
    if contains_secret_marker(&value) {
        "[REDACTED]".to_string()
    } else {
        value.chars().take(160).collect()
    }
}

#[must_use]
fn contains_secret_marker(value: &str) -> bool {
    let lower = value.to_ascii_lowercase();
    [
        "token",
        "secret",
        "password",
        "api_key",
        "authorization",
        "bearer",
        "cookie",
    ]
    .iter()
    .any(|marker| lower.contains(marker))
}

#[must_use]
fn count_lines(text: &str) -> usize {
    if text.is_empty() {
        return 0;
    }
    text.lines().count()
}

#[must_use]
fn truncate_utf8_by_bytes_and_lines(text: &str, limits: &ToolTruncationLimits) -> String {
    let mut kept = String::new();
    let mut line_count = 0usize;
    for piece in text.split_inclusive('\n') {
        if line_count >= limits.max_lines {
            break;
        }
        let remaining = limits.max_bytes.saturating_sub(kept.len());
        if remaining == 0 {
            break;
        }
        let prefix = utf8_prefix(piece, remaining);
        kept.push_str(prefix);
        line_count = line_count.saturating_add(usize::from(prefix.ends_with('\n')));
        if prefix.len() < piece.len() {
            break;
        }
        if !piece.ends_with('\n') {
            line_count = line_count.saturating_add(1);
        }
    }
    debug_assert!(kept.len() <= limits.max_bytes);
    debug_assert!(count_lines(&kept) <= limits.max_lines);
    kept
}

#[must_use]
fn utf8_prefix(text: &str, max_bytes: usize) -> &str {
    if text.len() <= max_bytes {
        return text;
    }
    let mut end = max_bytes;
    while end > 0 && !text.is_char_boundary(end) {
        end -= 1;
    }
    &text[..end]
}

#[cfg(test)]
mod tests {
    use std::future::Future;
    use std::task::Context;
    use std::task::Poll;

    use super::*;

    const SMALL_BYTES: usize = 6;
    const TWO_LINES: usize = 2;

    struct FakeCatalog {
        tools: Vec<ToolDescriptor>,
    }

    impl ToolCatalog for FakeCatalog {
        fn describe_tools(&self) -> Vec<ToolDescriptor> {
            self.tools.clone()
        }

        fn contains_tool(&self, name: &str) -> bool {
            self.tools.iter().any(|tool| tool.name == name)
        }
    }

    struct FakeCapabilityChecker {
        decision: CapabilityDecision,
    }

    impl CapabilityChecker for FakeCapabilityChecker {
        fn check_capability(&mut self, _call: &EngineToolCall) -> CapabilityDecision {
            self.decision.clone()
        }
    }

    #[derive(Default)]
    struct RecordingHook {
        events: Vec<&'static str>,
    }

    #[derive(Default)]
    struct RecordingProgressSink {
        events: std::sync::Mutex<Vec<ToolProgressEvent>>,
    }

    impl ToolProgressSink for RecordingProgressSink {
        fn emit(&self, event: ToolProgressEvent) -> Result<(), ToolHostError> {
            self.events.lock().expect("progress lock").push(event);
            Ok(())
        }
    }

    #[derive(Default)]
    struct FakeStorageService {
        writes: std::sync::Mutex<Vec<ToolStorageWriteRequest>>,
    }

    impl ToolStorageService for FakeStorageService {
        fn read(&self, _request: ToolStorageReadRequest) -> Result<ToolStorageReadResult, ToolHostError> {
            Ok(ToolStorageReadResult {
                value: Some(ToolStorageValue::new(b"fixture".to_vec()).with_content_type("text/plain")),
            })
        }

        fn write(&self, request: ToolStorageWriteRequest) -> Result<ToolStorageWriteResult, ToolHostError> {
            self.writes.lock().expect("storage lock").push(request);
            Ok(ToolStorageWriteResult {
                stored: true,
                metadata: BTreeMap::new(),
            })
        }
    }

    struct FakeSearchService;

    impl ToolSearchService for FakeSearchService {
        fn search(&self, request: ToolSearchRequest) -> Result<ToolSearchResult, ToolHostError> {
            Ok(ToolSearchResult {
                hits: vec![ToolSearchHit::new("fixture", request.query, 1)],
            })
        }
    }

    struct FakeHookService {
        decision: ToolHookDecision,
    }

    impl ToolHookService for FakeHookService {
        fn decide(&self, _request: ToolHookRequest) -> ToolHostFuture<'_, Result<ToolHookDecision, ToolHostError>> {
            let decision = self.decision.clone();
            Box::pin(async move { Ok(decision) })
        }
    }

    struct FakeToolCapabilityService {
        decision: CapabilityDecision,
    }

    impl ToolCapabilityService for FakeToolCapabilityService {
        fn check(&self, _request: ToolCapabilityRequest) -> Result<CapabilityDecision, ToolHostError> {
            Ok(self.decision.clone())
        }
    }

    struct FakeCancellationService {
        state: ToolInvocationCancellation,
    }

    impl ToolCancellationService for FakeCancellationService {
        fn cancellation_state(&self, _call_id: &str) -> ToolInvocationCancellation {
            self.state.clone()
        }
    }

    struct FakeRuntimePolicyService {
        decision: ToolRuntimePolicyDecision,
    }

    impl ToolRuntimePolicyService for FakeRuntimePolicyService {
        fn authorize(&self, _request: ToolRuntimePolicyRequest) -> Result<ToolRuntimePolicyDecision, ToolHostError> {
            Ok(self.decision.clone())
        }
    }

    struct NeutralReadFixtureTool;

    impl NeutralToolExecutor for NeutralReadFixtureTool {
        async fn execute_tool_with_context(
            &mut self,
            call: EngineToolCall,
            context: ToolInvocationContext,
        ) -> ToolHostOutcome {
            if let Err(outcome) = context.ensure_not_cancelled(&call.tool_name) {
                return outcome;
            }
            if let Err(outcome) = context.ensure_allowed(&call.tool_name) {
                return outcome;
            }
            let _ = context.emit_progress(ToolProgressKind::Started, "reading fixture");
            ToolHostOutcome::Succeeded {
                content: vec![Content::Text {
                    text: "read fixture".to_string(),
                }],
                details: serde_json::json!({"source": "neutral_read_fixture"}),
            }
        }
    }

    struct NeutralMutationFixtureTool;

    impl NeutralToolExecutor for NeutralMutationFixtureTool {
        async fn execute_tool_with_context(
            &mut self,
            call: EngineToolCall,
            context: ToolInvocationContext,
        ) -> ToolHostOutcome {
            if let Err(outcome) = context.ensure_not_cancelled(&call.tool_name) {
                return outcome;
            }
            if let Err(outcome) = context.ensure_allowed(&call.tool_name) {
                return outcome;
            }
            if let Err(outcome) = context.require_service(&call.tool_name, ToolHostServiceKind::Storage) {
                return outcome;
            }
            let _ = context.emit_progress(ToolProgressKind::Progress, "writing fixture");
            let mut accumulator = ToolOutputAccumulator::new(ToolTruncationLimits {
                max_bytes: SMALL_BYTES,
                max_lines: DEFAULT_TOOL_MAX_LINES,
            });
            accumulator.push("mutation-output");
            accumulator.finish()
        }
    }

    impl ToolHook for RecordingHook {
        fn before_tool(&mut self, _call: &EngineToolCall) -> Result<(), ToolHostError> {
            self.events.push("before");
            Ok(())
        }

        fn after_tool(&mut self, _call: &EngineToolCall, _outcome: &ToolHostOutcome) -> Result<(), ToolHostError> {
            self.events.push("after");
            Ok(())
        }
    }

    fn block_on<F: Future>(future: F) -> F::Output {
        let waker = std::task::Waker::noop();
        let mut context = Context::from_waker(waker);
        let mut future = Box::pin(future);
        loop {
            match future.as_mut().poll(&mut context) {
                Poll::Ready(output) => return output,
                Poll::Pending => std::thread::yield_now(),
            }
        }
    }

    fn engine_tool_call(name: &str) -> EngineToolCall {
        EngineToolCall {
            call_id: EngineCorrelationId("call-1".to_string()),
            tool_name: name.to_string(),
            input: serde_json::json!({}),
        }
    }

    fn first_text(content: &[Content]) -> &str {
        let Some(Content::Text { text }) = content.first() else {
            panic!("expected first text content block");
        };
        text
    }

    #[test]
    fn accumulator_keeps_short_output() {
        let mut accumulator = ToolOutputAccumulator::new(ToolTruncationLimits {
            max_bytes: DEFAULT_TOOL_MAX_BYTES,
            max_lines: DEFAULT_TOOL_MAX_LINES,
        });
        accumulator.push("hello");
        let outcome = accumulator.finish();
        assert!(matches!(outcome, ToolHostOutcome::Succeeded { .. }));
    }

    #[test]
    fn accumulator_truncates_by_utf8_boundary() {
        let mut accumulator = ToolOutputAccumulator::new(ToolTruncationLimits {
            max_bytes: SMALL_BYTES,
            max_lines: DEFAULT_TOOL_MAX_LINES,
        });
        accumulator.push("éééé");
        let outcome = accumulator.finish();
        let ToolHostOutcome::Truncated { content, metadata } = outcome else {
            panic!("expected truncated output");
        };
        assert_eq!(first_text(&content), "ééé");
        assert_eq!(metadata.original_bytes, "éééé".len());
        assert_eq!(metadata.truncated_bytes, SMALL_BYTES);
    }

    #[test]
    fn accumulator_truncates_by_line_boundary() {
        let mut accumulator = ToolOutputAccumulator::new(ToolTruncationLimits {
            max_bytes: DEFAULT_TOOL_MAX_BYTES,
            max_lines: TWO_LINES,
        });
        accumulator.push("one\ntwo\nthree\n");
        let outcome = accumulator.finish();
        let ToolHostOutcome::Truncated { content, metadata } = outcome else {
            panic!("expected line truncation");
        };
        assert_eq!(first_text(&content), "one\ntwo\n");
        assert_eq!(metadata.original_lines, 3);
        assert_eq!(metadata.truncated_lines, TWO_LINES);
    }

    #[test]
    #[should_panic(expected = "tool truncation max_bytes must be positive")]
    fn accumulator_rejects_zero_byte_limit() {
        let _ = ToolOutputAccumulator::new(ToolTruncationLimits {
            max_bytes: 0,
            max_lines: DEFAULT_TOOL_MAX_LINES,
        });
    }

    #[test]
    fn catalog_lists_metadata_and_checks_lookup() {
        let catalog = FakeCatalog {
            tools: vec![ToolDescriptor {
                name: "read".to_string(),
                description: "read file".to_string(),
            }],
        };

        let tools = catalog.describe_tools();

        assert_eq!(tools.len(), 1);
        assert_eq!(tools[0].name, "read");
        assert!(catalog.contains_tool("read"));
        assert!(!catalog.contains_tool("write"));
    }

    #[test]
    fn capability_checker_allows_and_denies() {
        let call = engine_tool_call("read");
        let mut allowed = FakeCapabilityChecker {
            decision: CapabilityDecision::Allowed,
        };
        let mut denied = FakeCapabilityChecker {
            decision: CapabilityDecision::Denied {
                reason: "blocked".to_string(),
            },
        };

        assert_eq!(allowed.check_capability(&call), CapabilityDecision::Allowed);
        assert_eq!(denied.check_capability(&call), CapabilityDecision::Denied {
            reason: "blocked".to_string()
        });
    }

    #[test]
    fn hook_ordering_is_explicit() {
        let call = engine_tool_call("read");
        let outcome = ToolHostOutcome::Succeeded {
            content: vec![Content::Text { text: "ok".to_string() }],
            details: serde_json::json!({}),
        };
        let mut hook = RecordingHook::default();

        hook.before_tool(&call).expect("before hook should pass");
        hook.after_tool(&call, &outcome).expect("after hook should pass");

        assert_eq!(hook.events, vec!["before", "after"]);
    }

    #[test]
    fn neutral_hook_service_fixtures_cover_continue_modify_and_deny() {
        let call = engine_tool_call("read");
        for decision in [
            ToolHookDecision::Continue,
            ToolHookDecision::Modify {
                input: serde_json::json!({"path": "safe"}),
            },
            ToolHookDecision::Deny {
                reason: "blocked".to_string(),
            },
        ] {
            let hooks = FakeHookService {
                decision: decision.clone(),
            };
            let request = ToolHookRequest::before(call.call_id.0.clone(), call.tool_name.clone(), call.input.clone());
            let observed = block_on(hooks.decide(request)).expect("hook decision");
            assert_eq!(observed, decision);
        }
    }

    #[test]
    fn neutral_service_contracts_cover_storage_search_hooks_capability_cancellation_and_runtime_policy() {
        let storage = Arc::new(FakeStorageService::default());
        let search = Arc::new(FakeSearchService);
        let hooks = Arc::new(FakeHookService {
            decision: ToolHookDecision::Modify {
                input: serde_json::json!({"path": "safe"}),
            },
        });
        let capability = Arc::new(FakeToolCapabilityService {
            decision: CapabilityDecision::Denied {
                reason: "blocked".to_string(),
            },
        });
        let cancellation = Arc::new(FakeCancellationService {
            state: ToolInvocationCancellation::cancelled("user"),
        });
        let runtime_policy = Arc::new(FakeRuntimePolicyService {
            decision: ToolRuntimePolicyDecision::Denied {
                reason: "steel disabled".to_string(),
            },
        });
        let context = ToolInvocationContext::new("call-1")
            .with_storage_service(storage.clone())
            .with_search_service(search)
            .with_hook_service(hooks)
            .with_capability_service(capability)
            .with_cancellation_service(cancellation)
            .with_runtime_policy(runtime_policy);

        let storage_key = ToolStorageKey::new("session", "state.json");
        let storage_write = context
            .storage
            .as_ref()
            .expect("storage service")
            .write(ToolStorageWriteRequest {
                key: storage_key.clone(),
                value: ToolStorageValue::new(br#"{"ok":true}"#.to_vec()).with_content_type("application/json"),
            })
            .expect("storage write");
        let storage_read = context
            .storage
            .as_ref()
            .expect("storage service")
            .read(ToolStorageReadRequest { key: storage_key })
            .expect("storage read");
        let search_result = context
            .search
            .as_ref()
            .expect("search service")
            .search(ToolSearchRequest::new("needle", 5))
            .expect("search");
        let hook_future = context.hooks.as_ref().expect("hook service").decide(ToolHookRequest::before(
            "call-1",
            "read",
            serde_json::json!({"path": "unsafe"}),
        ));
        let hook_decision = block_on(hook_future).expect("hook decision");
        let capability_decision = context
            .capability_service
            .as_ref()
            .expect("capability service")
            .check(ToolCapabilityRequest::new("call-1", "read", serde_json::json!({"path": "safe"})))
            .expect("capability decision");
        let cancellation_state =
            context.cancellation_service.as_ref().expect("cancellation service").cancellation_state("call-1");
        let runtime_decision = context
            .runtime_policy
            .as_ref()
            .expect("runtime policy")
            .authorize(ToolRuntimePolicyRequest::new("call-1", "run", ToolRuntimePolicyKind::Steel))
            .expect("runtime policy decision");

        assert!(storage_write.stored);
        assert_eq!(storage_read.value.expect("stored value").bytes, b"fixture".to_vec());
        assert_eq!(storage.writes.lock().expect("storage lock").len(), 1);
        assert_eq!(search_result.hits[0].snippet, "needle");
        assert!(matches!(hook_decision, ToolHookDecision::Modify { .. }));
        assert!(matches!(capability_decision, CapabilityDecision::Denied { reason } if reason == "blocked"));
        assert!(cancellation_state.cancelled);
        assert!(matches!(runtime_decision, ToolRuntimePolicyDecision::Denied { reason } if reason == "steel disabled"));
    }

    #[test]
    fn neutral_context_fixtures_cover_success_progress_storage_denial_cancel_and_truncation() {
        let call = engine_tool_call("neutral_read");
        let progress = Arc::new(RecordingProgressSink::default());
        let mut read_tool = NeutralReadFixtureTool;
        let read_outcome = block_on(read_tool.execute_tool_with_context(
            call.clone(),
            ToolInvocationContext::new("call-1").with_progress_sink(progress.clone()),
        ));
        assert!(matches!(read_outcome, ToolHostOutcome::Succeeded { .. }));
        assert_eq!(progress.events.lock().expect("progress lock").len(), 1);

        let denied = block_on(read_tool.execute_tool_with_context(
            call.clone(),
            ToolInvocationContext::new("call-1").with_capability(CapabilityDecision::Denied {
                reason: "blocked".to_string(),
            }),
        ));
        assert!(matches!(denied, ToolHostOutcome::CapabilityDenied { reason, .. } if reason == "blocked"));

        let cancelled = block_on(read_tool.execute_tool_with_context(
            call.clone(),
            ToolInvocationContext::new("call-1").with_cancellation(ToolInvocationCancellation::cancelled("user")),
        ));
        assert!(matches!(cancelled, ToolHostOutcome::Cancelled { .. }));

        let mut mutation_tool = NeutralMutationFixtureTool;
        let missing_storage =
            block_on(mutation_tool.execute_tool_with_context(call.clone(), ToolInvocationContext::new("call-1")));
        assert!(
            matches!(missing_storage, ToolHostOutcome::ToolError { message, .. } if message.to_ascii_lowercase().contains("storage"))
        );

        let truncation = block_on(mutation_tool.execute_tool_with_context(
            call,
            ToolInvocationContext::new("call-1").with_services(
                ToolHostServices::empty().with_service(ToolHostServiceHandle::available(ToolHostServiceKind::Storage)),
            ),
        ));
        assert!(matches!(truncation, ToolHostOutcome::Truncated { .. }));
    }

    #[test]
    fn neutral_context_redacts_secret_progress_and_metadata() {
        let event = ToolProgressEvent::new("call-secret", ToolProgressKind::Progress, "bearer token abc")
            .with_metadata("authorization", "secret value");
        assert_eq!(event.message, "[REDACTED]");
        assert_eq!(event.metadata.get("authorization").unwrap(), "[REDACTED]");

        let handle =
            ToolHostServiceHandle::available(ToolHostServiceKind::Storage).with_metadata("api_key", "secret value");
        assert_eq!(handle.metadata.get("api_key").unwrap(), "[REDACTED]");
    }

    #[test]
    fn outcome_variants_are_explicit() {
        let outcomes = [
            ToolHostOutcome::Succeeded {
                content: Vec::new(),
                details: serde_json::json!({}),
            },
            ToolHostOutcome::ToolError {
                content: Vec::new(),
                details: serde_json::json!({}),
                message: "bad".to_string(),
            },
            ToolHostOutcome::MissingTool {
                name: "missing".to_string(),
            },
            ToolHostOutcome::CapabilityDenied {
                name: "read".to_string(),
                reason: "blocked".to_string(),
            },
            ToolHostOutcome::Cancelled {
                name: "read".to_string(),
            },
            ToolHostOutcome::Truncated {
                content: Vec::new(),
                metadata: ToolTruncationMetadata {
                    original_bytes: 2,
                    original_lines: 1,
                    truncated_bytes: 1,
                    truncated_lines: 1,
                },
            },
        ];

        assert_eq!(outcomes.len(), 6);
    }

    #[test]
    #[should_panic(expected = "tool truncation max_lines must be positive")]
    fn accumulator_rejects_zero_line_limit() {
        let _ = ToolOutputAccumulator::new(ToolTruncationLimits {
            max_bytes: DEFAULT_TOOL_MAX_BYTES,
            max_lines: 0,
        });
    }

    #[test]
    fn capability_decision_can_deny() {
        let denied = CapabilityDecision::Denied {
            reason: "blocked".to_string(),
        };
        assert_eq!(denied, CapabilityDecision::Denied {
            reason: "blocked".to_string()
        });
    }
}
