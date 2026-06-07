//! Root-shell adapters from concrete desktop services to agent-owned ports.

use std::collections::BTreeMap;
use std::sync::Arc;

use async_trait::async_trait;
use clankers_agent::AgentCompletionRequest;
use clankers_agent::AgentHookPayload;
use clankers_agent::AgentModelError;
use clankers_agent::AgentModelResult;
use clankers_agent::AgentModelService;
use clankers_agent::AgentHookPoint;
use clankers_agent::AgentHookSafeError;
use clankers_agent::AgentHookService;
use clankers_agent::AgentHookStatus;
use clankers_agent::AgentHookUsage;
use clankers_agent::AgentHookVerdict;
use clankers_agent::AgentMemoryContextProvider;
use clankers_hooks::HookPayload;
use clankers_tool_host::ToolHookDecision;
use clankers_tool_host::ToolHookPhase;
use clankers_tool_host::ToolHookRequest;
use clankers_tool_host::ToolHookService;
use clankers_tool_host::ToolHostError;
use clankers_tool_host::ToolHostFuture;
use clankers_tool_host::ToolSearchHit;
use clankers_tool_host::ToolSearchRequest;
use clankers_tool_host::ToolSearchResult;
use clankers_tool_host::ToolSearchService;
use tokio::sync::mpsc;

#[derive(Clone)]
pub struct ProviderModelServiceAdapter {
    provider: Arc<dyn clankers_provider::Provider>,
}

impl ProviderModelServiceAdapter {
    #[must_use]
    pub fn new(provider: Arc<dyn clankers_provider::Provider>) -> Self {
        Self { provider }
    }
}

#[async_trait]
impl AgentModelService for ProviderModelServiceAdapter {
    async fn complete(
        &self,
        request: AgentCompletionRequest,
        tx: mpsc::Sender<clanker_message::streaming::StreamEvent>,
    ) -> AgentModelResult<()> {
        self.provider.complete(provider_request_from_agent(request), tx).await.map_err(agent_model_error_from_provider)
    }

    fn name(&self) -> &str {
        self.provider.name()
    }

    fn max_input_tokens(&self, model: &str) -> Option<usize> {
        self.provider.models().iter().find(|candidate| candidate.id == model).map(|candidate| candidate.max_input_tokens)
    }

    async fn reload_credentials(&self) {
        self.provider.reload_credentials().await;
    }
}

fn provider_request_from_agent(request: AgentCompletionRequest) -> clankers_provider::CompletionRequest {
    clankers_provider::CompletionRequest {
        model: request.model,
        messages: request.messages,
        system_prompt: request.system_prompt,
        max_tokens: request.max_tokens,
        temperature: request.temperature,
        tools: request.tools,
        thinking: request.thinking,
        no_cache: request.no_cache,
        cache_ttl: request.cache_ttl,
        extra_params: request.extra_params,
    }
}

fn agent_model_error_from_provider(error: clankers_provider::error::ProviderError) -> AgentModelError {
    let retryable = error.is_retryable();
    let should_compress = error.should_compress();
    let status = error.status;
    AgentModelError::new(error.message).with_status(status).retryable(retryable).should_compress(should_compress)
}

#[derive(Clone)]
pub struct DbMemoryContextProvider {
    db: clankers_db::Db,
}

impl DbMemoryContextProvider {
    #[must_use]
    pub fn new(db: clankers_db::Db) -> Self {
        Self { db }
    }
}

impl AgentMemoryContextProvider for DbMemoryContextProvider {
    fn memory_context(
        &self,
        cwd: Option<&str>,
        global_limit: Option<usize>,
        project_limit: Option<usize>,
    ) -> Option<String> {
        self.db
            .memory()
            .context_for_with_limits(cwd, global_limit, project_limit)
            .ok()
            .filter(|context| !context.is_empty())
    }
}

#[derive(Clone)]
pub struct DbMemorySearchService {
    db: clankers_db::Db,
}

impl DbMemorySearchService {
    #[must_use]
    pub fn new(db: clankers_db::Db) -> Self {
        Self { db }
    }
}

impl ToolSearchService for DbMemorySearchService {
    fn search(&self, request: ToolSearchRequest) -> std::result::Result<ToolSearchResult, ToolHostError> {
        let scope_filter = request.metadata.get("scope").map(String::as_str).unwrap_or("all");
        let entries = self.db.memory().search(&request.query).map_err(|error| ToolHostError::HostFailed {
            message: error.to_string(),
        })?;
        let hits = entries
            .into_iter()
            .filter(|entry| match scope_filter {
                "global" => matches!(entry.scope, clankers_db::memory::MemoryScope::Global),
                "project" => matches!(entry.scope, clankers_db::memory::MemoryScope::Project { .. }),
                _ => true,
            })
            .take(usize::try_from(request.limit).unwrap_or(usize::MAX))
            .enumerate()
            .map(|(index, entry)| {
                let mut metadata = BTreeMap::new();
                metadata.insert("memory_id".to_string(), entry.id.to_string());
                metadata.insert("scope".to_string(), entry.scope.to_string());
                ToolSearchHit {
                    title: entry.id.to_string(),
                    snippet: entry.text,
                    rank: u32::try_from(index + 1).unwrap_or(u32::MAX),
                    metadata,
                }
            })
            .collect();
        Ok(ToolSearchResult { hits })
    }
}

#[derive(Clone)]
pub struct HookPipelineAgentHookService {
    pipeline: Arc<clankers_hooks::HookPipeline>,
}

impl HookPipelineAgentHookService {
    #[must_use]
    pub fn new(pipeline: Arc<clankers_hooks::HookPipeline>) -> Self {
        Self { pipeline }
    }
}

#[async_trait]
impl AgentHookService for HookPipelineAgentHookService {
    async fn fire(&self, point: AgentHookPoint, payload: &AgentHookPayload) -> AgentHookVerdict {
        let hook_point = hook_point_from_agent(point);
        let payload = hook_payload_from_agent(payload);
        hook_verdict_to_agent(self.pipeline.fire(hook_point, &payload).await)
    }

    fn fire_async(&self, point: AgentHookPoint, payload: AgentHookPayload) {
        let pipeline = self.pipeline.clone();
        tokio::spawn(async move {
            let hook_point = hook_point_from_agent(point);
            let payload = hook_payload_from_agent(&payload);
            pipeline.fire(hook_point, &payload).await;
        });
    }
}

#[derive(Clone)]
pub struct HookPipelineToolHookService {
    pipeline: Arc<clankers_hooks::HookPipeline>,
    session_id: String,
}

impl HookPipelineToolHookService {
    #[must_use]
    pub fn new(pipeline: Arc<clankers_hooks::HookPipeline>, session_id: String) -> Self {
        Self { pipeline, session_id }
    }
}

impl ToolHookService for HookPipelineToolHookService {
    fn decide(
        &self,
        request: ToolHookRequest,
    ) -> ToolHostFuture<'_, std::result::Result<ToolHookDecision, ToolHostError>> {
        let pipeline = self.pipeline.clone();
        let session_id = self.session_id.clone();
        Box::pin(async move {
            let hook_point = match request.phase {
                ToolHookPhase::Before => clankers_hooks::HookPoint::PreTool,
                ToolHookPhase::After => clankers_hooks::HookPoint::PostTool,
            };
            let event_name = match request.phase {
                ToolHookPhase::Before => "pre-tool",
                ToolHookPhase::After => "post-tool",
            };
            let result_json = (request.phase == ToolHookPhase::After).then(|| request.input.clone());
            let input = if request.phase == ToolHookPhase::After {
                serde_json::json!({})
            } else {
                request.input.clone()
            };
            let payload = HookPayload::tool(
                event_name,
                &session_id,
                &request.tool_name,
                &request.call_id,
                input,
                result_json,
            );
            let decision = pipeline.fire(hook_point, &payload).await;
            Ok(match decision {
                clankers_hooks::HookVerdict::Continue => ToolHookDecision::Continue,
                clankers_hooks::HookVerdict::Modify(input) => ToolHookDecision::Modify { input },
                clankers_hooks::HookVerdict::Deny { reason } => ToolHookDecision::Deny { reason },
            })
        })
    }
}

fn hook_point_from_agent(point: AgentHookPoint) -> clankers_hooks::HookPoint {
    match point {
        AgentHookPoint::PrePrompt => clankers_hooks::HookPoint::PrePrompt,
        AgentHookPoint::PostPrompt => clankers_hooks::HookPoint::PostPrompt,
        AgentHookPoint::PreTurn => clankers_hooks::HookPoint::PreTurn,
        AgentHookPoint::PostTurn => clankers_hooks::HookPoint::PostTurn,
    }
}

fn hook_verdict_to_agent(verdict: clankers_hooks::HookVerdict) -> AgentHookVerdict {
    match verdict {
        clankers_hooks::HookVerdict::Continue => AgentHookVerdict::Continue,
        clankers_hooks::HookVerdict::Modify(value) => AgentHookVerdict::Modify(value),
        clankers_hooks::HookVerdict::Deny { reason } => AgentHookVerdict::Deny { reason },
    }
}

fn hook_payload_from_agent(payload: &AgentHookPayload) -> HookPayload {
    match &payload.data {
        clankers_agent::AgentHookData::Prompt {
            prompt_id,
            text,
            system_prompt,
            status,
            error,
        } => HookPayload::prompt_with_metadata(
            &payload.event_name,
            &payload.session_id,
            prompt_id,
            text,
            system_prompt.as_deref(),
            hook_status_from_agent(*status),
            error.as_ref().map(hook_error_from_agent),
        ),
        clankers_agent::AgentHookData::Turn {
            prompt_id,
            model,
            prompt_text,
            message_count,
            tool_call_count,
            status,
            error,
            usage,
        } => HookPayload::turn(
            &payload.event_name,
            &payload.session_id,
            prompt_id,
            model,
            prompt_text,
            *message_count,
            *tool_call_count,
            hook_status_from_agent(*status),
            error.as_ref().map(hook_error_from_agent),
            usage.as_ref().map(hook_usage_from_agent),
        ),
    }
}

fn hook_status_from_agent(status: AgentHookStatus) -> clankers_hooks::HookStatus {
    match status {
        AgentHookStatus::Pending => clankers_hooks::HookStatus::Pending,
        AgentHookStatus::Success => clankers_hooks::HookStatus::Success,
        AgentHookStatus::Denied => clankers_hooks::HookStatus::Denied,
        AgentHookStatus::Error => clankers_hooks::HookStatus::Error,
        AgentHookStatus::Cancelled => clankers_hooks::HookStatus::Cancelled,
    }
}

fn hook_error_from_agent(error: &AgentHookSafeError) -> clankers_hooks::HookSafeError {
    clankers_hooks::HookSafeError::new(&error.message, error.kind.as_deref())
}

fn hook_usage_from_agent(usage: &AgentHookUsage) -> clankers_hooks::HookUsage {
    clankers_hooks::HookUsage {
        input_tokens: usage.input_tokens,
        output_tokens: usage.output_tokens,
        cache_creation_input_tokens: usage.cache_creation_input_tokens,
        cache_read_input_tokens: usage.cache_read_input_tokens,
    }
}
