use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use clankers_model_selection::cost_tracker::CostTracker;
use clankers_provider::CompletionRequest;
use clankers_provider::Provider;
use clankers_provider::Usage;
use clankers_provider::message::ToolResultMessage;
use serde_json::Value;
use tokio::sync::broadcast;
use tokio_util::sync::CancellationToken;

use super::CollectedResponse;
use super::execute_tools_parallel;
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
        execute_tools_parallel(
            self.controller_tools,
            tool_calls,
            self.event_tx,
            self.cancel.clone(),
            self.hook_pipeline.clone(),
            self.session_id,
            self.db.clone(),
            self.capability_gate.clone(),
            self.user_tool_filter.clone(),
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
