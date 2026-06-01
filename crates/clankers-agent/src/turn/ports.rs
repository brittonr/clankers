use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use clanker_message::ToolResultMessage;
use clanker_message::Usage;
use clankers_model_selection::cost_tracker::CostTracker;
use clankers_provider::CompletionRequest;
use clankers_provider::Provider;
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
use crate::tool::CapabilityGate;
use crate::tool::ModelSwitchSlot;
use crate::tool::Tool;
use crate::tool::ToolDefinition;

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
        owner: AgentToolServiceOwner::ControllerToolPort,
        field: "event_tx",
        concrete_type: "broadcast::Sender<AgentEvent>",
        replacement: "neutral progress/event service",
        convergence: "replace AgentEvent progress output with semantic tool progress DTOs",
    },
    AgentToolServiceInventoryEntry {
        kind: AgentToolServiceKind::Cancellation,
        owner: AgentToolServiceOwner::ControllerToolPort,
        field: "cancel",
        concrete_type: "CancellationToken",
        replacement: "neutral cancellation service",
        convergence: "thread cancellation through ToolInvocationContext instead of legacy ToolContext",
    },
    AgentToolServiceInventoryEntry {
        kind: AgentToolServiceKind::Hooks,
        owner: AgentToolServiceOwner::ControllerToolPort,
        field: "hook_pipeline",
        concrete_type: "clankers_hooks::HookPipeline",
        replacement: "neutral hook decision service",
        convergence: "translate hook continue/modify/deny at the product edge",
    },
    AgentToolServiceInventoryEntry {
        kind: AgentToolServiceKind::SessionIdentity,
        owner: AgentToolServiceOwner::ControllerToolPort,
        field: "session_id",
        concrete_type: "&str",
        replacement: "neutral invocation identity DTO",
        convergence: "carry session identity in ToolInvocationContext metadata",
    },
    AgentToolServiceInventoryEntry {
        kind: AgentToolServiceKind::Storage,
        owner: AgentToolServiceOwner::ControllerToolPort,
        field: "db",
        concrete_type: "clankers_db::Db",
        replacement: "neutral storage/search service",
        convergence: "move storage access behind explicit missing-service/error receipts",
    },
    AgentToolServiceInventoryEntry {
        kind: AgentToolServiceKind::CapabilityGate,
        owner: AgentToolServiceOwner::ControllerToolPort,
        field: "capability_gate",
        concrete_type: "CapabilityGate",
        replacement: "neutral capability decision service",
        convergence: "translate capability denial to semantic tool denial receipts",
    },
    AgentToolServiceInventoryEntry {
        kind: AgentToolServiceKind::UserToolFilter,
        owner: AgentToolServiceOwner::ControllerToolPort,
        field: "user_tool_filter",
        concrete_type: "Vec<String>",
        replacement: "neutral tool visibility policy",
        convergence: "evaluate tool visibility before legacy tool dispatch",
    },
    AgentToolServiceInventoryEntry {
        kind: AgentToolServiceKind::SteelSubstratePolicy,
        owner: AgentToolServiceOwner::ControllerToolPort,
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

pub(crate) struct ControllerToolPort<'a> {
    pub(crate) controller_tools: &'a HashMap<String, Arc<dyn Tool>>,
    pub(crate) event_tx: &'a broadcast::Sender<AgentEvent>,
    pub(crate) cancel: CancellationToken,
    pub(crate) hook_pipeline: Option<Arc<clankers_hooks::HookPipeline>>,
    pub(crate) session_id: &'a str,
    pub(crate) db: Option<clankers_db::Db>,
    pub(crate) capability_gate: Option<Arc<dyn CapabilityGate>>,
    pub(crate) user_tool_filter: Option<Vec<String>>,
    pub(crate) steel_tool_substrate: Option<AgentToolSteelSubstrateConfig>,
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
        execute_tools_parallel_with_substrate(
            self.controller_tools,
            tool_calls,
            self.event_tx,
            self.cancel.clone(),
            self.hook_pipeline.clone(),
            self.session_id,
            self.db.clone(),
            self.capability_gate.clone(),
            self.user_tool_filter.clone(),
            self.steel_tool_substrate.clone(),
        )
        .await
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
            AgentToolServiceKind::CapabilityGate,
            AgentToolServiceKind::UserToolFilter,
            AgentToolServiceKind::SteelSubstratePolicy,
        ] {
            assert!(kinds.contains(&kind), "missing ControllerToolPort service inventory for {kind:?}");
        }
        for field in [
            "controller_tools",
            "event_tx",
            "cancel",
            "hook_pipeline",
            "session_id",
            "db",
            "capability_gate",
            "user_tool_filter",
            "steel_tool_substrate",
        ] {
            assert!(fields.contains(field), "missing ControllerToolPort field inventory for {field}");
        }
        assert!(CONTROLLER_TOOL_PORT_SERVICE_INVENTORY.iter().all(|entry| {
            entry.owner == AgentToolServiceOwner::ControllerToolPort
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
