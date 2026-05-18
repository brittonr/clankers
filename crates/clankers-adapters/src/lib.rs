//! Shell-free adapter bricks for embedding Clankers engine turns in products.
//!
//! This crate intentionally contains only small deterministic adapters and DTOs.
//! It does not start runtimes, discover providers, open daemon sockets, read
//! credentials, or load plugins.

use std::collections::BTreeSet;
use std::collections::HashMap;
use std::collections::VecDeque;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use std::time::Duration;

use clanker_message::Content;
use clanker_message::StopReason;
use clanker_message::Usage;
use clankers_engine::EngineCorrelationId;
use clankers_engine::EngineEvent;
use clankers_engine::EngineModelRequest;
use clankers_engine::EngineModelResponse;
use clankers_engine::EngineTerminalFailure;
use clankers_engine::EngineToolCall;
use clankers_engine_host::CancellationSource;
use clankers_engine_host::EngineEventSink;
use clankers_engine_host::HostAdapterError;
use clankers_engine_host::ModelHost;
use clankers_engine_host::ModelHostOutcome;
use clankers_engine_host::RetrySleeper;
use clankers_engine_host::UsageObservation;
use clankers_engine_host::UsageObserver;
use clankers_tool_host::ToolCatalog;
use clankers_tool_host::ToolDescriptor;
use clankers_tool_host::ToolExecutor;
use clankers_tool_host::ToolHostOutcome;
use clankers_tool_host::ToolOutputAccumulator;
use clankers_tool_host::ToolTruncationLimits;
use serde::Deserialize;
use serde::Serialize;
use serde_json::Value;
use thiserror::Error;

pub const DEFAULT_CANCELLED_REASON: &str = "embedded host cancelled";

#[derive(Debug, Default, Clone)]
pub struct MemoryEventSink {
    events: Vec<EngineEvent>,
}

impl MemoryEventSink {
    #[must_use]
    pub fn events(&self) -> &[EngineEvent] {
        &self.events
    }

    #[must_use]
    pub fn into_events(self) -> Vec<EngineEvent> {
        self.events
    }
}

impl EngineEventSink for MemoryEventSink {
    fn emit_engine_event(&mut self, event: &EngineEvent) -> Result<(), HostAdapterError> {
        self.events.push(event.clone());
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct AtomicCancellationSource {
    cancelled: Arc<AtomicBool>,
    reason: String,
}

impl Default for AtomicCancellationSource {
    fn default() -> Self {
        Self::new(DEFAULT_CANCELLED_REASON)
    }
}

impl AtomicCancellationSource {
    #[must_use]
    pub fn new(reason: impl Into<String>) -> Self {
        Self {
            cancelled: Arc::new(AtomicBool::new(false)),
            reason: reason.into(),
        }
    }

    #[must_use]
    pub fn handle(&self) -> Arc<AtomicBool> {
        Arc::clone(&self.cancelled)
    }

    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::SeqCst);
    }

    pub fn reset(&self) {
        self.cancelled.store(false, Ordering::SeqCst);
    }
}

impl CancellationSource for AtomicCancellationSource {
    fn is_cancelled(&mut self) -> bool {
        self.cancelled.load(Ordering::SeqCst)
    }

    fn cancellation_reason(&mut self) -> String {
        self.reason.clone()
    }
}

#[derive(Debug, Default, Clone)]
pub struct NoopRetrySleeper {
    sleeps: Vec<(EngineCorrelationId, Duration)>,
}

impl NoopRetrySleeper {
    #[must_use]
    pub fn sleeps(&self) -> &[(EngineCorrelationId, Duration)] {
        &self.sleeps
    }
}

impl RetrySleeper for NoopRetrySleeper {
    async fn sleep_for_retry(
        &mut self,
        request_id: EngineCorrelationId,
        delay: Duration,
    ) -> Result<(), HostAdapterError> {
        self.sleeps.push((request_id, delay));
        Ok(())
    }
}

#[derive(Debug, Default, Clone)]
pub struct CollectingUsageObserver {
    observations: Vec<UsageObservation>,
}

impl CollectingUsageObserver {
    #[must_use]
    pub fn observations(&self) -> &[UsageObservation] {
        &self.observations
    }
}

impl UsageObserver for CollectingUsageObserver {
    fn observe_usage(&mut self, observation: &UsageObservation) -> Result<(), HostAdapterError> {
        self.observations.push(observation.clone());
        Ok(())
    }
}

#[derive(Debug, Default, Clone)]
pub struct ScriptedModelHost {
    outcomes: VecDeque<ModelHostOutcome>,
    requests: Vec<EngineModelRequest>,
}

impl ScriptedModelHost {
    #[must_use]
    pub fn new(outcomes: impl IntoIterator<Item = ModelHostOutcome>) -> Self {
        Self {
            outcomes: outcomes.into_iter().collect(),
            requests: Vec::new(),
        }
    }

    #[must_use]
    pub fn completed_text(text: impl Into<String>) -> ModelHostOutcome {
        ModelHostOutcome::Completed {
            response: EngineModelResponse {
                output: vec![Content::Text { text: text.into() }],
                stop_reason: StopReason::Stop,
            },
            usage: None,
        }
    }

    #[must_use]
    pub fn completed_text_with_usage(text: impl Into<String>, usage: Usage) -> ModelHostOutcome {
        ModelHostOutcome::Completed {
            response: EngineModelResponse {
                output: vec![Content::Text { text: text.into() }],
                stop_reason: StopReason::Stop,
            },
            usage: Some(usage),
        }
    }

    #[must_use]
    pub fn tool_request(id: impl Into<String>, name: impl Into<String>, input: Value) -> ModelHostOutcome {
        ModelHostOutcome::Completed {
            response: EngineModelResponse {
                output: vec![Content::ToolUse {
                    id: id.into(),
                    name: name.into(),
                    input,
                }],
                stop_reason: StopReason::ToolUse,
            },
            usage: None,
        }
    }

    #[must_use]
    pub fn requests(&self) -> &[EngineModelRequest] {
        &self.requests
    }
}

impl ModelHost for ScriptedModelHost {
    async fn execute_model(&mut self, request: EngineModelRequest) -> ModelHostOutcome {
        self.requests.push(request);
        self.outcomes.pop_front().unwrap_or_else(|| ModelHostOutcome::Failed {
            failure: EngineTerminalFailure {
                message: "scripted model host has no queued outcome".to_string(),
                status: None,
                retryable: false,
            },
        })
    }
}

#[derive(Debug, Default, Clone)]
pub struct ScriptedToolExecutor {
    outcomes: VecDeque<ToolHostOutcome>,
    calls: Vec<EngineToolCall>,
}

impl ScriptedToolExecutor {
    #[must_use]
    pub fn new(outcomes: impl IntoIterator<Item = ToolHostOutcome>) -> Self {
        Self {
            outcomes: outcomes.into_iter().collect(),
            calls: Vec::new(),
        }
    }

    #[must_use]
    pub fn text_success(text: impl Into<String>) -> ToolHostOutcome {
        ToolHostOutcome::Succeeded {
            content: vec![Content::Text { text: text.into() }],
            details: serde_json::json!({}),
        }
    }

    #[must_use]
    pub fn text_error(message: impl Into<String>) -> ToolHostOutcome {
        let message = message.into();
        ToolHostOutcome::ToolError {
            content: vec![Content::Text { text: message.clone() }],
            details: serde_json::json!({}),
            message,
        }
    }

    #[must_use]
    pub fn calls(&self) -> &[EngineToolCall] {
        &self.calls
    }
}

impl ToolExecutor for ScriptedToolExecutor {
    async fn execute_tool(&mut self, call: EngineToolCall) -> ToolHostOutcome {
        self.calls.push(call.clone());
        self.outcomes.pop_front().unwrap_or_else(|| ToolHostOutcome::MissingTool { name: call.tool_name })
    }
}

#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct EmbeddedToolCatalog {
    pub tools: Vec<EmbeddedToolMetadata>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct EmbeddedToolMetadata {
    pub name: String,
    pub description: String,
    pub runtime: EmbeddedToolRuntime,
    #[serde(default)]
    pub capabilities: Vec<EmbeddedCapability>,
    #[serde(default)]
    pub approval: ApprovalPolicy,
    #[serde(default)]
    pub redaction: RedactionPolicy,
    #[serde(default)]
    pub input_schema: Value,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum EmbeddedCapability {
    Read,
    Mutate,
    Shell,
    Network,
    RawLog,
    SecretAdjacent,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum EmbeddedToolRuntime {
    ProductOwned,
    InProcess,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ApprovalPolicy {
    #[default]
    Never,
    PerCall,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum RedactionPolicy {
    #[default]
    None,
    Summarize,
    Drop,
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum CatalogValidationError {
    #[error("duplicate tool name `{name}`")]
    DuplicateToolName { name: String },
    #[error("tool `{name}` is missing description")]
    MissingDescription { name: String },
    #[error("tool `{name}` uses unsafe capability `{capability:?}` without per-call approval")]
    UnsafeDefault {
        name: String,
        capability: EmbeddedCapability,
    },
    #[error("tool `{name}` uses secret-adjacent capability without redaction")]
    SecretAdjacentWithoutRedaction { name: String },
}

impl EmbeddedToolCatalog {
    pub fn validate(&self) -> Result<(), CatalogValidationError> {
        let mut names = BTreeSet::new();
        for tool in &self.tools {
            if !names.insert(tool.name.clone()) {
                return Err(CatalogValidationError::DuplicateToolName {
                    name: tool.name.clone(),
                });
            }
            if tool.description.trim().is_empty() {
                return Err(CatalogValidationError::MissingDescription {
                    name: tool.name.clone(),
                });
            }
            for capability in &tool.capabilities {
                if capability.is_explicit_opt_in() && tool.approval != ApprovalPolicy::PerCall {
                    return Err(CatalogValidationError::UnsafeDefault {
                        name: tool.name.clone(),
                        capability: *capability,
                    });
                }
            }
            if tool.capabilities.contains(&EmbeddedCapability::SecretAdjacent)
                && tool.redaction == RedactionPolicy::None
            {
                return Err(CatalogValidationError::SecretAdjacentWithoutRedaction {
                    name: tool.name.clone(),
                });
            }
        }
        Ok(())
    }

    pub fn descriptors(&self) -> Result<Vec<ToolDescriptor>, CatalogValidationError> {
        self.validate()?;
        Ok(self
            .tools
            .iter()
            .map(|tool| ToolDescriptor {
                name: tool.name.clone(),
                description: tool.description.clone(),
            })
            .collect())
    }
}

impl ToolCatalog for EmbeddedToolCatalog {
    fn describe_tools(&self) -> Vec<ToolDescriptor> {
        self.descriptors().unwrap_or_default()
    }

    fn contains_tool(&self, name: &str) -> bool {
        self.tools.iter().any(|tool| tool.name == name)
    }
}

impl EmbeddedCapability {
    #[must_use]
    pub fn is_explicit_opt_in(self) -> bool {
        matches!(self, Self::Mutate | Self::Shell | Self::Network | Self::RawLog | Self::SecretAdjacent)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapabilityPack {
    pub name: &'static str,
    capabilities: BTreeSet<EmbeddedCapability>,
}

impl CapabilityPack {
    #[must_use]
    pub fn read_only() -> Self {
        Self::from_capabilities("read-only", [EmbeddedCapability::Read])
    }

    #[must_use]
    pub fn tool_user() -> Self {
        Self::from_capabilities("tool-user", [EmbeddedCapability::Read, EmbeddedCapability::Network])
    }

    #[must_use]
    pub fn operator() -> Self {
        Self::from_capabilities("operator", [
            EmbeddedCapability::Read,
            EmbeddedCapability::Mutate,
            EmbeddedCapability::Shell,
            EmbeddedCapability::Network,
            EmbeddedCapability::RawLog,
            EmbeddedCapability::SecretAdjacent,
        ])
    }

    #[must_use]
    pub fn capabilities(&self) -> Vec<EmbeddedCapability> {
        self.capabilities.iter().copied().collect()
    }

    fn from_capabilities(name: &'static str, capabilities: impl IntoIterator<Item = EmbeddedCapability>) -> Self {
        Self {
            name,
            capabilities: capabilities.into_iter().collect(),
        }
    }
}

#[derive(Debug, Default, Clone)]
pub struct CatalogToolExecutor {
    catalog: EmbeddedToolCatalog,
    outcomes: HashMap<String, VecDeque<ToolHostOutcome>>,
    limits: ToolTruncationLimits,
}

impl CatalogToolExecutor {
    #[must_use]
    pub fn new(catalog: EmbeddedToolCatalog) -> Self {
        Self {
            catalog,
            outcomes: HashMap::new(),
            limits: ToolTruncationLimits::default(),
        }
    }

    #[must_use]
    pub fn with_limits(mut self, limits: ToolTruncationLimits) -> Self {
        self.limits = limits;
        self
    }

    #[must_use]
    pub fn with_outcome(mut self, tool_name: impl Into<String>, outcome: ToolHostOutcome) -> Self {
        self.outcomes.entry(tool_name.into()).or_default().push_back(outcome);
        self
    }

    #[must_use]
    pub fn catalog(&self) -> &EmbeddedToolCatalog {
        &self.catalog
    }
}

impl ToolExecutor for CatalogToolExecutor {
    async fn execute_tool(&mut self, call: EngineToolCall) -> ToolHostOutcome {
        if let Err(error) = self.catalog.validate() {
            return ScriptedToolExecutor::text_error(error.to_string());
        }
        let Some(metadata) = self.catalog.tools.iter().find(|tool| tool.name == call.tool_name) else {
            return ToolHostOutcome::MissingTool { name: call.tool_name };
        };
        if metadata.capabilities.iter().any(|capability| capability.is_explicit_opt_in())
            && metadata.approval != ApprovalPolicy::PerCall
        {
            return ToolHostOutcome::CapabilityDenied {
                name: metadata.name.clone(),
                reason: "unsafe capability requires explicit per-call approval".to_string(),
            };
        }
        let Some(queue) = self.outcomes.get_mut(&call.tool_name) else {
            return ToolHostOutcome::MissingTool { name: call.tool_name };
        };
        let Some(outcome) = queue.pop_front() else {
            return ToolHostOutcome::MissingTool { name: call.tool_name };
        };
        match outcome {
            ToolHostOutcome::Succeeded { content, details } => {
                let mut accumulator = ToolOutputAccumulator::new(self.limits.clone());
                for block in &content {
                    if let Content::Text { text } = block {
                        accumulator.push(text.clone());
                    }
                }
                match accumulator.finish() {
                    ToolHostOutcome::Succeeded { .. } => ToolHostOutcome::Succeeded { content, details },
                    truncated @ ToolHostOutcome::Truncated { .. } => truncated,
                    other => other,
                }
            }
            other => other,
        }
    }
}

#[cfg(test)]
mod tests {
    use clankers_tool_host::ToolHostOutcome;

    use super::*;

    #[test]
    fn adapter_bricks_record_deterministic_state() {
        let mut events = MemoryEventSink::default();
        events
            .emit_engine_event(&EngineEvent::Notice {
                message: "ok".to_string(),
            })
            .unwrap();
        assert_eq!(events.events().len(), 1);

        let cancellation = AtomicCancellationSource::new("stop");
        let mut cancellation_reader = cancellation.clone();
        assert!(!cancellation_reader.is_cancelled());
        cancellation.cancel();
        assert!(cancellation_reader.is_cancelled());
        assert_eq!(cancellation_reader.cancellation_reason(), "stop");

        let mut usage = CollectingUsageObserver::default();
        usage
            .observe_usage(&UsageObservation {
                kind: clankers_engine_host::UsageObservationKind::FinalSummary,
                usage: Usage {
                    input_tokens: 1,
                    output_tokens: 2,
                    cache_creation_input_tokens: 0,
                    cache_read_input_tokens: 0,
                },
            })
            .unwrap();
        assert_eq!(usage.observations().len(), 1);
    }

    #[test]
    fn replaceable_app_owned_adapter_implements_same_event_sink_seam() {
        struct AppEvents {
            count: usize,
        }
        impl EngineEventSink for AppEvents {
            fn emit_engine_event(&mut self, _event: &EngineEvent) -> Result<(), HostAdapterError> {
                self.count += 1;
                Ok(())
            }
        }
        fn emit_once(sink: &mut impl EngineEventSink) {
            sink.emit_engine_event(&EngineEvent::BusyChanged { busy: true }).unwrap();
        }
        let mut reusable = MemoryEventSink::default();
        emit_once(&mut reusable);
        assert_eq!(reusable.events().len(), 1);
        let mut app_owned = AppEvents { count: 0 };
        emit_once(&mut app_owned);
        assert_eq!(app_owned.count, 1);
    }

    #[test]
    fn tool_catalog_metadata_converts_to_descriptors_without_runtime_startup() {
        let catalog = EmbeddedToolCatalog {
            tools: vec![EmbeddedToolMetadata {
                name: "lookup".to_string(),
                description: "Lookup product data".to_string(),
                runtime: EmbeddedToolRuntime::ProductOwned,
                capabilities: vec![EmbeddedCapability::Read],
                approval: ApprovalPolicy::Never,
                redaction: RedactionPolicy::None,
                input_schema: serde_json::json!({"type":"object"}),
            }],
        };
        let descriptors = catalog.descriptors().unwrap();
        assert_eq!(descriptors[0].name, "lookup");
        assert!(catalog.contains_tool("lookup"));
    }

    #[test]
    fn tool_catalog_validation_fails_closed() {
        let duplicate = EmbeddedToolCatalog {
            tools: vec![tool("a"), tool("a")],
        };
        assert!(matches!(duplicate.validate(), Err(CatalogValidationError::DuplicateToolName { .. })));

        let mut shell = tool("shell");
        shell.capabilities = vec![EmbeddedCapability::Shell];
        shell.approval = ApprovalPolicy::Never;
        let shell_catalog = EmbeddedToolCatalog { tools: vec![shell] };
        assert!(matches!(shell_catalog.validate(), Err(CatalogValidationError::UnsafeDefault { .. })));

        let mut secret = tool("secret");
        secret.capabilities = vec![EmbeddedCapability::SecretAdjacent];
        secret.approval = ApprovalPolicy::PerCall;
        secret.redaction = RedactionPolicy::None;
        let secret_catalog = EmbeddedToolCatalog { tools: vec![secret] };
        assert!(matches!(
            secret_catalog.validate(),
            Err(CatalogValidationError::SecretAdjacentWithoutRedaction { .. })
        ));

        let unknown = serde_json::from_value::<EmbeddedToolCatalog>(
            serde_json::json!({"tools":[{"name":"x","description":"x","runtime":"wasm"}]}),
        );
        assert!(unknown.is_err());
    }

    #[test]
    fn capability_pack_snapshots_do_not_expand_silently() {
        assert_eq!(CapabilityPack::read_only().capabilities(), vec![EmbeddedCapability::Read]);
        assert_eq!(CapabilityPack::tool_user().capabilities(), vec![
            EmbeddedCapability::Read,
            EmbeddedCapability::Network
        ]);
        assert_eq!(CapabilityPack::operator().capabilities(), vec![
            EmbeddedCapability::Read,
            EmbeddedCapability::Mutate,
            EmbeddedCapability::Shell,
            EmbeddedCapability::Network,
            EmbeddedCapability::RawLog,
            EmbeddedCapability::SecretAdjacent,
        ]);
    }

    #[test]
    fn catalog_executor_covers_missing_error_denial_and_truncation_paths() {
        let safe_catalog = EmbeddedToolCatalog {
            tools: vec![tool("lookup")],
        };
        let mut missing = CatalogToolExecutor::new(safe_catalog.clone());
        assert!(matches!(block_on(missing.execute_tool(call("unknown"))), ToolHostOutcome::MissingTool { .. }));

        let mut error_exec = CatalogToolExecutor::new(safe_catalog.clone())
            .with_outcome("lookup", ScriptedToolExecutor::text_error("boom"));
        assert!(matches!(block_on(error_exec.execute_tool(call("lookup"))), ToolHostOutcome::ToolError { .. }));

        let mut denied_tool = tool("danger");
        denied_tool.capabilities = vec![EmbeddedCapability::Shell];
        denied_tool.approval = ApprovalPolicy::Never;
        let denied_catalog = EmbeddedToolCatalog {
            tools: vec![denied_tool],
        };
        let mut denied = CatalogToolExecutor::new(denied_catalog);
        assert!(matches!(block_on(denied.execute_tool(call("danger"))), ToolHostOutcome::ToolError { .. }));

        let mut truncating = CatalogToolExecutor::new(safe_catalog)
            .with_limits(ToolTruncationLimits {
                max_bytes: 4,
                max_lines: 1,
            })
            .with_outcome("lookup", ScriptedToolExecutor::text_success("abcdef"));
        assert!(matches!(block_on(truncating.execute_tool(call("lookup"))), ToolHostOutcome::Truncated { .. }));
    }

    fn tool(name: &str) -> EmbeddedToolMetadata {
        EmbeddedToolMetadata {
            name: name.to_string(),
            description: format!("{name} tool"),
            runtime: EmbeddedToolRuntime::ProductOwned,
            capabilities: vec![EmbeddedCapability::Read],
            approval: ApprovalPolicy::Never,
            redaction: RedactionPolicy::None,
            input_schema: serde_json::json!({}),
        }
    }

    fn call(name: &str) -> EngineToolCall {
        EngineToolCall {
            call_id: EngineCorrelationId("call-1".to_string()),
            tool_name: name.to_string(),
            input: serde_json::json!({}),
        }
    }

    fn block_on<F: std::future::Future>(future: F) -> F::Output {
        use std::sync::Arc;
        use std::task::Context;
        use std::task::Poll;
        use std::task::Wake;
        use std::task::Waker;

        struct NoopWaker;
        impl Wake for NoopWaker {
            fn wake(self: Arc<Self>) {}
        }
        let waker = Waker::from(Arc::new(NoopWaker));
        let mut context = Context::from_waker(&waker);
        let mut future = Box::pin(future);
        loop {
            match future.as_mut().poll(&mut context) {
                Poll::Ready(output) => return output,
                Poll::Pending => std::thread::yield_now(),
            }
        }
    }
}
