//! Root-shell adapters from concrete desktop services to agent-owned ports.

use std::collections::BTreeMap;
use std::sync::Arc;

use async_trait::async_trait;
use clanker_message::transcript::AgentMessage;
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
pub struct HookPipelineControllerHookService {
    pipeline: Arc<clankers_hooks::HookPipeline>,
}

impl HookPipelineControllerHookService {
    #[must_use]
    pub fn new(pipeline: Arc<clankers_hooks::HookPipeline>) -> Self {
        Self { pipeline }
    }
}

#[async_trait]
impl clankers_controller::ControllerHookService for HookPipelineControllerHookService {
    async fn fire(
        &self,
        point: clankers_controller::ControllerHookPoint,
        payload: &clankers_controller::ControllerHookPayload,
    ) -> clankers_controller::ControllerHookVerdict {
        let hook_point = controller_hook_point_to_hooks(point);
        let payload = controller_hook_payload_to_hooks(payload);
        controller_hook_verdict_from_hooks(self.pipeline.fire(hook_point, &payload).await)
    }

    fn fire_async(
        &self,
        point: clankers_controller::ControllerHookPoint,
        payload: clankers_controller::ControllerHookPayload,
    ) {
        let pipeline = self.pipeline.clone();
        tokio::spawn(async move {
            let hook_point = controller_hook_point_to_hooks(point);
            let payload = controller_hook_payload_to_hooks(&payload);
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

#[derive(Clone)]
pub struct DbControllerPersistenceService {
    db: clankers_db::Db,
    search_index: Option<Arc<clankers_db::search_index::SearchIndex>>,
}

impl DbControllerPersistenceService {
    #[must_use]
    pub fn new(db: clankers_db::Db) -> Self {
        Self { db, search_index: None }
    }

    #[must_use]
    pub fn with_search_index(mut self, search_index: Arc<clankers_db::search_index::SearchIndex>) -> Self {
        self.search_index = Some(search_index);
        self
    }
}

impl clankers_controller::ControllerPersistenceService for DbControllerPersistenceService {
    fn index_messages(&self, session_id: &str, messages: &[AgentMessage]) {
        let Some(search_index) = &self.search_index else {
            return;
        };
        index_messages_for_search(search_index, session_id, messages);
    }

    fn store_compaction_summary_tool_result(&self, session_id: &str, summary: &str) {
        if let Err(error) = persist_compaction_summary_tool_result(&self.db, session_id, summary) {
            tracing::warn!("failed to persist compaction summary tool result: {error}");
        }
    }
}

fn index_messages_for_search(
    search_index: &clankers_db::search_index::SearchIndex,
    session_id: &str,
    messages: &[AgentMessage],
) {
    let mut batch: Vec<(&str, String, &str, String, i64)> = Vec::new();

    for msg in messages {
        let id = msg.id().to_string();
        let role = msg.role();
        let timestamp = msg.timestamp().timestamp();

        let text = match msg {
            AgentMessage::User(m) => extract_text(&m.content),
            AgentMessage::Assistant(m) => extract_text(&m.content),
            AgentMessage::ToolResult(m) => extract_text(&m.content),
            AgentMessage::BashExecution(m) => format!("{} {} {}", m.command, m.stdout, m.stderr),
            _ => continue,
        };

        if !text.trim().is_empty() {
            batch.push((session_id, id, role, text, timestamp));
        }
    }

    if batch.is_empty() {
        return;
    }

    let refs: Vec<(&str, &str, &str, &str, i64)> = batch
        .iter()
        .map(|(sid, id, role, text, ts)| (*sid, id.as_str(), *role, text.as_str(), *ts))
        .collect();

    if let Err(error) = search_index.index_messages_batch(&refs) {
        tracing::warn!("failed to index messages for search: {error}");
    }
}

fn extract_text(content: &[clanker_message::Content]) -> String {
    content
        .iter()
        .filter_map(|content| match content {
            clanker_message::Content::Text { text } => Some(text.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn persist_compaction_summary_tool_result(
    db: &clankers_db::Db,
    session_id: &str,
    summary: &str,
) -> clankers_db::error::Result<()> {
    let entry = clankers_db::tool_results::StoredToolResult {
        session_id: session_id.to_string(),
        call_id: "compaction-summary".to_string(),
        tool_name: "compaction-summary".to_string(),
        content_text: summary.to_string(),
        has_image: false,
        is_error: false,
        byte_count: summary.len(),
        line_count: summary.lines().count(),
    };
    db.tool_results().store(&entry)
}

fn controller_hook_point_to_hooks(point: clankers_controller::ControllerHookPoint) -> clankers_hooks::HookPoint {
    match point {
        clankers_controller::ControllerHookPoint::PrePrompt => clankers_hooks::HookPoint::PrePrompt,
        clankers_controller::ControllerHookPoint::PostPrompt => clankers_hooks::HookPoint::PostPrompt,
        clankers_controller::ControllerHookPoint::SessionStart => clankers_hooks::HookPoint::SessionStart,
        clankers_controller::ControllerHookPoint::SessionEnd => clankers_hooks::HookPoint::SessionEnd,
        clankers_controller::ControllerHookPoint::PreTurn => clankers_hooks::HookPoint::PreTurn,
        clankers_controller::ControllerHookPoint::TurnStart => clankers_hooks::HookPoint::TurnStart,
        clankers_controller::ControllerHookPoint::TurnEnd => clankers_hooks::HookPoint::TurnEnd,
        clankers_controller::ControllerHookPoint::PostTurn => clankers_hooks::HookPoint::PostTurn,
        clankers_controller::ControllerHookPoint::ModelChange => clankers_hooks::HookPoint::ModelChange,
    }
}

fn controller_hook_payload_to_hooks(payload: &clankers_controller::ControllerHookPayload) -> HookPayload {
    match &payload.data {
        clankers_controller::ControllerHookData::Prompt {
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
            controller_hook_status_to_hooks(*status),
            error.as_ref().map(controller_hook_error_to_hooks),
        ),
        clankers_controller::ControllerHookData::Turn {
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
            controller_hook_status_to_hooks(*status),
            error.as_ref().map(controller_hook_error_to_hooks),
            usage.as_ref().map(controller_hook_usage_to_hooks),
        ),
        clankers_controller::ControllerHookData::Session { session_id } => {
            HookPayload::session(&payload.event_name, session_id)
        }
        clankers_controller::ControllerHookData::ModelChange { from, to, reason } => {
            HookPayload::model_change(&payload.event_name, &payload.session_id, from, to, reason)
        }
        clankers_controller::ControllerHookData::Empty => HookPayload::empty(&payload.event_name, &payload.session_id),
    }
}

fn controller_hook_status_to_hooks(status: clankers_controller::ControllerHookStatus) -> clankers_hooks::HookStatus {
    match status {
        clankers_controller::ControllerHookStatus::Pending => clankers_hooks::HookStatus::Pending,
        clankers_controller::ControllerHookStatus::Success => clankers_hooks::HookStatus::Success,
        clankers_controller::ControllerHookStatus::Denied => clankers_hooks::HookStatus::Denied,
        clankers_controller::ControllerHookStatus::Cancelled => clankers_hooks::HookStatus::Cancelled,
        clankers_controller::ControllerHookStatus::Error => clankers_hooks::HookStatus::Error,
    }
}

fn controller_hook_error_to_hooks(error: &clankers_controller::ControllerHookSafeError) -> clankers_hooks::HookSafeError {
    clankers_hooks::HookSafeError::new(&error.message, error.kind.as_deref())
}

fn controller_hook_usage_to_hooks(usage: &clankers_controller::ControllerHookUsage) -> clankers_hooks::HookUsage {
    clankers_hooks::HookUsage {
        input_tokens: usage.input_tokens,
        output_tokens: usage.output_tokens,
        cache_creation_input_tokens: usage.cache_creation_input_tokens,
        cache_read_input_tokens: usage.cache_read_input_tokens,
    }
}

fn controller_hook_verdict_from_hooks(verdict: clankers_hooks::HookVerdict) -> clankers_controller::ControllerHookVerdict {
    match verdict {
        clankers_hooks::HookVerdict::Continue => clankers_controller::ControllerHookVerdict::Continue,
        clankers_hooks::HookVerdict::Modify(value) => clankers_controller::ControllerHookVerdict::Modify(value),
        clankers_hooks::HookVerdict::Deny { reason } => clankers_controller::ControllerHookVerdict::Deny { reason },
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
