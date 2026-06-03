use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use clanker_message::ToolResultMessage;
use clanker_message::Usage;
use clankers_model_selection::cost_tracker::CostTracker;
use clankers_provider::CompletionRequest;
use clankers_provider::Provider;
use clankers_tool_host::ToolCancellationService;
use clankers_tool_host::ToolCapabilityService;
use clankers_tool_host::ToolHookService;
use clankers_tool_host::ToolHostServiceHandle;
use clankers_tool_host::ToolHostServiceKind;
use clankers_tool_host::ToolHostServices;
use clankers_tool_host::ToolInvocationContext;
use clankers_tool_host::ToolProgressSink;
use clankers_tool_host::ToolSearchService;
use clankers_tool_host::ToolStorageService;
use serde_json::Value;
use tokio::sync::broadcast;
use tokio_util::sync::CancellationToken;

use super::AgentToolSteelSubstrateConfig;
use super::CollectedResponse;
use super::execute_tools_parallel_with_substrate;
use super::stream_model_request;
use super::tool_definitions_from_tool_catalog;
use super::update_usage_tracking;
use crate::error::Result;
use crate::events::AgentEvent;
use crate::tool::ModelSwitchSlot;
use crate::tool::Tool;
use crate::tool::ToolDefinition;
use crate::tool::ToolResult as ToolExecResult;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AgentRuntimeServiceKind {
    ModelExecution,
    ToolRegistry,
    Storage,
    PromptContext,
    Hooks,
    Skills,
    Cost,
    Cancellation,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AgentRuntimeServiceOwner {
    RuntimePort,
    EngineHostAdapter,
    DesktopAdapter,
    NeutralDto,
    SafeDefault,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct AgentRuntimeServiceReceipt {
    pub(crate) kind: AgentRuntimeServiceKind,
    pub(crate) owner: AgentRuntimeServiceOwner,
    pub(crate) adapter: &'static str,
    pub(crate) reason: &'static str,
    pub(crate) convergence: &'static str,
}

pub(crate) const DESKTOP_AGENT_SERVICE_RECEIPTS: &[AgentRuntimeServiceReceipt] = &[
    AgentRuntimeServiceReceipt {
        kind: AgentRuntimeServiceKind::ModelExecution,
        owner: AgentRuntimeServiceOwner::RuntimePort,
        adapter: "ProviderModelPort",
        reason: "turn policy streams provider requests through a model port",
        convergence: "replace concrete provider construction at app edge only",
    },
    AgentRuntimeServiceReceipt {
        kind: AgentRuntimeServiceKind::ToolRegistry,
        owner: AgentRuntimeServiceOwner::RuntimePort,
        adapter: "ControllerToolPort",
        reason: "tool definitions and executions are exposed through a tool port",
        convergence: "migrate built-in tools to neutral clankers-tool-host context",
    },
    AgentRuntimeServiceReceipt {
        kind: AgentRuntimeServiceKind::Storage,
        owner: AgentRuntimeServiceOwner::DesktopAdapter,
        adapter: "ControllerToolPort",
        reason: "legacy tools receive optional database handles only inside the desktop tool adapter",
        convergence: "replace with neutral storage service traits per tool family",
    },
    AgentRuntimeServiceReceipt {
        kind: AgentRuntimeServiceKind::PromptContext,
        owner: AgentRuntimeServiceOwner::NeutralDto,
        adapter: "TurnConfig",
        reason: "turn policy receives assembled system prompt and messages as neutral DTOs",
        convergence: "move prompt assembly behind runtime prompt services",
    },
    AgentRuntimeServiceReceipt {
        kind: AgentRuntimeServiceKind::Hooks,
        owner: AgentRuntimeServiceOwner::DesktopAdapter,
        adapter: "ControllerToolPort",
        reason: "hook pipeline is invoked inside the desktop tool adapter, not engine policy",
        convergence: "replace hook payloads with reusable effect/receipt services",
    },
    AgentRuntimeServiceReceipt {
        kind: AgentRuntimeServiceKind::Skills,
        owner: AgentRuntimeServiceOwner::SafeDefault,
        adapter: "TurnConfig",
        reason: "turn loop receives already-assembled skill context or no skill service",
        convergence: "introduce explicit skill resolver service for prompt assembly",
    },
    AgentRuntimeServiceReceipt {
        kind: AgentRuntimeServiceKind::Cost,
        owner: AgentRuntimeServiceOwner::RuntimePort,
        adapter: "CostTrackerPort",
        reason: "usage accounting is routed through a cost port",
        convergence: "move cost receipts to reusable semantic event services",
    },
    AgentRuntimeServiceReceipt {
        kind: AgentRuntimeServiceKind::Cancellation,
        owner: AgentRuntimeServiceOwner::EngineHostAdapter,
        adapter: "TokenCancellationPort",
        reason: "engine-host cancellation source owns cancellation checks",
        convergence: "keep cancellation as an engine-host adapter seam",
    },
];

// Source inventory consumed by `scripts/check-lego-architecture-boundaries.rs`.
#[allow(dead_code)]
pub(crate) const NEUTRAL_TOOL_SERVICE_CONTEXT_REQUIREMENT: &str = "r[neutral-tool-service-context.service-contracts]";

#[allow(dead_code)]
pub(crate) const AGENT_CONCRETE_DEPENDENCY_DRAIN_REQUIREMENT: &str =
    "r[agent-concrete-dependency-drain.dependency-budget]";

#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum AgentConcreteDependencyFamily {
    Provider,
    StorageSearch,
    Config,
    Procmon,
    DisplayProtocol,
    Router,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AgentConcreteDependencyOwner {
    ModelAdapter,
    LegacyToolAdapter,
    AppEdgeSettingsAdapter,
    ProcessEventProjection,
    NoProductionEdge,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct AgentConcreteDependencyBudgetEntry {
    pub(crate) family: AgentConcreteDependencyFamily,
    pub(crate) crate_name: &'static str,
    pub(crate) owner: AgentConcreteDependencyOwner,
    pub(crate) production_modules: &'static [&'static str],
    pub(crate) selected_slice_status: &'static str,
    pub(crate) convergence: &'static str,
}

#[allow(dead_code)]
pub(crate) const AGENT_CONCRETE_DEPENDENCY_BUDGET: &[AgentConcreteDependencyBudgetEntry] = &[
    AgentConcreteDependencyBudgetEntry {
        family: AgentConcreteDependencyFamily::Provider,
        crate_name: "clankers-provider",
        owner: AgentConcreteDependencyOwner::ModelAdapter,
        production_modules: &[
            "crates/clankers-agent/src/lib.rs",
            "crates/clankers-agent/src/builder.rs",
            "crates/clankers-agent/src/error.rs",
            "crates/clankers-agent/src/compaction.rs",
            "crates/clankers-agent/src/turn/execution.rs",
            "crates/clankers-agent/src/turn/ports.rs",
        ],
        selected_slice_status: "remaining",
        convergence: "replace provider-native CompletionRequest/Provider with agent model request/stream ports at reusable policy seams",
    },
    AgentConcreteDependencyBudgetEntry {
        family: AgentConcreteDependencyFamily::StorageSearch,
        crate_name: "clankers-db",
        owner: AgentConcreteDependencyOwner::LegacyToolAdapter,
        production_modules: &[
            "crates/clankers-agent/src/lib.rs",
            "crates/clankers-agent/src/builder.rs",
            "crates/clankers-agent/src/tool.rs",
            "crates/clankers-agent/src/turn/execution.rs",
            "crates/clankers-agent/src/turn/mod.rs",
        ],
        selected_slice_status: "remaining",
        convergence: "complete tool storage/search migration to clankers-tool-host services and remove legacy Db/SearchIndex context fields",
    },
    AgentConcreteDependencyBudgetEntry {
        family: AgentConcreteDependencyFamily::Config,
        crate_name: "clankers-config",
        owner: AgentConcreteDependencyOwner::AppEdgeSettingsAdapter,
        production_modules: &[
            "crates/clankers-agent/src/lib.rs",
            "crates/clankers-agent/src/builder.rs",
        ],
        selected_slice_status: "Steel tool substrate, Steel turn planning, auto-compaction, context assembly, and prompt discovery settings converted to agent-owned DTOs at app edge",
        convergence: "move remaining settings-derived planning/prompt policy to agent-owned DTOs and shell adapters",
    },
    AgentConcreteDependencyBudgetEntry {
        family: AgentConcreteDependencyFamily::Procmon,
        crate_name: "clankers-procmon",
        owner: AgentConcreteDependencyOwner::ProcessEventProjection,
        production_modules: &["crates/clankers-agent/src/events.rs"],
        selected_slice_status: "remaining",
        convergence: "replace procmon-native process events with reusable process observation DTOs before AgentEvent projection",
    },
    AgentConcreteDependencyBudgetEntry {
        family: AgentConcreteDependencyFamily::DisplayProtocol,
        crate_name: "clanker-tui-types/clankers-protocol",
        owner: AgentConcreteDependencyOwner::NoProductionEdge,
        production_modules: &[],
        selected_slice_status: "absent",
        convergence: "keep display and wire protocol DTOs outside clankers-agent reusable policy",
    },
    AgentConcreteDependencyBudgetEntry {
        family: AgentConcreteDependencyFamily::Router,
        crate_name: "clanker-router",
        owner: AgentConcreteDependencyOwner::NoProductionEdge,
        production_modules: &[],
        selected_slice_status: "absent",
        convergence: "keep router composition in provider/root adapters rather than agent policy",
    },
];

#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum AgentToolServiceKind {
    LegacyToolRegistry,
    ProgressEvents,
    Cancellation,
    Hooks,
    SessionIdentity,
    Storage,
    SearchIndex,
    CapabilityGate,
    UserToolFilter,
    SteelSubstratePolicy,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AgentToolServiceOwner {
    ControllerToolPort,
    ControllerNeutralToolServices,
    LegacyToolRunner,
    LegacyToolContext,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct AgentToolServiceInventoryEntry {
    pub(crate) kind: AgentToolServiceKind,
    pub(crate) owner: AgentToolServiceOwner,
    pub(crate) field: &'static str,
    pub(crate) concrete_type: &'static str,
    pub(crate) replacement: &'static str,
    pub(crate) convergence: &'static str,
}

#[allow(dead_code)]
pub(crate) const CONTROLLER_TOOL_PORT_SERVICE_INVENTORY: &[AgentToolServiceInventoryEntry] = &[
    AgentToolServiceInventoryEntry {
        kind: AgentToolServiceKind::LegacyToolRegistry,
        owner: AgentToolServiceOwner::ControllerToolPort,
        field: "controller_tools",
        concrete_type: "HashMap<String, Arc<dyn Tool>>",
        replacement: "neutral tool catalog service",
        convergence: "migrate built-in tools behind clankers-tool-host execution contracts",
    },
    AgentToolServiceInventoryEntry {
        kind: AgentToolServiceKind::ProgressEvents,
        owner: AgentToolServiceOwner::ControllerNeutralToolServices,
        field: "progress",
        concrete_type: "ToolProgressSink",
        replacement: "neutral progress/event service",
        convergence: "replace AgentEvent progress output with semantic tool progress DTOs",
    },
    AgentToolServiceInventoryEntry {
        kind: AgentToolServiceKind::Cancellation,
        owner: AgentToolServiceOwner::ControllerNeutralToolServices,
        field: "cancellation",
        concrete_type: "ToolCancellationService",
        replacement: "neutral cancellation service",
        convergence: "thread cancellation through ToolInvocationContext instead of legacy ToolContext",
    },
    AgentToolServiceInventoryEntry {
        kind: AgentToolServiceKind::Hooks,
        owner: AgentToolServiceOwner::ControllerNeutralToolServices,
        field: "hooks",
        concrete_type: "ToolHookService",
        replacement: "neutral hook decision service",
        convergence: "translate hook continue/modify/deny at the product edge",
    },
    AgentToolServiceInventoryEntry {
        kind: AgentToolServiceKind::SessionIdentity,
        owner: AgentToolServiceOwner::ControllerNeutralToolServices,
        field: "metadata",
        concrete_type: "ToolInvocationContext metadata",
        replacement: "neutral invocation identity DTO",
        convergence: "carry session identity in ToolInvocationContext metadata",
    },
    AgentToolServiceInventoryEntry {
        kind: AgentToolServiceKind::Storage,
        owner: AgentToolServiceOwner::ControllerNeutralToolServices,
        field: "storage",
        concrete_type: "ToolStorageService",
        replacement: "neutral storage service",
        convergence: "move storage access behind explicit missing-service/error receipts",
    },
    AgentToolServiceInventoryEntry {
        kind: AgentToolServiceKind::SearchIndex,
        owner: AgentToolServiceOwner::ControllerNeutralToolServices,
        field: "search",
        concrete_type: "ToolSearchService",
        replacement: "neutral search service",
        convergence: "move search access behind explicit missing-service/error receipts",
    },
    AgentToolServiceInventoryEntry {
        kind: AgentToolServiceKind::Storage,
        owner: AgentToolServiceOwner::LegacyToolRunner,
        field: "legacy_runner",
        concrete_type: "LegacyToolRunner",
        replacement: "legacy storage/search compatibility runner",
        convergence: "remove once concrete DB/search users migrate to neutral services",
    },
    AgentToolServiceInventoryEntry {
        kind: AgentToolServiceKind::CapabilityGate,
        owner: AgentToolServiceOwner::ControllerNeutralToolServices,
        field: "capability",
        concrete_type: "ToolCapabilityService",
        replacement: "neutral capability decision service",
        convergence: "translate capability denial to semantic tool denial receipts",
    },
    AgentToolServiceInventoryEntry {
        kind: AgentToolServiceKind::UserToolFilter,
        owner: AgentToolServiceOwner::ControllerNeutralToolServices,
        field: "capability",
        concrete_type: "ToolCapabilityService",
        replacement: "neutral tool visibility policy",
        convergence: "evaluate tool visibility before legacy tool dispatch",
    },
    AgentToolServiceInventoryEntry {
        kind: AgentToolServiceKind::SteelSubstratePolicy,
        owner: AgentToolServiceOwner::LegacyToolRunner,
        field: "steel_tool_substrate",
        concrete_type: "AgentToolSteelSubstrateConfig",
        replacement: "neutral runtime policy service",
        convergence: "authorize Steel-backed tool execution through runtime policy receipts",
    },
];

#[allow(dead_code)]
pub(crate) const LEGACY_TOOL_CONTEXT_SERVICE_INVENTORY: &[AgentToolServiceInventoryEntry] = &[
    AgentToolServiceInventoryEntry {
        kind: AgentToolServiceKind::ProgressEvents,
        owner: AgentToolServiceOwner::LegacyToolContext,
        field: "event_tx",
        concrete_type: "broadcast::Sender<AgentEvent>",
        replacement: "neutral progress/event service",
        convergence: "map progress/result chunks to semantic progress DTOs before legacy adapters",
    },
    AgentToolServiceInventoryEntry {
        kind: AgentToolServiceKind::Cancellation,
        owner: AgentToolServiceOwner::LegacyToolContext,
        field: "signal",
        concrete_type: "CancellationToken",
        replacement: "neutral cancellation service",
        convergence: "pass cancellation through ToolInvocationContext",
    },
    AgentToolServiceInventoryEntry {
        kind: AgentToolServiceKind::Hooks,
        owner: AgentToolServiceOwner::LegacyToolContext,
        field: "hook_pipeline",
        concrete_type: "clankers_hooks::HookPipeline",
        replacement: "neutral hook decision service",
        convergence: "remove hook pipeline from legacy ToolContext once hook service owns decisions",
    },
    AgentToolServiceInventoryEntry {
        kind: AgentToolServiceKind::Storage,
        owner: AgentToolServiceOwner::LegacyToolContext,
        field: "db",
        concrete_type: "clankers_db::Db",
        replacement: "neutral storage service",
        convergence: "tools request storage from neutral services and fail closed when missing",
    },
    AgentToolServiceInventoryEntry {
        kind: AgentToolServiceKind::SearchIndex,
        owner: AgentToolServiceOwner::LegacyToolContext,
        field: "search_index",
        concrete_type: "clankers_db::search_index::SearchIndex",
        replacement: "neutral search service",
        convergence: "search-capable tools depend on neutral search DTOs instead of concrete index handles",
    },
];

#[async_trait]
pub(crate) trait AgentModelPort: Send + Sync {
    async fn stream_model_request(
        &self,
        request: CompletionRequest,
        event_tx: &broadcast::Sender<AgentEvent>,
        cancel: &CancellationToken,
    ) -> Result<CollectedResponse>;
}

pub(crate) struct ProviderModelPort<'a> {
    provider: &'a dyn Provider,
}

impl<'a> ProviderModelPort<'a> {
    pub(crate) fn new(provider: &'a dyn Provider) -> Self {
        Self { provider }
    }
}

#[async_trait]
impl AgentModelPort for ProviderModelPort<'_> {
    async fn stream_model_request(
        &self,
        request: CompletionRequest,
        event_tx: &broadcast::Sender<AgentEvent>,
        cancel: &CancellationToken,
    ) -> Result<CollectedResponse> {
        stream_model_request(self.provider, request, event_tx, cancel).await
    }
}

#[async_trait]
pub(crate) trait AgentToolPort: Send + Sync {
    fn tool_definitions(&self) -> Vec<ToolDefinition>;

    fn sorted_tool_names(&self) -> Vec<String>;

    async fn execute_tools(&self, tool_calls: &[(String, String, Value)]) -> Vec<ToolResultMessage>;
}

pub(crate) trait AgentToolEventSink: Send + Sync {
    fn emit(&self, event: AgentEvent);
}

#[async_trait]
pub(crate) trait LegacyToolRunner: Send + Sync {
    async fn execute_legacy_tool(&self, tool: Arc<dyn Tool>, call_id: String, input: Value) -> ToolExecResult;
}

#[derive(Clone)]
pub(crate) struct ControllerToolServices {
    pub(crate) events: Arc<dyn AgentToolEventSink>,
    pub(crate) progress: Arc<dyn ToolProgressSink>,
    pub(crate) cancellation: Arc<dyn ToolCancellationService>,
    pub(crate) storage: Option<Arc<dyn ToolStorageService>>,
    pub(crate) search: Option<Arc<dyn ToolSearchService>>,
    pub(crate) hooks: Option<Arc<dyn ToolHookService>>,
    pub(crate) capability: Option<Arc<dyn ToolCapabilityService>>,
    pub(crate) legacy_runner: Arc<dyn LegacyToolRunner>,
    pub(crate) steel_tool_substrate: Option<AgentToolSteelSubstrateConfig>,
}

impl ControllerToolServices {
    pub(crate) fn invocation_context(&self, call_id: &str) -> ToolInvocationContext {
        let mut services = ToolHostServices::empty()
            .with_service(ToolHostServiceHandle::available(ToolHostServiceKind::Progress))
            .with_service(ToolHostServiceHandle::available(ToolHostServiceKind::Cancellation));
        if self.storage.is_some() {
            services = services.with_service(ToolHostServiceHandle::available(ToolHostServiceKind::Storage));
        }
        if self.search.is_some() {
            services = services.with_service(ToolHostServiceHandle::available(ToolHostServiceKind::Search));
        }
        if self.hooks.is_some() {
            services = services.with_service(ToolHostServiceHandle::available(ToolHostServiceKind::Hooks));
        }
        if self.capability.is_some() {
            services = services.with_service(ToolHostServiceHandle::available(ToolHostServiceKind::Capability));
        }

        let mut context = ToolInvocationContext::new(call_id)
            .with_services(services)
            .with_progress_sink(self.progress.clone())
            .with_cancellation_service(self.cancellation.clone());
        if let Some(storage) = &self.storage {
            context = context.with_storage_service(storage.clone());
        }
        if let Some(search) = &self.search {
            context = context.with_search_service(search.clone());
        }
        if let Some(hooks) = &self.hooks {
            context = context.with_hook_service(hooks.clone());
        }
        if let Some(capability) = &self.capability {
            context = context.with_capability_service(capability.clone());
        }
        context
    }
}

pub(crate) struct ControllerToolPort<'a> {
    pub(crate) controller_tools: &'a HashMap<String, Arc<dyn Tool>>,
    pub(crate) services: ControllerToolServices,
}

#[async_trait]
impl AgentToolPort for ControllerToolPort<'_> {
    fn tool_definitions(&self) -> Vec<ToolDefinition> {
        tool_definitions_from_tool_catalog(self.controller_tools)
    }

    fn sorted_tool_names(&self) -> Vec<String> {
        let mut names = self.controller_tools.keys().cloned().collect::<Vec<_>>();
        names.sort();
        names
    }

    async fn execute_tools(&self, tool_calls: &[(String, String, Value)]) -> Vec<ToolResultMessage> {
        execute_tools_parallel_with_substrate(self.controller_tools, tool_calls, self.services.clone()).await
    }
}

pub(crate) trait AgentCostPort: Send + Sync {
    fn observe_turn_usage(
        &self,
        cumulative_usage: &mut Usage,
        turn_usage: &Usage,
        active_model: &str,
        event_tx: &broadcast::Sender<AgentEvent>,
    );
}

pub(crate) struct CostTrackerPort<'a> {
    cost_tracker: Option<&'a Arc<CostTracker>>,
}

impl<'a> CostTrackerPort<'a> {
    pub(crate) fn new(cost_tracker: Option<&'a Arc<CostTracker>>) -> Self {
        Self { cost_tracker }
    }
}

impl AgentCostPort for CostTrackerPort<'_> {
    fn observe_turn_usage(
        &self,
        cumulative_usage: &mut Usage,
        turn_usage: &Usage,
        active_model: &str,
        event_tx: &broadcast::Sender<AgentEvent>,
    ) {
        update_usage_tracking(cumulative_usage, turn_usage, active_model, self.cost_tracker, event_tx);
    }
}

pub(crate) trait AgentCancellationPort: Send + Sync {
    fn token(&self) -> CancellationToken;

    fn is_cancelled(&self) -> bool {
        self.token().is_cancelled()
    }
}

pub(crate) struct TokenCancellationPort {
    cancel: CancellationToken,
}

impl TokenCancellationPort {
    pub(crate) fn new(cancel: CancellationToken) -> Self {
        Self { cancel }
    }
}

impl AgentCancellationPort for TokenCancellationPort {
    fn token(&self) -> CancellationToken {
        self.cancel.clone()
    }
}

pub(crate) struct AgentRuntimeServices<'a> {
    pub(crate) model: &'a dyn AgentModelPort,
    pub(crate) tools: &'a dyn AgentToolPort,
    pub(crate) cost: &'a dyn AgentCostPort,
    pub(crate) cancellation: &'a dyn AgentCancellationPort,
    pub(crate) events: &'a broadcast::Sender<AgentEvent>,
    pub(crate) model_switch_slot: Option<&'a ModelSwitchSlot>,
    pub(crate) service_receipts: &'a [AgentRuntimeServiceReceipt],
}

impl AgentRuntimeServices<'_> {
    pub(crate) fn has_service_kind(&self, kind: AgentRuntimeServiceKind) -> bool {
        self.service_receipts.iter().any(|receipt| receipt.kind == kind)
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::*;

    fn inventory_kinds(entries: &[AgentToolServiceInventoryEntry]) -> BTreeSet<AgentToolServiceKind> {
        entries.iter().map(|entry| entry.kind).collect()
    }

    fn inventory_fields(entries: &[AgentToolServiceInventoryEntry]) -> BTreeSet<&'static str> {
        entries.iter().map(|entry| entry.field).collect()
    }

    fn dependency_families(entries: &[AgentConcreteDependencyBudgetEntry]) -> BTreeSet<AgentConcreteDependencyFamily> {
        entries.iter().map(|entry| entry.family).collect()
    }

    #[test]
    fn concrete_dependency_budget_names_current_agent_edges_and_selected_config_slice() {
        let families = dependency_families(AGENT_CONCRETE_DEPENDENCY_BUDGET);

        assert_eq!(
            AGENT_CONCRETE_DEPENDENCY_DRAIN_REQUIREMENT,
            "r[agent-concrete-dependency-drain.dependency-budget]"
        );
        for family in [
            AgentConcreteDependencyFamily::Provider,
            AgentConcreteDependencyFamily::StorageSearch,
            AgentConcreteDependencyFamily::Config,
            AgentConcreteDependencyFamily::Procmon,
            AgentConcreteDependencyFamily::DisplayProtocol,
            AgentConcreteDependencyFamily::Router,
        ] {
            assert!(families.contains(&family), "missing concrete dependency budget for {family:?}");
        }

        let config_entry = AGENT_CONCRETE_DEPENDENCY_BUDGET
            .iter()
            .find(|entry| entry.family == AgentConcreteDependencyFamily::Config)
            .expect("config dependency budget entry");
        assert_eq!(config_entry.owner, AgentConcreteDependencyOwner::AppEdgeSettingsAdapter);
        assert!(config_entry.selected_slice_status.contains("AgentToolSteelSubstrateSettings"));
        assert!(!config_entry.production_modules.contains(&"crates/clankers-agent/src/turn/steel_tool_substrate.rs"));
        assert!(AGENT_CONCRETE_DEPENDENCY_BUDGET.iter().all(|entry| {
            !entry.crate_name.is_empty() && !entry.selected_slice_status.is_empty() && !entry.convergence.is_empty()
        }));
    }

    #[test]
    fn controller_tool_services_build_neutral_invocation_context() {
        let (event_tx, _rx) = tokio::sync::broadcast::channel(16);
        let services = ControllerToolServices::from_concrete(
            event_tx,
            CancellationToken::new(),
            None,
            "session".to_string(),
            None,
            None,
            Some(vec!["read".to_string()]),
            None,
        );

        let context = services.invocation_context("call-1");

        assert!(context.services.is_available(ToolHostServiceKind::Progress));
        assert!(context.services.is_available(ToolHostServiceKind::Cancellation));
        assert!(context.services.is_available(ToolHostServiceKind::Capability));
        assert!(!context.services.is_available(ToolHostServiceKind::Hooks));
        assert!(
            context
                .progress
                .emit(clankers_tool_host::ToolProgressEvent::new(
                    "call-1",
                    clankers_tool_host::ToolProgressKind::Progress,
                    "visible progress",
                ))
                .is_ok()
        );
        assert!(context.capability_service.is_some());
        assert!(context.cancellation_service.is_some());
        assert!(context.hooks.is_none());
    }

    #[test]
    fn controller_tool_port_service_inventory_names_current_concrete_edges() {
        let kinds = inventory_kinds(CONTROLLER_TOOL_PORT_SERVICE_INVENTORY);
        let fields = inventory_fields(CONTROLLER_TOOL_PORT_SERVICE_INVENTORY);

        assert_eq!(NEUTRAL_TOOL_SERVICE_CONTEXT_REQUIREMENT, "r[neutral-tool-service-context.service-contracts]");
        for kind in [
            AgentToolServiceKind::LegacyToolRegistry,
            AgentToolServiceKind::ProgressEvents,
            AgentToolServiceKind::Cancellation,
            AgentToolServiceKind::Hooks,
            AgentToolServiceKind::SessionIdentity,
            AgentToolServiceKind::Storage,
            AgentToolServiceKind::SearchIndex,
            AgentToolServiceKind::CapabilityGate,
            AgentToolServiceKind::UserToolFilter,
            AgentToolServiceKind::SteelSubstratePolicy,
        ] {
            assert!(kinds.contains(&kind), "missing ControllerToolPort service inventory for {kind:?}");
        }
        for field in [
            "controller_tools",
            "progress",
            "cancellation",
            "storage",
            "search",
            "hooks",
            "metadata",
            "legacy_runner",
            "capability",
            "steel_tool_substrate",
        ] {
            assert!(fields.contains(field), "missing ControllerToolPort field inventory for {field}");
        }
        assert!(CONTROLLER_TOOL_PORT_SERVICE_INVENTORY.iter().all(|entry| {
            let expected_owner = match entry.field {
                "controller_tools" => AgentToolServiceOwner::ControllerToolPort,
                "legacy_runner" | "steel_tool_substrate" => AgentToolServiceOwner::LegacyToolRunner,
                _ => AgentToolServiceOwner::ControllerNeutralToolServices,
            };
            entry.owner == expected_owner
                && !entry.concrete_type.is_empty()
                && !entry.replacement.is_empty()
                && !entry.convergence.is_empty()
        }));
    }

    #[test]
    fn legacy_tool_context_inventory_names_storage_search_hooks_progress_and_cancellation() {
        let kinds = inventory_kinds(LEGACY_TOOL_CONTEXT_SERVICE_INVENTORY);
        let fields = inventory_fields(LEGACY_TOOL_CONTEXT_SERVICE_INVENTORY);

        for kind in [
            AgentToolServiceKind::ProgressEvents,
            AgentToolServiceKind::Cancellation,
            AgentToolServiceKind::Hooks,
            AgentToolServiceKind::Storage,
            AgentToolServiceKind::SearchIndex,
        ] {
            assert!(kinds.contains(&kind), "missing legacy ToolContext service inventory for {kind:?}");
        }
        for field in ["event_tx", "signal", "hook_pipeline", "db", "search_index"] {
            assert!(fields.contains(field), "missing legacy ToolContext field inventory for {field}");
        }
        assert!(LEGACY_TOOL_CONTEXT_SERVICE_INVENTORY.iter().all(|entry| {
            entry.owner == AgentToolServiceOwner::LegacyToolContext
                && !entry.concrete_type.is_empty()
                && !entry.replacement.is_empty()
                && !entry.convergence.is_empty()
        }));
    }
}
