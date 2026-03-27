//! Agent core — turn loop, event bus, tool interface, context management

pub mod builder;
pub mod compaction;
pub mod context;
pub mod error;
pub mod events;
pub mod system_prompt;
pub mod tool;
pub mod ttsr;
pub mod turn;

use std::collections::HashMap;
use std::sync::Arc;

use chrono::Utc;
use clankers_config::model_roles::ModelRoles;
use clankers_config::settings::Settings;
use clankers_db::Db;
use clankers_model_selection::cost_tracker::CostTracker;
use clankers_model_selection::orchestration;
use clankers_model_selection::policy::RoutingPolicy;
use clankers_model_selection::signals::ComplexitySignals;
use clankers_model_selection::signals::ToolCallSummary;
use clankers_provider::Provider;
use clankers_provider::ThinkingConfig;
use clankers_provider::ThinkingLevel;
use clankers_provider::message::*;
use tokio::sync::broadcast;
use tokio_util::sync::CancellationToken;

pub use self::error::AgentError;
pub use self::error::Result;
use self::events::AgentEvent;
pub use self::tool::CapabilityGate;
pub use self::tool::ModelSwitchSlot;
pub use self::tool::Tool;
pub use self::tool::ToolContext;
pub use self::tool::ToolDefinition;
pub use self::tool::ToolResult;
pub use self::tool::ToolResultContent;
pub use self::tool::model_switch_slot;
use self::turn::TurnConfig;

/// The main agent that manages the conversation loop
pub struct Agent {
    /// The LLM provider
    provider: Arc<dyn Provider>,
    /// Available tools by name
    tools: HashMap<String, Arc<dyn Tool>>,
    /// Conversation messages
    messages: Vec<AgentMessage>,
    /// Event broadcast channel
    event_tx: broadcast::Sender<AgentEvent>,
    /// Cancellation token for the current operation
    cancel: CancellationToken,
    /// Settings
    settings: Settings,
    /// Current model ID
    model: String,
    /// System prompt
    system_prompt: String,
    /// Extended thinking configuration
    thinking: Option<ThinkingConfig>,
    /// Current thinking level
    thinking_level: ThinkingLevel,
    /// Persistent database handle (memory, usage, history, etc.)
    db: Option<Db>,
    /// Routing policy for multi-model conversations
    routing_policy: Option<RoutingPolicy>,
    /// Model roles for resolving role names to model IDs
    model_roles: ModelRoles,
    /// Cost tracker for budget enforcement
    cost_tracker: Option<Arc<CostTracker>>,
    /// Shared slot for agent-initiated model switching
    model_switch_slot: Option<ModelSwitchSlot>,
    /// Hook pipeline for lifecycle/tool/git hooks
    hook_pipeline: Option<Arc<clankers_hooks::HookPipeline>>,
    /// Session ID for hook payloads
    session_id: String,
    /// Capability gate for tool call authorization (None = full access).
    /// Set at session creation from UCAN token + settings. Immutable.
    capability_gate: Option<Arc<dyn tool::CapabilityGate>>,
    /// User-adjustable tool filter (None = no additional restriction).
    /// Checked after capability_gate. Can only be narrowed within the
    /// session's capability ceiling — never escalated.
    user_tool_filter: Option<Vec<String>>,
}

impl Agent {
    /// Create a new agent with the given provider and tools
    pub fn new(
        provider: Arc<dyn Provider>,
        tools: Vec<Arc<dyn Tool>>,
        settings: Settings,
        model: String,
        system_prompt: String,
    ) -> Self {
        let (event_tx, _) = broadcast::channel(1024);
        let tool_map: HashMap<String, Arc<dyn Tool>> =
            tools.into_iter().map(|t| (t.definition().name.clone(), t)).collect();

        Self {
            provider,
            tools: tool_map,
            messages: Vec::new(),
            event_tx,
            cancel: CancellationToken::new(),
            settings,
            model,
            system_prompt,
            thinking: None,
            thinking_level: ThinkingLevel::Off,
            db: None,
            routing_policy: None,
            model_roles: ModelRoles::default(),
            cost_tracker: None,
            model_switch_slot: None,
            hook_pipeline: None,
            session_id: String::new(),
            capability_gate: None,
            user_tool_filter: None,
        }
    }

    /// Attach a database handle to this agent.
    pub fn with_db(mut self, db: Db) -> Self {
        self.db = Some(db);
        self
    }

    /// Get the database handle, if attached.
    pub fn db(&self) -> Option<&Db> {
        self.db.as_ref()
    }

    /// Set the routing policy for multi-model conversations
    pub fn with_routing_policy(mut self, policy: RoutingPolicy) -> Self {
        self.routing_policy = Some(policy);
        self
    }

    /// Set the model roles for resolving role names to model IDs
    pub fn with_model_roles(mut self, roles: ModelRoles) -> Self {
        self.model_roles = roles;
        self
    }

    /// Set the cost tracker for budget enforcement
    pub fn with_cost_tracker(mut self, tracker: Arc<CostTracker>) -> Self {
        self.cost_tracker = Some(tracker);
        self
    }

    /// Get the cost tracker, if attached.
    pub fn cost_tracker(&self) -> Option<&Arc<CostTracker>> {
        self.cost_tracker.as_ref()
    }

    /// Set the model switch slot for agent-initiated switching
    pub fn with_model_switch_slot(mut self, slot: ModelSwitchSlot) -> Self {
        self.model_switch_slot = Some(slot);
        self
    }

    /// Attach a hook pipeline for lifecycle/tool/git hooks
    pub fn with_hook_pipeline(mut self, pipeline: Arc<clankers_hooks::HookPipeline>) -> Self {
        self.hook_pipeline = Some(pipeline);
        self
    }

    /// Set the session ID (used in hook payloads)
    pub fn set_session_id(&mut self, id: String) {
        self.session_id = id;
    }

    /// Attach a capability gate for tool call authorization.
    pub fn with_capability_gate(mut self, gate: Arc<dyn tool::CapabilityGate>) -> Self {
        self.capability_gate = Some(gate);
        self
    }

    /// Set or clear the user-adjustable tool filter.
    ///
    /// This is a second layer checked after the capability gate.
    /// The controller validates that filters don't exceed the session's
    /// capability ceiling before calling this.
    pub fn set_user_tool_filter(&mut self, filter: Option<Vec<String>>) {
        self.user_tool_filter = filter;
    }

    /// Build output truncation config from settings
    fn output_truncation_config(&self) -> clanker_loop::OutputTruncationConfig {
        clanker_loop::OutputTruncationConfig {
            max_bytes: self.settings.max_output_bytes,
            max_lines: self.settings.max_output_lines,
            enabled: true,
        }
    }

    /// Subscribe to agent events
    pub fn subscribe(&self) -> broadcast::Receiver<AgentEvent> {
        self.event_tx.subscribe()
    }

    /// Get a clone of the event sender (for wiring up tools that emit progress events)
    pub fn event_sender(&self) -> broadcast::Sender<AgentEvent> {
        self.event_tx.clone()
    }

    /// Replace the agent's tools (consuming self and returning a new Agent)
    pub fn with_tools(mut self, tools: Vec<Arc<dyn Tool>>) -> Self {
        self.tools = tools.into_iter().map(|t| (t.definition().name.clone(), t)).collect();
        self
    }

    /// Replace the active tool set (hot-reload for tool toggles).
    pub fn set_tools(&mut self, tools: Vec<Arc<dyn Tool>>) {
        self.tools = tools.into_iter().map(|t| (t.definition().name.clone(), t)).collect();
    }

    /// Get the active tools.
    pub fn tools(&self) -> Vec<&Arc<dyn Tool>> {
        self.tools.values().collect()
    }

    /// Remove the last user+assistant exchange from history.
    pub fn pop_last_exchange(&mut self) {
        // Remove from the end: last assistant, then last user
        if let Some(pos) = self.messages.iter().rposition(|m| matches!(m, AgentMessage::Assistant(_))) {
            self.messages.truncate(pos);
        }
        if let Some(pos) = self.messages.iter().rposition(|m| matches!(m, AgentMessage::User(_))) {
            self.messages.truncate(pos);
        }
    }

    /// Compact messages by truncating stale tool results.
    pub fn compact_messages(&mut self) {
        self.messages = crate::context::compact_stale_tool_results(&self.messages, 3);
    }

    /// Run the agent with a user prompt and optional image content blocks
    pub async fn prompt_with_images(&mut self, text: &str, images: Vec<Content>) -> Result<()> {
        let mut content = vec![Content::Text { text: text.to_string() }];
        content.extend(images);
        self.prompt_with_content(text, content).await
    }

    /// Run the agent with a user prompt
    pub async fn prompt(&mut self, text: &str) -> Result<()> {
        let content = vec![Content::Text { text: text.to_string() }];
        self.prompt_with_content(text, content).await
    }

    /// Internal: run agent with arbitrary user content blocks
    async fn prompt_with_content(&mut self, text: &str, content: Vec<Content>) -> Result<()> {
        // Create and append user message
        self.append_user_message(text, content);

        self.event_tx.send(AgentEvent::AgentStart).ok();

        // Get model context limits
        let max_input = self.get_max_input_tokens();

        // Auto-compact if needed
        self.handle_auto_compaction(max_input).await;

        // Select model based on complexity signals
        if let Some(plan) = self.select_model_for_turn(text)? {
            return self.execute_orchestrated_turn(text, plan).await;
        }

        // Prepare context and run turn
        let ctx = self.prepare_turn_context(max_input);

        self.event_tx.send(AgentEvent::BeforeAgentStart {
            prompt: text.to_string(),
            system_prompt: ctx.system_prompt.clone(),
        }).ok();

        let config = TurnConfig {
            model: self.model.clone(),
            system_prompt: ctx.system_prompt,
            max_tokens: Some(self.settings.max_tokens),
            temperature: None,
            thinking: self.thinking.clone(),
            max_turns: 25,
            output_truncation: self.output_truncation_config(),
            no_cache: self.settings.no_cache,
            cache_ttl: self.settings.cache_ttl.clone(),
        };

        let result = turn::run_turn_loop(
            self.provider.as_ref(),
            &self.tools,
            &mut self.messages,
            &config,
            &self.event_tx,
            self.cancel.clone(),
            self.cost_tracker.as_ref(),
            self.model_switch_slot.as_ref(),
            self.hook_pipeline.clone(),
            &self.session_id,
            self.db.clone(),
            self.capability_gate.as_ref(),
            self.user_tool_filter.as_ref(),
        )
        .await;

        // Sync model switch if tool requested it
        self.sync_model_switch();

        self.event_tx.send(AgentEvent::AgentEnd {
            messages: self.messages.clone(),
        }).ok();

        result
    }

    /// Append a user message to the conversation
    fn append_user_message(&mut self, text: &str, content: Vec<Content>) {
        let user_msg = AgentMessage::User(UserMessage {
            id: MessageId::generate(),
            content,
            timestamp: Utc::now(),
        });

        let agent_msg_count = self.messages.len();
        self.event_tx.send(AgentEvent::UserInput {
            text: text.to_string(),
            agent_msg_count,
        }).ok();
        self.messages.push(user_msg);
    }

    /// Get the maximum input token count for the current model
    fn get_max_input_tokens(&self) -> usize {
        self.provider
            .models()
            .iter()
            .find(|m| m.id == self.model)
            .map(|m| m.max_input_tokens)
            .unwrap_or(200_000)
    }

    /// Handle auto-compaction if messages exceed threshold
    async fn handle_auto_compaction(&mut self, max_input: usize) {
        let auto_compact_config = compaction::AutoCompactConfig::default();
        if !compaction::should_auto_compact(&self.messages, max_input, &auto_compact_config) {
            return;
        }

        tracing::info!(
            "Auto-compacting: messages exceed {}% of {} token context window",
            (auto_compact_config.threshold * 100.0) as u32,
            max_input,
        );

        let result = compaction::compact_with_llm(
            &self.messages,
            max_input,
            auto_compact_config.keep_recent,
            self.provider.as_ref(),
            &self.model,
        )
        .await;

        if result.compacted_count > 0 {
            self.messages = result.messages;
            self.event_tx.send(AgentEvent::SessionCompaction {
                compacted_count: result.compacted_count,
                tokens_saved: result.tokens_saved,
            }).ok();
            tracing::info!("Auto-compacted {} messages, saved ~{} tokens", result.compacted_count, result.tokens_saved,);
        }
    }

    /// Select model based on complexity signals, returning orchestration plan if needed
    fn select_model_for_turn(&mut self, text: &str) -> Result<Option<orchestration::OrchestrationPlan>> {
        let Some(policy) = &self.routing_policy else {
            return Ok(None);
        };

        let signals = ComplexitySignals {
            token_count: text.len() / 4, // rough token estimate
            recent_tools: self.recent_tool_summaries(),
            keywords: policy.extract_keywords(text),
            user_hint: policy.parse_user_hint(text),
            current_cost: self.cost_tracker.as_ref().map_or(0.0, |ct| ct.total_cost()),
            prompt_text: Some(text.to_string()),
        };
        let selection = policy.select_model(&signals);

        // If orchestration is planned, return it
        if let Some(plan) = selection.orchestration {
            return Ok(Some(plan));
        }

        // Switch model if needed
        if selection.role != "default" {
            let new_model = self.model_roles.resolve(&selection.role, &self.model);
            if new_model != self.model {
                let old = std::mem::replace(&mut self.model, new_model.clone());
                self.event_tx.send(AgentEvent::ModelChange {
                    from: old,
                    to: new_model,
                    reason: selection.reason.to_string(),
                }).ok();
            }
        }

        Ok(None)
    }

    /// Prepare context for turn execution
    fn prepare_turn_context(&self, max_input: usize) -> context::AgentContext {
        let system_prompt_with_memory = self.system_prompt_with_memory();
        // Skip tool result compaction when prompt caching is active (default).
        // Compaction changes the token prefix, invalidating cache hits.
        // Caching saves ~90% on reads vs compaction's ~23% context reduction.
        let is_compact = self.settings.no_cache;
        context::build_context(&self.messages, &system_prompt_with_memory, max_input, is_compact)
    }

    /// Sync model switch from tool-requested slot
    fn sync_model_switch(&mut self) {
        if let Some(slot) = &self.model_switch_slot
            && let Some(new_model) = slot.lock().take()
        {
            self.model = new_model;
        }
    }

    /// Build the system prompt with memories appended (if db is available).
    fn system_prompt_with_memory(&self) -> String {
        let Some(db) = &self.db else {
            return self.system_prompt.clone();
        };

        // Derive project path from cwd (best-effort)
        let cwd = std::env::current_dir().ok();
        let cwd_str = cwd.as_ref().and_then(|p| p.to_str());

        let global_limit = Some(self.settings.memory.global_char_limit);
        let project_limit = Some(self.settings.memory.project_char_limit);

        match db
            .memory()
            .context_for_with_limits(cwd_str, global_limit, project_limit)
        {
            Ok(memory_context) if !memory_context.is_empty() => {
                format!("{}\n\n{}", self.system_prompt, memory_context)
            }
            _ => self.system_prompt.clone(),
        }
    }

    /// Abort the current operation
    pub fn abort(&self) {
        self.cancel.cancel();
    }

    /// Get a clone of the current cancellation token.
    pub fn cancel_token(&self) -> CancellationToken {
        self.cancel.clone()
    }

    /// Get a new cancellation token (resets abort state)
    pub fn reset_cancel(&mut self) {
        self.cancel = CancellationToken::new();
    }

    /// Get the current conversation messages
    pub fn messages(&self) -> &[AgentMessage] {
        &self.messages
    }

    /// Get the current model ID
    pub fn model(&self) -> &str {
        &self.model
    }

    /// Change the model
    pub fn set_model(&mut self, model: String) {
        let old = std::mem::replace(&mut self.model, model.clone());
        self.event_tx.send(AgentEvent::ModelChange {
            from: old,
            to: model,
            reason: "user_request".to_string(),
        }).ok();
    }

    /// Seed the agent with pre-existing messages (for session resume)
    pub fn seed_messages(&mut self, messages: Vec<AgentMessage>) {
        self.messages = messages;
    }

    /// Clear conversation history
    pub fn clear_messages(&mut self) {
        self.messages.clear();
    }

    /// Truncate conversation history to the first `n` messages.
    pub fn truncate_messages(&mut self, n: usize) {
        self.messages.truncate(n);
    }

    /// Toggle extended thinking on/off. Returns the new state.
    pub fn toggle_thinking(&mut self, budget_tokens: usize) -> bool {
        if self.thinking.as_ref().is_some_and(|t| t.enabled) {
            self.thinking = None;
            self.thinking_level = ThinkingLevel::Off;
            false
        } else {
            self.thinking = Some(ThinkingConfig {
                enabled: true,
                budget_tokens: Some(budget_tokens),
            });
            self.thinking_level = ThinkingLevel::from_budget(budget_tokens);
            true
        }
    }

    /// Set thinking to a specific level. Returns the new level.
    pub fn set_thinking_level(&mut self, level: ThinkingLevel) -> ThinkingLevel {
        self.thinking_level = level;
        self.thinking = clankers_provider::thinking_level_to_config(level);
        level
    }

    /// Cycle to the next thinking level. Returns the new level.
    pub fn cycle_thinking_level(&mut self) -> ThinkingLevel {
        let next = self.thinking_level.next();
        self.set_thinking_level(next)
    }

    /// Get the current thinking level
    pub fn thinking_level(&self) -> ThinkingLevel {
        self.thinking_level
    }

    /// Check if thinking is currently enabled
    pub fn is_thinking_enabled(&self) -> bool {
        self.thinking_level.is_enabled()
    }

    /// Get the current system prompt
    pub fn system_prompt(&self) -> &str {
        &self.system_prompt
    }

    /// Replace the system prompt
    pub fn set_system_prompt(&mut self, prompt: String) {
        self.system_prompt = prompt;
    }

    /// Get available tool definitions
    pub fn tool_definitions(&self) -> Vec<ToolDefinition> {
        self.tools.values().map(|t| t.definition().clone()).collect()
    }

    /// Get a reference to the provider
    pub fn provider(&self) -> &Arc<dyn Provider> {
        &self.provider
    }

    /// Run an orchestrated multi-phase turn.
    async fn execute_orchestrated_turn(
        &mut self,
        user_text: &str,
        plan: orchestration::OrchestrationPlan,
    ) -> Result<()> {
        let total_phases = plan.phases.len();
        tracing::info!("Starting orchestrated turn: {} ({} phases)", plan.pattern, total_phases,);

        let base_system_prompt = self.system_prompt_with_memory();
        let model_info = self.provider.models().iter().find(|m| m.id == self.model).cloned();
        let max_input = model_info.map(|m| m.max_input_tokens).unwrap_or(200_000);

        for (phase_idx, phase) in plan.phases.iter().enumerate() {
            if self.cancel.is_cancelled() {
                return Err(AgentError::Cancelled);
            }

            // Resolve phase model
            let phase_model = self.model_roles.resolve(&phase.role, &self.model);
            let old_model = std::mem::replace(&mut self.model, phase_model.clone());
            if phase_model != old_model {
                self.event_tx.send(AgentEvent::ModelChange {
                    from: old_model.clone(),
                    to: phase_model.clone(),
                    reason: format!("orchestration_phase({}/{}:{})", phase_idx + 1, total_phases, phase.label,),
                }).ok();
            }

            // Build phase system prompt
            let phase_system = format!("{}{}", base_system_prompt, phase.system_suffix);
            let is_compact = self.settings.no_cache;
            let ctx = context::build_context(&self.messages, &phase_system, max_input, is_compact);

            self.event_tx.send(AgentEvent::BeforeAgentStart {
                prompt: if phase_idx == 0 {
                    user_text.to_string()
                } else {
                    format!("[Orchestration phase {}/{}] {}", phase_idx + 1, total_phases, phase.label)
                },
                system_prompt: ctx.system_prompt.clone(),
            }).ok();

            // Run turn loop for this phase
            let config = TurnConfig {
                model: phase_model,
                system_prompt: ctx.system_prompt,
                max_tokens: Some(self.settings.max_tokens),
                temperature: None,
                thinking: self.thinking.clone(),
                max_turns: if phase_idx == 0 { 25 } else { 10 },
                output_truncation: self.output_truncation_config(),
                no_cache: self.settings.no_cache,
                cache_ttl: self.settings.cache_ttl.clone(),
            };

            let result = turn::run_turn_loop(
                self.provider.as_ref(),
                &self.tools,
                &mut self.messages,
                &config,
                &self.event_tx,
                self.cancel.clone(),
                self.cost_tracker.as_ref(),
                self.model_switch_slot.as_ref(),
                self.hook_pipeline.clone(),
                &self.session_id,
                self.db.clone(),
                self.capability_gate.as_ref(),
                self.user_tool_filter.as_ref(),
            )
            .await;

            // If the agent switched models during the phase, sync state
            if let Some(slot) = &self.model_switch_slot
                && let Some(new_model) = slot.lock().take()
            {
                self.model = new_model;
            }

            result?;

            tracing::info!("Orchestration phase {}/{} ({}) complete", phase_idx + 1, total_phases, phase.label,);
        }

        self.event_tx.send(AgentEvent::AgentEnd {
            messages: self.messages.clone(),
        }).ok();

        Ok(())
    }

    /// Extract recent tool call summaries from conversation history
    fn recent_tool_summaries(&self) -> Vec<ToolCallSummary> {
        let mut summaries = Vec::new();
        let start_index = self.messages.len().saturating_sub(5);

        for msg in &self.messages[start_index..] {
            if let AgentMessage::Assistant(asst) = msg {
                for content in &asst.content {
                    if let Content::ToolUse { name, .. } = content
                        && let Some(policy) = &self.routing_policy
                    {
                        summaries.push(ToolCallSummary {
                            tool_name: name.clone(),
                            complexity: policy.classify_tool(name),
                        });
                    }
                }
            } else if let AgentMessage::ToolResult(tool_result) = msg
                && let Some(policy) = &self.routing_policy
            {
                summaries.push(ToolCallSummary {
                    tool_name: tool_result.tool_name.clone(),
                    complexity: policy.classify_tool(&tool_result.tool_name),
                });
            }
        }

        summaries
    }
}
