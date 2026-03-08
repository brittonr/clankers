//! Agent struct and state

pub mod builder;
pub mod compaction;
pub mod context;
pub mod events;
pub mod system_prompt;
pub mod ttsr;
pub mod turn;

use std::collections::HashMap;
use std::sync::Arc;

use chrono::Utc;
use tokio::sync::broadcast;
use tokio_util::sync::CancellationToken;

use self::events::AgentEvent;
use self::turn::TurnConfig;
use crate::config::model_roles::ModelRoles;
use crate::config::settings::Settings;
use crate::db::Db;
use crate::error::Result;
use crate::provider::Provider;
use crate::provider::ThinkingConfig;
use crate::provider::ThinkingLevel;
use crate::provider::message::*;
use crate::model_selection::cost_tracker::CostTracker;
use crate::model_selection::policy::RoutingPolicy;
use crate::model_selection::signals::{ComplexitySignals, ToolCallSummary};
use crate::tools::Tool;
use crate::tools::switch_model::ModelSwitchSlot;

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
        }
    }

    /// Attach a database handle to this agent.
    ///
    /// When set, the agent will:
    /// - Inject relevant memories into the system prompt each turn
    /// - Record per-turn token usage
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

        let _ = self.event_tx.send(AgentEvent::AgentStart);

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

        let _ = self.event_tx.send(AgentEvent::BeforeAgentStart {
            prompt: text.to_string(),
            system_prompt: ctx.system_prompt.clone(),
        });

        let config = TurnConfig {
            model: self.model.clone(),
            system_prompt: ctx.system_prompt,
            max_tokens: Some(self.settings.max_tokens),
            temperature: None,
            thinking: self.thinking.clone(),
            max_turns: 25,
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
        )
        .await;

        // Sync model switch if tool requested it
        self.sync_model_switch();

        let _ = self.event_tx.send(AgentEvent::AgentEnd {
            messages: self.messages.clone(),
        });

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
        let _ = self.event_tx.send(AgentEvent::UserInput {
            text: text.to_string(),
            agent_msg_count,
        });
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
            let _ = self.event_tx.send(AgentEvent::SessionCompaction {
                compacted_count: result.compacted_count,
                tokens_saved: result.tokens_saved,
            });
            tracing::info!(
                "Auto-compacted {} messages, saved ~{} tokens",
                result.compacted_count,
                result.tokens_saved,
            );
        }
    }

    /// Select model based on complexity signals, returning orchestration plan if needed
    fn select_model_for_turn(
        &mut self,
        text: &str,
    ) -> Result<Option<crate::model_selection::orchestration::OrchestrationPlan>> {
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
                let _ = self.event_tx.send(AgentEvent::ModelChange {
                    from: old,
                    to: new_model,
                    reason: selection.reason.to_string(),
                });
            }
        }

        Ok(None)
    }

    /// Prepare context for turn execution
    fn prepare_turn_context(&self, max_input: usize) -> context::AgentContext {
        let system_prompt_with_memory = self.system_prompt_with_memory();
        context::build_context(&self.messages, &system_prompt_with_memory, max_input)
    }

    /// Sync model switch from tool-requested slot
    fn sync_model_switch(&mut self) {
        if let Some(slot) = &self.model_switch_slot && let Some(new_model) = slot.lock().take() {
            self.model = new_model;
        }
    }

    /// Build the system prompt with memories appended (if db is available).
    ///
    /// Reads global + project-scoped memories from redb and appends them
    /// to the base system prompt. This is called every turn so newly saved
    /// memories appear immediately.
    fn system_prompt_with_memory(&self) -> String {
        let Some(db) = &self.db else {
            return self.system_prompt.clone();
        };

        // Derive project path from cwd (best-effort)
        let cwd = std::env::current_dir().ok();
        let cwd_str = cwd.as_ref().and_then(|p| p.to_str());

        match db.memory().context_for(cwd_str) {
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
    ///
    /// This allows external code to cancel operations directly (e.g. from
    /// a `tokio::select!` branch) without going through the command channel.
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
        let _ = self.event_tx.send(AgentEvent::ModelChange {
            from: old,
            to: model,
            reason: "user_request".to_string(),
        });
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
    /// Used when branching to rewind to a fork point.
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
        self.thinking = level.to_config();
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
    pub fn tool_definitions(&self) -> Vec<crate::tools::ToolDefinition> {
        self.tools.values().map(|t| t.definition().clone()).collect()
    }

    /// Get a reference to the provider (e.g. to reload credentials after login).
    pub fn provider(&self) -> &Arc<dyn Provider> {
        &self.provider
    }

    /// Run an orchestrated multi-phase turn.
    ///
    /// Each phase uses a different model and system prompt suffix. The output
    /// from one phase becomes context for the next via the conversation history
    /// (the assistant message from phase N is visible to phase N+1).
    async fn execute_orchestrated_turn(
        &mut self,
        user_text: &str,
        plan: crate::model_selection::orchestration::OrchestrationPlan,
    ) -> crate::error::Result<()> {
        let total_phases = plan.phases.len();
        tracing::info!(
            "Starting orchestrated turn: {} ({} phases)",
            plan.pattern,
            total_phases,
        );

        let base_system_prompt = self.system_prompt_with_memory();
        let model_info = self.provider.models().iter().find(|m| m.id == self.model).cloned();
        let max_input = model_info.map(|m| m.max_input_tokens).unwrap_or(200_000);

        for (phase_idx, phase) in plan.phases.iter().enumerate() {
            if self.cancel.is_cancelled() {
                return Err(crate::error::Error::Cancelled);
            }

            // Resolve phase model
            let phase_model = self.model_roles.resolve(&phase.role, &self.model);
            let old_model = std::mem::replace(&mut self.model, phase_model.clone());
            if phase_model != old_model {
                let _ = self.event_tx.send(AgentEvent::ModelChange {
                    from: old_model.clone(),
                    to: phase_model.clone(),
                    reason: format!(
                        "orchestration_phase({}/{}:{})",
                        phase_idx + 1,
                        total_phases,
                        phase.label,
                    ),
                });
            }

            // Build phase system prompt
            let phase_system = format!("{}{}", base_system_prompt, phase.system_suffix);
            let ctx = context::build_context(&self.messages, &phase_system, max_input);

            let _ = self.event_tx.send(AgentEvent::BeforeAgentStart {
                prompt: if phase_idx == 0 {
                    user_text.to_string()
                } else {
                    format!("[Orchestration phase {}/{}] {}", phase_idx + 1, total_phases, phase.label)
                },
                system_prompt: ctx.system_prompt.clone(),
            });

            // Run turn loop for this phase
            let config = TurnConfig {
                model: phase_model,
                system_prompt: ctx.system_prompt,
                max_tokens: Some(self.settings.max_tokens),
                temperature: None,
                thinking: self.thinking.clone(),
                max_turns: if phase_idx == 0 { 25 } else { 10 }, // later phases are shorter
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
            )
            .await;

            // If the agent switched models during the phase, sync state
            if let Some(slot) = &self.model_switch_slot && let Some(new_model) = slot.lock().take() {
                self.model = new_model;
            }

            result?;

            tracing::info!(
                "Orchestration phase {}/{} ({}) complete",
                phase_idx + 1,
                total_phases,
                phase.label,
            );
        }

        let _ = self.event_tx.send(AgentEvent::AgentEnd {
            messages: self.messages.clone(),
        });

        Ok(())
    }

    /// Extract recent tool call summaries from conversation history
    /// for complexity analysis. Looks at the last 5 messages.
    fn recent_tool_summaries(&self) -> Vec<ToolCallSummary> {
        let mut summaries = Vec::new();
        let start_index = self.messages.len().saturating_sub(5);

        for msg in &self.messages[start_index..] {
            if let AgentMessage::Assistant(asst) = msg {
                for content in &asst.content {
                    if let Content::ToolUse { name, .. } = content && let Some(policy) = &self.routing_policy {
                        summaries.push(ToolCallSummary {
                            tool_name: name.clone(),
                            complexity: policy.classify_tool(name),
                        });
                    }
                }
            } else if let AgentMessage::ToolResult(tool_result) = msg && let Some(policy) = &self.routing_policy {
                summaries.push(ToolCallSummary {
                    tool_name: tool_result.tool_name.clone(),
                    complexity: policy.classify_tool(&tool_result.tool_name),
                });
            }
        }

        summaries
    }
}
