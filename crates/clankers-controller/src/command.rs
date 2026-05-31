//! Command handling and prompt execution.
//!
//! Contains the main command dispatch and prompt processing logic.

use clanker_message::AgentMessage;
use clanker_message::AssistantMessage;
use clanker_message::Content;
use clanker_message::MessageId;
use clanker_message::StopReason;
use clanker_message::UserMessage;
use clankers_agent::AgentError;
use clankers_core::CompletionStatus;
use clankers_core::CoreFailure;
use clankers_core::CoreInput;
use clankers_core::CoreOutcome;
use clankers_core::CoreThinkingLevel;
use clankers_core::CoreThinkingLevelInput;
use clankers_core::DisabledToolsUpdate;
use clankers_core::LoopRequest;
use clankers_core::PromptRequest;
use clankers_core::ToolFilterApplied;
use clankers_protocol::DaemonEvent;
use clankers_protocol::ImageData;
use clankers_protocol::SerializedMessage;
use clankers_protocol::SessionCommand;
use clankers_provider::message::Content as ProviderContent;
use tracing::info;
use tracing::warn;

use crate::SessionController;
use crate::convert::semantic_event_to_daemon_event;
use crate::runtime_adapter::ControllerRuntimeAdapter;
use crate::runtime_adapter::RuntimeControlRequest;
use crate::runtime_adapter::RuntimePromptCompletion;
use crate::runtime_adapter::RuntimePromptRequest;

impl SessionController {
    /// Process a command from a client. Requires daemon mode (agent owned).
    ///
    /// In embedded mode, commands are handled by the agent_task;
    /// the controller only processes events via `feed_event()`.
    pub async fn handle_command(&mut self, cmd: SessionCommand) {
        if self.agent.is_none() {
            warn!("handle_command called in embedded mode (no agent)");
            return;
        }

        match cmd {
            SessionCommand::Prompt { text, images } => {
                self.handle_prompt(text, images).await;
            }
            SessionCommand::Abort => {
                if let Some(ref mut agent) = self.agent {
                    agent.abort();
                }
                self.busy = false;
                self.core_state.busy = false;
                self.core_state.pending_prompt = None;
                self.emit(DaemonEvent::SystemMessage {
                    text: "Operation cancelled".to_string(),
                    is_error: false,
                });
            }
            SessionCommand::ResetCancel => {
                if let Some(ref mut agent) = self.agent {
                    agent.reset_cancel();
                }
            }
            SessionCommand::SetModel { model } => {
                self.set_model_from_command(model, "user request");
            }
            SessionCommand::ClearHistory => {
                if !self.ensure_session_manage_authorized("clear_history") {
                    return;
                }
                if let Some(ref mut agent) = self.agent {
                    agent.clear_messages();
                }
                self.emit(DaemonEvent::SystemMessage {
                    text: "History cleared".to_string(),
                    is_error: false,
                });
            }
            SessionCommand::TruncateMessages { count } => {
                if !self.ensure_session_manage_authorized("truncate_messages") {
                    return;
                }
                if let Some(ref mut agent) = self.agent {
                    agent.truncate_messages(count);
                }
                self.emit(DaemonEvent::SystemMessage {
                    text: format!("Truncated to {count} messages"),
                    is_error: false,
                });
            }
            SessionCommand::SetThinkingLevel { level } => {
                if self.ensure_session_manage_authorized("set_thinking_level") {
                    self.handle_set_thinking_level(level);
                }
            }
            SessionCommand::CycleThinkingLevel => {
                if self.ensure_session_manage_authorized("cycle_thinking_level") {
                    self.handle_cycle_thinking_level();
                }
            }
            SessionCommand::SeedMessages { messages } => {
                if !self.ensure_session_manage_authorized("seed_messages") {
                    return;
                }
                let agent_messages = self.convert_seed_messages(&messages);
                let count = agent_messages.len();
                if let Some(ref mut agent) = self.agent {
                    agent.seed_messages(agent_messages);
                }
                self.emit(DaemonEvent::SystemMessage {
                    text: format!("Seeded {count} messages"),
                    is_error: false,
                });
            }
            SessionCommand::SetSystemPrompt { prompt } => {
                if !self.ensure_session_manage_authorized("set_system_prompt") {
                    return;
                }
                if let Some(ref mut agent) = self.agent {
                    agent.set_system_prompt(prompt);
                }
                self.emit(DaemonEvent::SystemMessage {
                    text: "System prompt updated".to_string(),
                    is_error: false,
                });
            }
            SessionCommand::GetSystemPrompt => {
                let prompt = self.agent.as_ref().map(|a| a.system_prompt().to_string()).unwrap_or_default();
                self.emit(DaemonEvent::SystemPromptResponse { prompt });
            }
            SessionCommand::SwitchAccount { account } => {
                self.emit(DaemonEvent::SystemMessage {
                    text: format!("Account switch to '{account}' requested"),
                    is_error: false,
                });
            }
            SessionCommand::SetDisabledTools { tools } => {
                if self.ensure_session_manage_authorized("set_disabled_tools") {
                    self.handle_set_disabled_tools(tools);
                }
            }
            SessionCommand::ConfirmBash { request_id, approved } => {
                if !self.bash_confirms.respond(&request_id, approved) {
                    warn!("bash confirm response for unknown request: {request_id}");
                }
            }
            SessionCommand::TodoResponse { request_id, response } => {
                if !self.todo_confirms.respond(&request_id, response) {
                    warn!("todo response for unknown request: {request_id}");
                }
            }
            SessionCommand::RewriteAndPrompt { text } => {
                if !self.ensure_session_manage_authorized("rewrite_prompt") || !self.ensure_prompt_authorized(&text) {
                    return;
                }
                // Remove the last user message and re-prompt
                if let Some(ref mut agent) = self.agent {
                    agent.pop_last_exchange();
                }
                self.handle_prompt(text, vec![]).await;
            }
            SessionCommand::CompactHistory => {
                if !self.ensure_session_manage_authorized("compact_history") {
                    return;
                }
                if let Some(ref mut agent) = self.agent {
                    let result = agent.compact_messages();
                    self.emit(DaemonEvent::SessionCompaction {
                        compacted_count: result.compacted_count,
                        tokens_saved: result.tokens_saved,
                    });
                }
            }
            SessionCommand::StartLoop {
                iterations,
                prompt,
                break_condition,
            } => {
                if self.ensure_session_manage_authorized("start_loop") {
                    self.handle_start_loop(iterations, prompt, break_condition);
                }
            }
            SessionCommand::StopLoop => {
                if self.ensure_session_manage_authorized("stop_loop") {
                    self.handle_stop_loop();
                }
            }
            SessionCommand::SetAutoTest { enabled, command } => {
                if !self.ensure_session_manage_authorized("set_auto_test") {
                    return;
                }
                self.auto_test_enabled = enabled;
                if let Some(cmd) = command.clone() {
                    self.auto_test_command = Some(cmd);
                }
                self.core_state.auto_test_enabled = self.auto_test_enabled;
                self.core_state.auto_test_command = self.auto_test_command.clone();
                self.emit(DaemonEvent::AutoTestChanged {
                    enabled,
                    command: self.auto_test_command.clone(),
                });
            }
            SessionCommand::GetToolList => {
                self.emit(DaemonEvent::ToolList {
                    tools: self.current_tool_infos(),
                });
            }
            SessionCommand::SlashCommand { command, args } => {
                info!("slash command: /{command} {args}");
                self.handle_slash_command_sync(&command, &args);
            }
            SessionCommand::ReplayHistory => {
                self.replay_history();
            }
            SessionCommand::GetCapabilities => {
                self.emit(DaemonEvent::Capabilities {
                    capabilities: self.capabilities.clone(),
                });
            }
            SessionCommand::SetCapabilities { capabilities } => {
                if !self.ensure_session_manage_authorized("set_capabilities") {
                    return;
                }
                // Validate against ceiling: clamped result must match request
                let effective = crate::capability::clamp_capabilities(&self.capability_ceiling, &capabilities);
                if effective != capabilities {
                    // User tried to escalate beyond their ceiling
                    let ceiling_desc =
                        self.capability_ceiling.as_ref().map(|c| c.join(", ")).unwrap_or_else(|| "none".to_string());
                    self.emit(DaemonEvent::SystemMessage {
                        text: format!("Cannot set capabilities: request exceeds session ceiling [{}]", ceiling_desc,),
                        is_error: true,
                    });
                } else {
                    self.capabilities = capabilities.clone();
                    if let Some(ref mut agent) = self.agent {
                        agent.set_user_tool_filter(capabilities.clone());
                    }
                    let desc = capabilities.as_ref().map(|c| c.join(", ")).unwrap_or_else(|| "full access".to_string());
                    self.emit(DaemonEvent::SystemMessage {
                        text: format!("Capabilities updated: {desc}"),
                        is_error: false,
                    });
                    self.emit(DaemonEvent::Capabilities {
                        capabilities: self.capabilities.clone(),
                    });
                }
            }
            SessionCommand::Disconnect => {
                tracing::debug!("client disconnected");
            }
            SessionCommand::GetPlugins => {
                // Handled by the daemon's agent process actor, not the controller.
                // If we get here in embedded mode, emit an empty list.
                self.emit(DaemonEvent::PluginList { plugins: vec![] });
            }
        }
    }

    /// Process a command and synchronously flush controller events while a prompt is running.
    ///
    /// The normal `handle_command` API preserves accumulated events until callers drain them.
    /// Daemon actors use this variant so text deltas are broadcast while `Agent::prompt` is
    /// still awaiting the provider stream instead of after the whole turn completes.
    pub async fn handle_command_with_streaming_events(
        &mut self,
        cmd: SessionCommand,
        on_events: &mut (dyn FnMut(Vec<DaemonEvent>) + Send),
    ) {
        match cmd {
            SessionCommand::Prompt { text, images } => {
                self.handle_prompt_inner(text, images, Some(on_events)).await;
            }
            SessionCommand::RewriteAndPrompt { text } => {
                if !self.ensure_session_manage_authorized("rewrite_prompt") || !self.ensure_prompt_authorized(&text) {
                    let events = self.drain_events();
                    if !events.is_empty() {
                        on_events(events);
                    }
                    return;
                }
                if let Some(ref mut agent) = self.agent {
                    agent.pop_last_exchange();
                }
                self.handle_prompt_inner(text, vec![], Some(on_events)).await;
            }
            other => {
                self.handle_command(other).await;
                let events = self.drain_events();
                if !events.is_empty() {
                    on_events(events);
                }
            }
        }
    }

    fn emit_authorization_error(&mut self, error: AgentError) {
        self.emit(DaemonEvent::SystemMessage {
            text: error.to_string(),
            is_error: true,
        });
    }

    fn ensure_session_manage_authorized(&mut self, action: &str) -> bool {
        let Some(agent) = self.agent.as_ref() else {
            return true;
        };
        match agent.check_session_manage_authorization(action) {
            Ok(()) => true,
            Err(error) => {
                self.emit_authorization_error(error);
                false
            }
        }
    }

    fn ensure_prompt_authorized(&mut self, text: &str) -> bool {
        let Some(agent) = self.agent.as_ref() else {
            return true;
        };
        match agent.check_prompt_authorization(text) {
            Ok(()) => true,
            Err(error) => {
                self.emit_authorization_error(error);
                false
            }
        }
    }

    fn set_model_from_command(&mut self, model: String, reason: &str) -> bool {
        let from = self.model.clone();
        let authorization_error = self.agent.as_mut().and_then(|agent| agent.try_set_model(model.clone()).err());
        if let Some(error) = authorization_error {
            self.emit_authorization_error(error);
            return false;
        }
        self.model = model.clone();
        self.emit(DaemonEvent::ModelChanged {
            from,
            to: model,
            reason: reason.to_string(),
        });
        true
    }

    fn handle_set_thinking_level(&mut self, level: String) {
        let input = CoreInput::SetThinkingLevel {
            requested: Self::parse_core_thinking_level_input(&level),
        };

        match clankers_core::reduce(&self.core_state, &input) {
            CoreOutcome::Transitioned { next_state, effects } => {
                self.apply_core_state(next_state);
                let thinking_change = self.execute_thinking_effects(effects);
                self.emit(DaemonEvent::SystemMessage {
                    text: format!(
                        "Thinking: {} → {}",
                        Self::thinking_label(thinking_change.previous),
                        Self::thinking_label(thinking_change.current)
                    ),
                    is_error: false,
                });
            }
            CoreOutcome::Rejected {
                error: clankers_core::CoreError::InvalidThinkingLevel { raw },
                ..
            } => {
                self.emit(DaemonEvent::SystemMessage {
                    text: format!("Unknown thinking level: {raw}"),
                    is_error: true,
                });
            }
            CoreOutcome::Rejected { .. } => unreachable!("thinking-level input should only reject as invalid"),
        }
    }

    fn handle_cycle_thinking_level(&mut self) {
        match clankers_core::reduce(&self.core_state, &CoreInput::CycleThinkingLevel) {
            CoreOutcome::Transitioned { next_state, effects } => {
                self.apply_core_state(next_state);
                let _thinking_change = self.execute_thinking_effects(effects);
                self.emit(DaemonEvent::SystemMessage {
                    text: "Thinking level cycled".to_string(),
                    is_error: false,
                });
            }
            CoreOutcome::Rejected { .. } => unreachable!("cycle thinking level should not reject"),
        }
    }

    fn handle_set_disabled_tools(&mut self, tools: Vec<String>) {
        let input = CoreInput::SetDisabledTools(DisabledToolsUpdate {
            requested_disabled_tools: tools.clone(),
        });

        match clankers_core::reduce(&self.core_state, &input) {
            CoreOutcome::Transitioned { next_state, effects } => {
                self.apply_core_state(next_state);
                if self.execute_tool_filter_request_effects(effects) {
                    self.emit(DaemonEvent::SystemMessage {
                        text: format!("Disabled tools updated: {}", tools.join(", ")),
                        is_error: false,
                    });
                }
            }
            CoreOutcome::Rejected {
                error: clankers_core::CoreError::ToolFilterStillPending,
                ..
            } => {
                self.emit(DaemonEvent::SystemMessage {
                    text: "Disabled tools update rejected: tool-filter rebuild still pending".to_string(),
                    is_error: true,
                });
            }
            CoreOutcome::Rejected { .. } => unreachable!("disabled-tool update should only reject on a pending filter"),
        }
    }

    pub(crate) fn apply_tool_filter_feedback(&mut self, feedback: ToolFilterApplied) -> bool {
        let input = CoreInput::ToolFilterApplied(feedback);
        match clankers_core::reduce(&self.core_state, &input) {
            CoreOutcome::Transitioned { next_state, effects } => {
                self.apply_core_state(next_state);
                self.execute_tool_filter_feedback_effects(effects);
                true
            }
            CoreOutcome::Rejected { .. } => {
                self.emit(DaemonEvent::SystemMessage {
                    text: "Disabled tools update rejected".to_string(),
                    is_error: true,
                });
                false
            }
        }
    }

    fn handle_start_loop(&mut self, iterations: u32, prompt: String, break_condition: Option<String>) {
        let input = CoreInput::StartLoop(LoopRequest {
            loop_id: format!("loop-{}", self.session_id),
            prompt_text: prompt,
            max_iterations: iterations,
            break_condition,
        });

        match clankers_core::reduce(&self.core_state, &input) {
            CoreOutcome::Transitioned { next_state, effects } => {
                self.apply_core_state(next_state);
                self.execute_start_loop_effects(effects);
            }
            CoreOutcome::Rejected {
                error: clankers_core::CoreError::LoopAlreadyActive,
                ..
            } => {
                self.emit(DaemonEvent::SystemMessage {
                    text: "Loop already active".to_string(),
                    is_error: true,
                });
            }
            CoreOutcome::Rejected {
                error: clankers_core::CoreError::LoopFollowUpStillPending,
                ..
            } => {
                self.emit(DaemonEvent::SystemMessage {
                    text: "Loop control rejected: loop follow-up still pending".to_string(),
                    is_error: true,
                });
            }
            CoreOutcome::Rejected { .. } => unreachable!("start-loop should reject only on active/pending loop state"),
        }
    }

    fn handle_stop_loop(&mut self) {
        match clankers_core::reduce(&self.core_state, &CoreInput::StopLoop) {
            CoreOutcome::Transitioned { next_state, effects } => {
                self.apply_core_state(next_state);
                self.execute_stop_loop_effects(effects);
            }
            CoreOutcome::Rejected {
                error: clankers_core::CoreError::LoopNotActive,
                ..
            } => {
                self.emit(DaemonEvent::SystemMessage {
                    text: "No active loop".to_string(),
                    is_error: true,
                });
            }
            CoreOutcome::Rejected {
                error: clankers_core::CoreError::LoopFollowUpStillPending,
                ..
            } => {
                self.emit(DaemonEvent::SystemMessage {
                    text: "Loop control rejected: loop follow-up still pending".to_string(),
                    is_error: true,
                });
            }
            CoreOutcome::Rejected { .. } => unreachable!("stop-loop should reject only on inactive/pending loop state"),
        }
    }

    fn parse_core_thinking_level_input(level: &str) -> CoreThinkingLevelInput {
        if let Some(parsed) = clanker_tui_types::ThinkingLevel::from_str_or_budget(level) {
            CoreThinkingLevelInput::Level(Self::core_thinking_level(parsed))
        } else {
            CoreThinkingLevelInput::Invalid(level.to_string())
        }
    }

    fn core_thinking_level(level: clanker_tui_types::ThinkingLevel) -> CoreThinkingLevel {
        match level {
            clanker_tui_types::ThinkingLevel::Off => CoreThinkingLevel::Off,
            clanker_tui_types::ThinkingLevel::Low => CoreThinkingLevel::Low,
            clanker_tui_types::ThinkingLevel::Medium => CoreThinkingLevel::Medium,
            clanker_tui_types::ThinkingLevel::High => CoreThinkingLevel::High,
            clanker_tui_types::ThinkingLevel::Max => CoreThinkingLevel::Max,
        }
    }

    pub(crate) fn provider_thinking_level(level: CoreThinkingLevel) -> clankers_provider::ThinkingLevel {
        match level {
            CoreThinkingLevel::Off => clankers_provider::ThinkingLevel::Off,
            CoreThinkingLevel::Low => clankers_provider::ThinkingLevel::Low,
            CoreThinkingLevel::Medium => clankers_provider::ThinkingLevel::Medium,
            CoreThinkingLevel::High => clankers_provider::ThinkingLevel::High,
            CoreThinkingLevel::Max => clankers_provider::ThinkingLevel::Max,
        }
    }

    fn thinking_label(level: CoreThinkingLevel) -> &'static str {
        Self::provider_thinking_level(level).label()
    }

    /// Submit a prompt through a controller runtime/session adapter.
    ///
    /// This path exercises the same reducer-backed prompt lifecycle as daemon
    /// mode, but receives semantic events from an injected runtime adapter. It
    /// is intentionally independent from sockets, TUI state, providers, and
    /// desktop session storage.
    pub fn submit_prompt_with_runtime_adapter(
        &mut self,
        adapter: &mut dyn ControllerRuntimeAdapter,
        text: String,
        image_count: u32,
    ) -> bool {
        let prompt_input = CoreInput::PromptRequested(PromptRequest {
            text: text.clone(),
            image_count,
            originating_follow_up_effect_id: None,
        });

        let accepted_prompt = match clankers_core::reduce(&self.core_state, &prompt_input) {
            CoreOutcome::Transitioned { next_state, effects } => {
                self.apply_core_state(next_state);
                self.execute_prompt_request_effects(effects, &text, image_count)
            }
            CoreOutcome::Rejected {
                error: clankers_core::CoreError::Busy,
                ..
            } => {
                self.emit(DaemonEvent::SystemMessage {
                    text: "A prompt is already in progress".to_string(),
                    is_error: true,
                });
                return false;
            }
            CoreOutcome::Rejected { .. } => unreachable!("prompt request should only reject while busy"),
        };

        let prompt_effect_id = accepted_prompt.prompt_start().core_effect_id;
        let result = adapter.submit_prompt(RuntimePromptRequest {
            session_id: self.session_id.clone(),
            model: self.model.clone(),
            text,
            image_count,
        });

        for event in result.semantic_events {
            if let Some(daemon_event) = semantic_event_to_daemon_event(&event) {
                self.outgoing.push(daemon_event);
            }
        }

        let (completion_status, prompt_error) = match result.completion {
            RuntimePromptCompletion::Succeeded => (CompletionStatus::Succeeded, None),
            RuntimePromptCompletion::Cancelled => {
                (CompletionStatus::Failed(CoreFailure::Cancelled), Some("cancelled".to_string()))
            }
            RuntimePromptCompletion::Failed { message } => {
                (CompletionStatus::Failed(CoreFailure::Message(message.clone())), Some(message))
            }
        };

        let applied = self.apply_prompt_completion(clankers_core::PromptCompleted {
            effect_id: prompt_effect_id,
            completion_status,
        });
        debug_assert!(applied, "prompt completion should match the pending prompt");
        self.emit(DaemonEvent::PromptDone { error: prompt_error });
        true
    }

    /// Apply a controller control request through an injected runtime/session adapter.
    pub fn apply_control_with_runtime_adapter(
        &mut self,
        adapter: &mut dyn ControllerRuntimeAdapter,
        request: RuntimeControlRequest,
    ) -> bool {
        match request {
            RuntimeControlRequest::Abort => {
                adapter.apply_control(RuntimeControlRequest::Abort);
                self.busy = false;
                self.core_state.busy = false;
                self.core_state.pending_prompt = None;
                self.emit(DaemonEvent::SystemMessage {
                    text: "Operation cancelled".to_string(),
                    is_error: false,
                });
                true
            }
            RuntimeControlRequest::ResetCancel => {
                adapter.apply_control(RuntimeControlRequest::ResetCancel);
                true
            }
            RuntimeControlRequest::SetThinkingLevel { level } => self.apply_adapter_thinking_level(adapter, level),
            RuntimeControlRequest::SetDisabledTools { tools } => self.apply_adapter_disabled_tools(adapter, tools),
        }
    }

    fn apply_adapter_thinking_level(
        &mut self,
        adapter: &mut dyn ControllerRuntimeAdapter,
        level: CoreThinkingLevel,
    ) -> bool {
        let input = CoreInput::SetThinkingLevel {
            requested: CoreThinkingLevelInput::Level(level),
        };

        match clankers_core::reduce(&self.core_state, &input) {
            CoreOutcome::Transitioned { next_state, effects } => {
                self.apply_core_state(next_state);
                let applied_levels =
                    crate::effect_interpretation::applied_thinking_levels(&effects).collect::<Vec<_>>();
                let thinking_change = crate::effect_interpretation::interpret_thinking_change(&effects)
                    .expect("thinking level change must emit a logical event");
                for level in applied_levels {
                    adapter.apply_control(RuntimeControlRequest::SetThinkingLevel { level });
                }
                self.emit(DaemonEvent::SystemMessage {
                    text: format!(
                        "Thinking: {} → {}",
                        Self::thinking_label(thinking_change.previous),
                        Self::thinking_label(thinking_change.current)
                    ),
                    is_error: false,
                });
                true
            }
            CoreOutcome::Rejected { .. } => false,
        }
    }

    fn apply_adapter_disabled_tools(&mut self, adapter: &mut dyn ControllerRuntimeAdapter, tools: Vec<String>) -> bool {
        let input = CoreInput::SetDisabledTools(DisabledToolsUpdate {
            requested_disabled_tools: tools.clone(),
        });

        match clankers_core::reduce(&self.core_state, &input) {
            CoreOutcome::Transitioned { next_state, effects } => {
                self.apply_core_state(next_state);
                if let Some(application) = crate::effect_interpretation::interpret_tool_filter_application(&effects) {
                    adapter.apply_control(RuntimeControlRequest::SetDisabledTools {
                        tools: application.disabled_tools.clone(),
                    });
                    self.apply_tool_filter_feedback(ToolFilterApplied {
                        effect_id: application.effect_id,
                        applied_disabled_tool_set: application.disabled_tools,
                    });
                    self.emit(DaemonEvent::SystemMessage {
                        text: format!("Disabled tools updated: {}", tools.join(", ")),
                        is_error: false,
                    });
                    return true;
                }
                false
            }
            CoreOutcome::Rejected { .. } => false,
        }
    }

    /// Handle a prompt command (daemon mode only).
    #[cfg_attr(
        dylint_lib = "tigerstyle",
        allow(no_unwrap, reason = "agent is always Some when handle_prompt is called")
    )]
    async fn handle_prompt(&mut self, text: String, images: Vec<ImageData>) {
        self.handle_prompt_inner(text, images, None).await;
    }

    async fn handle_prompt_inner(
        &mut self,
        text: String,
        images: Vec<ImageData>,
        mut on_events: Option<&mut (dyn FnMut(Vec<DaemonEvent>) + Send)>,
    ) {
        if self.agent.is_none() {
            warn!("handle_prompt called in embedded mode");
            return;
        }

        let image_count = u32::try_from(images.len()).unwrap_or(u32::MAX);
        let prompt_input = CoreInput::PromptRequested(PromptRequest {
            text: text.clone(),
            image_count,
            originating_follow_up_effect_id: None,
        });

        let accepted_prompt = match clankers_core::reduce(&self.core_state, &prompt_input) {
            CoreOutcome::Transitioned { next_state, effects } => {
                self.apply_core_state(next_state);
                self.execute_prompt_request_effects(effects, &text, image_count)
            }
            CoreOutcome::Rejected {
                error: clankers_core::CoreError::Busy,
                ..
            } => {
                self.emit(DaemonEvent::SystemMessage {
                    text: "A prompt is already in progress".to_string(),
                    is_error: true,
                });
                return;
            }
            CoreOutcome::Rejected { .. } => unreachable!("prompt request should only reject while busy"),
        };

        let prompt_start = accepted_prompt.prompt_start();
        let prompt_effect_id = prompt_start.core_effect_id;
        let prompt_kind = prompt_start.kind.clone();
        debug_assert!(
            matches!(prompt_kind, crate::core_engine_composition::AcceptedPromptKind::UserPrompt),
            "daemon prompt command must retain user-prompt lifecycle kind"
        );

        self.outgoing.push(DaemonEvent::AgentStart);
        self.flush_outgoing_for_streaming(&mut on_events);

        let image_content = if images.is_empty() {
            None
        } else {
            Some(
                images
                    .into_iter()
                    .map(|img| ProviderContent::Image {
                        source: clankers_provider::message::ImageSource::Base64 {
                            media_type: img.media_type,
                            data: img.data,
                        },
                    })
                    .collect::<Vec<_>>(),
            )
        };

        // Take the agent and event receiver out to avoid borrow conflicts while
        // the prompt future is alive and we keep draining broadcast events.
        let mut agent = self.agent.take().unwrap();
        let mut event_rx = self.event_rx.take();
        let result = {
            let prompt_future = async {
                if let Some(image_content) = image_content {
                    agent.prompt_with_images(&text, image_content).await
                } else {
                    agent.prompt(&text).await
                }
            };
            tokio::pin!(prompt_future);

            if on_events.is_some() {
                if let Some(rx) = event_rx.as_mut() {
                    loop {
                        tokio::select! {
                            result = &mut prompt_future => break result,
                            event = rx.recv() => {
                                match event {
                                    Ok(event) => {
                                        self.process_agent_event(&event);
                                        self.flush_outgoing_for_streaming(&mut on_events);
                                    }
                                    Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                                        warn!("agent event stream lagged while prompt was running, skipped {n} events");
                                    }
                                    Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                                        break (&mut prompt_future).await;
                                    }
                                }
                            }
                        }
                    }
                } else {
                    prompt_future.await
                }
            } else {
                prompt_future.await
            }
        };
        self.event_rx = event_rx;
        self.agent = Some(agent);
        // Drain model stream events before terminal PromptDone so daemon clients
        // never observe a stale completion before the accepted turn's output.
        self.drain_agent_events_to_outgoing();
        self.flush_outgoing_for_streaming(&mut on_events);

        let (completion_status, prompt_error) = match result {
            Ok(()) => (CompletionStatus::Succeeded, None),
            Err(AgentError::Cancelled) => {
                (CompletionStatus::Failed(CoreFailure::Cancelled), Some("cancelled".to_string()))
            }
            Err(error) => {
                let message = error.to_string();
                (CompletionStatus::Failed(CoreFailure::Message(message.clone())), Some(message))
            }
        };

        let applied = self.apply_prompt_completion(clankers_core::PromptCompleted {
            effect_id: prompt_effect_id,
            completion_status: completion_status.clone(),
        });
        debug_assert!(applied, "prompt completion should match the pending prompt");

        self.emit(DaemonEvent::PromptDone { error: prompt_error });
        self.flush_outgoing_for_streaming(&mut on_events);
    }

    fn flush_outgoing_for_streaming(&mut self, on_events: &mut Option<&mut (dyn FnMut(Vec<DaemonEvent>) + Send)>) {
        let Some(callback) = on_events.as_deref_mut() else {
            return;
        };
        let events = std::mem::take(&mut self.outgoing);
        if !events.is_empty() {
            callback(events);
        }
    }

    pub(crate) fn apply_prompt_completion(&mut self, completed: clankers_core::PromptCompleted) -> bool {
        let completion_input = CoreInput::PromptCompleted(completed);
        match clankers_core::reduce(&self.core_state, &completion_input) {
            CoreOutcome::Transitioned { next_state, effects } => {
                self.apply_core_state(next_state);
                self.execute_prompt_completion_effects(effects);
                true
            }
            CoreOutcome::Rejected { .. } => {
                self.emit(DaemonEvent::SystemMessage {
                    text: "Prompt completion rejected".to_string(),
                    is_error: true,
                });
                false
            }
        }
    }

    /// Replay conversation history to a newly-attached client (daemon mode).
    ///
    /// Also emits current session state (tool list, disabled tools, etc.)
    /// so the client can sync its UI.
    fn replay_history(&mut self) {
        // Emit tool list so client knows what's available
        let tools = self.current_tool_infos();
        if !tools.is_empty() {
            self.outgoing.push(DaemonEvent::ToolList { tools });
        }

        // Emit disabled tools
        if !self.disabled_tools.is_empty() {
            self.outgoing.push(DaemonEvent::DisabledToolsChanged {
                tools: self.disabled_tools.clone(),
            });
        }

        // Emit auto-test state
        if self.auto_test_command.is_some() {
            self.outgoing.push(DaemonEvent::AutoTestChanged {
                enabled: self.auto_test_enabled,
                command: self.auto_test_command.clone(),
            });
        }

        // Replay conversation messages
        if let Some(ref agent) = self.agent {
            for msg in agent.messages() {
                let block = serde_json::to_value(msg).unwrap_or_default();
                self.outgoing.push(DaemonEvent::HistoryBlock { block });
            }
        }
        self.emit(DaemonEvent::HistoryEnd);
    }

    /// Handle slash commands forwarded from the client.
    ///
    /// Routes well-known commands directly instead of recursing through
    /// `handle_command` (which would require boxing the future).
    fn handle_slash_command_sync(&mut self, command: &str, args: &str) {
        match command {
            "model" => {
                if args.is_empty() {
                    let model = self.model.clone();
                    self.emit(DaemonEvent::SystemMessage {
                        text: format!("Current model: {model}"),
                        is_error: false,
                    });
                } else {
                    self.set_model_from_command(args.to_string(), "slash command");
                }
            }
            "clear" => {
                if !self.ensure_session_manage_authorized("clear_history") {
                    return;
                }
                if let Some(ref mut agent) = self.agent {
                    agent.clear_messages();
                }
                self.emit(DaemonEvent::SystemMessage {
                    text: "History cleared".to_string(),
                    is_error: false,
                });
            }
            "compact" => {
                if !self.ensure_session_manage_authorized("compact_history") {
                    return;
                }
                if let Some(ref mut agent) = self.agent {
                    let result = agent.compact_messages();
                    self.emit(DaemonEvent::SessionCompaction {
                        compacted_count: result.compacted_count,
                        tokens_saved: result.tokens_saved,
                    });
                }
            }
            "thinking" => {
                if args.is_empty() {
                    if self.ensure_session_manage_authorized("cycle_thinking_level") {
                        self.handle_cycle_thinking_level();
                    }
                } else if self.ensure_session_manage_authorized("set_thinking_level") {
                    self.handle_set_thinking_level(args.to_string());
                }
            }
            "stop" => {
                if self.ensure_session_manage_authorized("stop_loop") {
                    self.handle_stop_loop();
                }
            }
            "autotest" => {
                if !self.ensure_session_manage_authorized("set_auto_test") {
                    return;
                }
                if args.is_empty() {
                    self.auto_test_enabled = !self.auto_test_enabled;
                } else {
                    self.auto_test_enabled = true;
                    self.auto_test_command = Some(args.to_string());
                }
                self.emit(DaemonEvent::AutoTestChanged {
                    enabled: self.auto_test_enabled,
                    command: self.auto_test_command.clone(),
                });
            }
            "tools" => {
                self.emit(DaemonEvent::ToolList {
                    tools: self.current_tool_infos(),
                });
            }
            "prompt" => {
                let prompt = self.agent.as_ref().map(|a| a.system_prompt().to_string()).unwrap_or_default();
                self.emit(DaemonEvent::SystemPromptResponse { prompt });
            }
            _ => {
                self.emit(DaemonEvent::SystemMessage {
                    text: format!("/{command}: not implemented in daemon mode"),
                    is_error: true,
                });
            }
        }
    }

    /// Convert serialized messages to agent messages for seeding.
    fn convert_seed_messages(&self, messages: &[SerializedMessage]) -> Vec<AgentMessage> {
        messages
            .iter()
            .filter_map(|msg| {
                let id = MessageId::generate();
                let now = chrono::Utc::now();
                match msg.role.as_str() {
                    "user" => Some(AgentMessage::User(UserMessage {
                        id,
                        content: vec![Content::Text {
                            text: msg.content.clone(),
                        }],
                        timestamp: now,
                    })),
                    "assistant" => Some(AgentMessage::Assistant(AssistantMessage {
                        id,
                        content: vec![Content::Text {
                            text: msg.content.clone(),
                        }],
                        model: msg.model.clone().unwrap_or_default(),
                        usage: Default::default(),
                        stop_reason: StopReason::Stop,
                        timestamp: now,
                    })),
                    other => {
                        warn!("skipping unknown role in seed message: {other}");
                        None
                    }
                }
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::sync::Mutex;
    use std::sync::atomic::AtomicBool;
    use std::sync::atomic::Ordering;
    use std::time::Duration;

    use clankers_core::CoreEffect;
    use clankers_core::CoreLogicalEvent;
    use clankers_protocol::SessionCommand;
    use clankers_provider::ThinkingLevel;

    use super::*;
    use crate::PostPromptAction;
    use crate::ShellFollowUpDispatch;
    use crate::ToolRebuilder;
    use crate::config::ControllerConfig;
    use crate::test_helpers::make_test_controller;

    const FIRST_EFFECT_ID: clankers_core::CoreEffectId = clankers_core::CoreEffectId(1);
    const LOOP_ITERATION_LIMIT: u32 = 2;

    #[derive(Debug, Clone, PartialEq, Eq)]
    struct RecordedPromptRequest {
        model: String,
        prompt_text: String,
        system_prompt: Option<String>,
        session_id: Option<String>,
    }

    struct RecordingPromptProvider {
        requests: Arc<Mutex<Vec<RecordedPromptRequest>>>,
    }

    fn record_prompt_request(
        requests: &Mutex<Vec<RecordedPromptRequest>>,
        request: clankers_provider::CompletionRequest,
    ) -> RecordedPromptRequest {
        let prompt_text =
            extract_last_user_prompt_text(&request.messages).expect("prompt request should carry a user message");
        let recorded = RecordedPromptRequest {
            model: request.model,
            prompt_text,
            system_prompt: request.system_prompt,
            session_id: request.extra_params.get("_session_id").and_then(|value| value.as_str()).map(str::to_string),
        };
        requests.lock().unwrap_or_else(|poisoned| poisoned.into_inner()).push(recorded.clone());
        recorded
    }

    #[async_trait::async_trait]
    impl clankers_provider::Provider for RecordingPromptProvider {
        async fn complete(
            &self,
            request: clankers_provider::CompletionRequest,
            _tx: tokio::sync::mpsc::Sender<clankers_provider::streaming::StreamEvent>,
        ) -> clankers_provider::error::Result<()> {
            record_prompt_request(&self.requests, request);
            Ok(())
        }

        fn models(&self) -> &[clankers_provider::Model] {
            &[]
        }

        fn name(&self) -> &str {
            "recording"
        }
    }

    struct StreamingPromptProvider {
        requests: Arc<Mutex<Vec<RecordedPromptRequest>>>,
    }

    struct DelayedStreamingPromptProvider {
        requests: Arc<Mutex<Vec<RecordedPromptRequest>>>,
        streamed: Arc<tokio::sync::Notify>,
        release: Arc<tokio::sync::Notify>,
        returned: Arc<AtomicBool>,
    }

    #[async_trait::async_trait]
    impl clankers_provider::Provider for StreamingPromptProvider {
        async fn complete(
            &self,
            request: clankers_provider::CompletionRequest,
            tx: tokio::sync::mpsc::Sender<clankers_provider::streaming::StreamEvent>,
        ) -> clankers_provider::error::Result<()> {
            let recorded = record_prompt_request(&self.requests, request);
            let streamed_text = format!("stream:{}", recorded.prompt_text);
            tx.send(clankers_provider::streaming::StreamEvent::MessageStart {
                message: clankers_provider::streaming::MessageMetadata {
                    id: format!("msg-{}", recorded.prompt_text),
                    model: recorded.model,
                    role: "assistant".to_string(),
                },
            })
            .await
            .ok();
            tx.send(clankers_provider::streaming::StreamEvent::ContentBlockStart {
                index: 0,
                content_block: clanker_message::Content::Text { text: String::new() },
            })
            .await
            .ok();
            tx.send(clankers_provider::streaming::StreamEvent::ContentBlockDelta {
                index: 0,
                delta: clankers_provider::streaming::ContentDelta::TextDelta { text: streamed_text },
            })
            .await
            .ok();
            tx.send(clankers_provider::streaming::StreamEvent::ContentBlockStop { index: 0 }).await.ok();
            tx.send(clankers_provider::streaming::StreamEvent::MessageDelta {
                stop_reason: Some("end_turn".to_string()),
                usage: clanker_message::Usage::default(),
            })
            .await
            .ok();
            tx.send(clankers_provider::streaming::StreamEvent::MessageStop).await.ok();
            Ok(())
        }

        fn models(&self) -> &[clankers_provider::Model] {
            &[]
        }

        fn name(&self) -> &str {
            "streaming-recording"
        }
    }

    #[async_trait::async_trait]
    impl clankers_provider::Provider for DelayedStreamingPromptProvider {
        async fn complete(
            &self,
            request: clankers_provider::CompletionRequest,
            tx: tokio::sync::mpsc::Sender<clankers_provider::streaming::StreamEvent>,
        ) -> clankers_provider::error::Result<()> {
            let recorded = record_prompt_request(&self.requests, request);
            tx.send(clankers_provider::streaming::StreamEvent::MessageStart {
                message: clankers_provider::streaming::MessageMetadata {
                    id: format!("msg-{}", recorded.prompt_text),
                    model: recorded.model,
                    role: "assistant".to_string(),
                },
            })
            .await
            .ok();
            tx.send(clankers_provider::streaming::StreamEvent::ContentBlockStart {
                index: 0,
                content_block: clanker_message::Content::Text { text: String::new() },
            })
            .await
            .ok();
            tx.send(clankers_provider::streaming::StreamEvent::ContentBlockDelta {
                index: 0,
                delta: clankers_provider::streaming::ContentDelta::TextDelta {
                    text: "stream:delayed".to_string(),
                },
            })
            .await
            .ok();
            self.streamed.notify_waiters();
            self.release.notified().await;
            self.returned.store(true, Ordering::SeqCst);
            tx.send(clankers_provider::streaming::StreamEvent::ContentBlockStop { index: 0 }).await.ok();
            tx.send(clankers_provider::streaming::StreamEvent::MessageDelta {
                stop_reason: Some("end_turn".to_string()),
                usage: clanker_message::Usage::default(),
            })
            .await
            .ok();
            tx.send(clankers_provider::streaming::StreamEvent::MessageStop).await.ok();
            Ok(())
        }

        fn models(&self) -> &[clankers_provider::Model] {
            &[]
        }

        fn name(&self) -> &str {
            "delayed-streaming-recording"
        }
    }

    #[derive(Clone, Default)]
    struct RecordingRebuilder {
        calls: Arc<Mutex<Vec<Vec<String>>>>,
    }

    impl RecordingRebuilder {
        fn take_calls(&self) -> Vec<Vec<String>> {
            self.calls.lock().unwrap_or_else(|poisoned| poisoned.into_inner()).clone()
        }
    }

    impl ToolRebuilder for RecordingRebuilder {
        fn rebuild_filtered(&self, disabled: &[String]) -> Vec<Arc<dyn clankers_agent::Tool>> {
            self.calls.lock().unwrap_or_else(|poisoned| poisoned.into_inner()).push(disabled.to_vec());
            Vec::new()
        }
    }

    struct DenySessionOperationGate;

    impl clankers_agent::CapabilityGate for DenySessionOperationGate {
        fn check_prompt(&self, _session_id: &str, _text: &str) -> std::result::Result<(), String> {
            Err("prompt denied by test gate".to_string())
        }

        fn check_session_manage(&self, _session_id: &str, action: &str) -> std::result::Result<(), String> {
            Err(format!("session manage denied by test gate: {action}"))
        }

        fn check_model_switch(&self, model: &str) -> std::result::Result<(), String> {
            Err(format!("model switch denied by test gate: {model}"))
        }

        fn check_tool_call(&self, _tool_name: &str, _input: &serde_json::Value) -> std::result::Result<(), String> {
            Ok(())
        }
    }

    fn install_capability_gate(ctrl: &mut SessionController, gate: Arc<dyn clankers_agent::CapabilityGate>) {
        let agent = ctrl.agent.take().expect("test controller owns an agent").with_capability_gate(gate);
        ctrl.agent = Some(agent);
    }

    fn extract_last_user_prompt_text(messages: &[clankers_provider::message::AgentMessage]) -> Option<String> {
        messages.iter().rev().find_map(|message| match message {
            clankers_provider::message::AgentMessage::User(user_message) => {
                user_message.content.iter().find_map(|content| match content {
                    clankers_provider::message::Content::Text { text } => Some(text.clone()),
                    _ => None,
                })
            }
            _ => None,
        })
    }

    struct DenyPreTurnHook;

    #[async_trait::async_trait]
    impl clankers_hooks::HookHandler for DenyPreTurnHook {
        fn name(&self) -> &str {
            "deny-pre-turn"
        }

        fn priority(&self) -> u32 {
            clankers_hooks::dispatcher::PRIORITY_PLUGIN_HOOKS
        }

        fn subscribes_to(&self, point: clankers_hooks::HookPoint) -> bool {
            matches!(point, clankers_hooks::HookPoint::PreTurn)
        }

        async fn handle(
            &self,
            _point: clankers_hooks::HookPoint,
            _payload: &clankers_hooks::HookPayload,
        ) -> clankers_hooks::HookVerdict {
            clankers_hooks::HookVerdict::Deny {
                reason: "controller hook blocked turn".to_string(),
            }
        }
    }

    fn make_test_controller_with_provider(provider: Arc<dyn clankers_provider::Provider>) -> SessionController {
        make_test_controller_with_provider_and_hooks(provider, None)
    }

    fn make_test_controller_with_provider_and_hooks(
        provider: Arc<dyn clankers_provider::Provider>,
        hook_pipeline: Option<Arc<clankers_hooks::HookPipeline>>,
    ) -> SessionController {
        let agent = clankers_agent::Agent::new(
            provider,
            vec![],
            clankers_config::settings::Settings::default(),
            "test-model".to_string(),
            "You are a test assistant.".to_string(),
        );
        SessionController::new(agent, ControllerConfig {
            session_id: "test-session".to_string(),
            model: "test-model".to_string(),
            hook_pipeline,
            ..Default::default()
        })
    }

    fn seed_pending_tool_filter(ctrl: &mut SessionController, disabled_tools: Vec<String>) {
        ctrl.core_state.disabled_tools = disabled_tools.clone();
        ctrl.core_state.pending_tool_filter = Some(clankers_core::PendingToolFilterState {
            effect_id: FIRST_EFFECT_ID,
            requested_disabled_tools: disabled_tools,
        });
        ctrl.core_state.next_effect_id = FIRST_EFFECT_ID;
    }

    fn assert_tool_filter_feedback_rejected(
        ctrl: &mut SessionController,
        feedback: ToolFilterApplied,
        expected_error: clankers_core::CoreError,
    ) {
        let previous_state = ctrl.core_state.clone();
        let expected_outcome = clankers_core::reduce(&previous_state, &CoreInput::ToolFilterApplied(feedback.clone()));
        assert!(matches!(
            expected_outcome,
            CoreOutcome::Rejected { unchanged_state, error }
                if unchanged_state == previous_state && error == expected_error
        ));

        let applied = ctrl.apply_tool_filter_feedback(feedback);

        assert!(!applied);
        assert_eq!(ctrl.core_state, previous_state);
        assert!(matches!(ctrl.drain_events().as_slice(), [DaemonEvent::SystemMessage { is_error: true, .. }]));
    }

    #[tokio::test]
    async fn test_handle_abort() {
        let mut ctrl = make_test_controller();
        ctrl.handle_command(SessionCommand::Abort).await;

        let events = ctrl.drain_events();
        assert!(
            events.iter().any(
                |e| matches!(e, DaemonEvent::SystemMessage { text, is_error: false } if text.contains("cancelled"))
            )
        );
    }

    #[tokio::test]
    async fn test_handle_clear_history() {
        let mut ctrl = make_test_controller();
        ctrl.handle_command(SessionCommand::ClearHistory).await;

        let events = ctrl.drain_events();
        assert!(
            events
                .iter()
                .any(|e| matches!(e, DaemonEvent::SystemMessage { text, .. } if text.contains("cleared")))
        );
    }

    #[tokio::test]
    async fn test_handle_set_model() {
        let mut ctrl = make_test_controller();
        ctrl.handle_command(SessionCommand::SetModel {
            model: "opus".to_string(),
        })
        .await;

        let events = ctrl.drain_events();
        assert!(events.iter().any(|e| matches!(
            e,
            DaemonEvent::ModelChanged {
                from,
                to,
                ..
            } if from == "test-model" && to == "opus"
        )));
        assert_eq!(ctrl.model(), "opus");
    }

    #[tokio::test]
    async fn set_model_is_denied_by_capability_gate() {
        let mut ctrl = make_test_controller();
        install_capability_gate(&mut ctrl, Arc::new(DenySessionOperationGate));

        ctrl.handle_command(SessionCommand::SetModel {
            model: "opus".to_string(),
        })
        .await;

        let events = ctrl.drain_events();
        assert!(matches!(events.as_slice(), [DaemonEvent::SystemMessage { text, is_error: true }]
            if text.contains("model switch denied")));
        assert_eq!(ctrl.model(), "test-model");
    }

    #[tokio::test]
    async fn session_manage_command_is_denied_by_capability_gate_before_mutation() {
        let mut ctrl = make_test_controller();
        install_capability_gate(&mut ctrl, Arc::new(DenySessionOperationGate));

        ctrl.handle_command(SessionCommand::SetSystemPrompt {
            prompt: "new prompt".to_string(),
        })
        .await;

        let events = ctrl.drain_events();
        assert!(matches!(events.as_slice(), [DaemonEvent::SystemMessage { text, is_error: true }]
            if text.contains("session manage denied")));
        assert_eq!(ctrl.agent.as_ref().expect("agent").system_prompt(), "You are a test assistant.");
    }

    #[tokio::test]
    async fn prompt_is_denied_by_capability_gate_before_history_mutation() {
        let mut ctrl = make_test_controller();
        install_capability_gate(&mut ctrl, Arc::new(DenySessionOperationGate));

        ctrl.handle_command(SessionCommand::Prompt {
            text: "hello".to_string(),
            images: vec![],
        })
        .await;

        let events = ctrl.drain_events();
        assert!(events.iter().any(|event| matches!(
            event,
            DaemonEvent::PromptDone { error: Some(message) } if message.contains("prompt denied")
        )));
        assert!(ctrl.agent.as_ref().expect("agent").messages().is_empty());
    }

    #[tokio::test]
    async fn controller_owned_prompt_pre_turn_denial_prevents_provider_request() {
        let requests = Arc::new(Mutex::new(Vec::new()));
        let mut pipeline = clankers_hooks::HookPipeline::new();
        pipeline.register(Arc::new(DenyPreTurnHook));
        let mut ctrl = make_test_controller_with_provider_and_hooks(
            Arc::new(RecordingPromptProvider {
                requests: requests.clone(),
            }),
            Some(Arc::new(pipeline)),
        );

        ctrl.handle_command(SessionCommand::Prompt {
            text: "hello".to_string(),
            images: vec![],
        })
        .await;

        let events = ctrl.drain_events();
        assert!(events.iter().any(|event| matches!(
            event,
            DaemonEvent::PromptDone { error: Some(message) } if message.contains("controller hook blocked turn")
        )));
        assert!(requests.lock().unwrap_or_else(|poisoned| poisoned.into_inner()).is_empty());
        assert_eq!(ctrl.agent.as_ref().expect("agent").messages().len(), 1);
    }

    #[tokio::test]
    async fn test_handle_get_system_prompt() {
        let mut ctrl = make_test_controller();
        ctrl.handle_command(SessionCommand::GetSystemPrompt).await;

        let events = ctrl.drain_events();
        assert!(events.iter().any(|e| matches!(
            e,
            DaemonEvent::SystemPromptResponse { prompt } if prompt == "You are a test assistant."
        )));
    }

    #[tokio::test]
    async fn test_handle_get_capabilities_none() {
        let mut ctrl = make_test_controller();
        ctrl.handle_command(SessionCommand::GetCapabilities).await;

        let events = ctrl.drain_events();
        assert!(events.iter().any(|e| matches!(e, DaemonEvent::Capabilities { capabilities: None })));
    }

    #[tokio::test]
    async fn test_handle_get_capabilities_some() {
        let mut ctrl = make_test_controller();
        ctrl.capabilities = Some(vec!["read".to_string(), "grep".to_string()]);
        ctrl.handle_command(SessionCommand::GetCapabilities).await;

        let events = ctrl.drain_events();
        assert!(events.iter().any(|e| matches!(
            e,
            DaemonEvent::Capabilities { capabilities: Some(caps) } if caps.len() == 2
        )));
    }

    #[test]
    fn runtime_adapter_fixture_covers_prompt_control_identity_and_semantic_projection() {
        let mut ctrl = SessionController::new_embedded(ControllerConfig {
            session_id: "runtime-fixture-session".to_string(),
            model: "runtime-fixture-model".to_string(),
            ..Default::default()
        });
        let mut adapter = crate::runtime_adapter::FakeRuntimeAdapter::new(vec![
            crate::runtime_adapter::RuntimePromptResult::succeeded(vec![
                clanker_message::SemanticEvent::AssistantDelta {
                    text: "runtime answer".to_string(),
                    metadata: clanker_message::SemanticEventMetadata::empty().with("source", "fake-runtime"),
                },
                clanker_message::SemanticEvent::UsageUpdated {
                    input_tokens: 11,
                    output_tokens: 7,
                    cache_read_tokens: 3,
                    metadata: clanker_message::SemanticEventMetadata::empty().with("source", "fake-runtime"),
                },
                clanker_message::SemanticEvent::Completed {
                    stop_reason: clanker_message::SemanticStopReason::Complete,
                    metadata: clanker_message::SemanticEventMetadata::empty().with("source", "fake-runtime"),
                },
            ]),
        ]);

        assert!(ctrl.submit_prompt_with_runtime_adapter(&mut adapter, "hello runtime".to_string(), 0));
        assert_eq!(adapter.prompts.len(), 1);
        assert_eq!(adapter.prompts[0].session_id, "runtime-fixture-session");
        assert_eq!(adapter.prompts[0].model, "runtime-fixture-model");
        assert_eq!(adapter.prompts[0].text, "hello runtime");
        assert!(!ctrl.busy);
        assert!(!ctrl.core_state.busy);
        assert!(ctrl.core_state.pending_prompt.is_none());
        let prompt_events = ctrl.drain_events();
        assert!(matches!(prompt_events.first(), Some(DaemonEvent::TextDelta { text }) if text == "runtime answer"));
        assert!(prompt_events.iter().any(|event| matches!(event, DaemonEvent::UsageUpdate {
            input_tokens: 11,
            output_tokens: 7,
            cache_read: 3,
            ..
        })));
        assert!(matches!(prompt_events.last(), Some(DaemonEvent::PromptDone { error: None })));

        assert!(ctrl.apply_control_with_runtime_adapter(&mut adapter, RuntimeControlRequest::SetThinkingLevel {
            level: CoreThinkingLevel::High,
        },));
        assert!(ctrl.apply_control_with_runtime_adapter(&mut adapter, RuntimeControlRequest::SetDisabledTools {
            tools: vec!["bash".to_string()],
        },));
        assert!(ctrl.start_embedded_prompt("cancel me", 0));
        assert!(ctrl.apply_control_with_runtime_adapter(&mut adapter, RuntimeControlRequest::Abort));
        assert_eq!(adapter.controls, vec![
            RuntimeControlRequest::SetThinkingLevel {
                level: CoreThinkingLevel::High,
            },
            RuntimeControlRequest::SetDisabledTools {
                tools: vec!["bash".to_string()],
            },
            RuntimeControlRequest::Abort,
        ]);
        assert!(!ctrl.busy);
        assert!(!ctrl.core_state.busy);
        assert!(ctrl.core_state.pending_prompt.is_none());
        let control_events = ctrl.drain_events();
        assert!(control_events.iter().any(|event| matches!(
            event,
            DaemonEvent::SystemMessage { text, is_error: false } if text.contains("Thinking")
        )));
        assert!(control_events.iter().any(|event| matches!(
            event,
            DaemonEvent::DisabledToolsChanged { tools } if tools == &vec!["bash".to_string()]
        )));
        assert!(control_events.iter().any(|event| matches!(
            event,
            DaemonEvent::SystemMessage { text, is_error: false } if text == "Operation cancelled"
        )));
    }

    #[tokio::test]
    async fn test_reject_concurrent_prompt_preserves_busy_rejection_kind_and_no_shell_work() {
        let mut ctrl = make_test_controller();
        ctrl.busy = true;
        ctrl.core_state.busy = true;
        let previous_state = ctrl.core_state.clone();
        let prompt_input = CoreInput::PromptRequested(PromptRequest {
            text: "hello".to_string(),
            image_count: 0,
            originating_follow_up_effect_id: None,
        });

        let expected_outcome = clankers_core::reduce(&previous_state, &prompt_input);
        assert!(matches!(
            expected_outcome,
            CoreOutcome::Rejected {
                unchanged_state,
                error: clankers_core::CoreError::Busy,
            } if unchanged_state == previous_state
        ));

        ctrl.handle_command(SessionCommand::Prompt {
            text: "hello".to_string(),
            images: vec![],
        })
        .await;

        assert_eq!(ctrl.core_state, previous_state);
        assert!(matches!(
            ctrl.take_outgoing().as_slice(),
            [DaemonEvent::SystemMessage { text, is_error: true }] if text == "A prompt is already in progress"
        ));
    }

    #[tokio::test]
    async fn test_handle_command_prompt_uses_reducer_start_effect_and_preserves_shell_events() {
        let recorded_requests = Arc::new(Mutex::new(Vec::new()));
        let provider = Arc::new(RecordingPromptProvider {
            requests: Arc::clone(&recorded_requests),
        });
        let mut ctrl = make_test_controller_with_provider(provider);
        let prompt_text = "hello".to_string();
        let prompt_input = CoreInput::PromptRequested(PromptRequest {
            text: prompt_text.clone(),
            image_count: 0,
            originating_follow_up_effect_id: None,
        });

        let expected_outcome = clankers_core::reduce(&ctrl.core_state, &prompt_input);
        assert!(matches!(
            expected_outcome,
            CoreOutcome::Transitioned { next_state, effects }
                if next_state.busy
                    && next_state.pending_prompt
                        == Some(clankers_core::PendingPromptState {
                            effect_id: FIRST_EFFECT_ID,
                            prompt_text: prompt_text.clone(),
                            image_count: 0,
                            originating_follow_up_effect_id: None,
                        })
                    && effects
                        == vec![
                            CoreEffect::EmitLogicalEvent(CoreLogicalEvent::BusyChanged { busy: true }),
                            CoreEffect::StartPrompt {
                                effect_id: FIRST_EFFECT_ID,
                                prompt_text: prompt_text.clone(),
                                image_count: 0,
                            },
                        ]
        ));

        ctrl.handle_command(SessionCommand::Prompt {
            text: prompt_text.clone(),
            images: vec![],
        })
        .await;

        assert!(!ctrl.busy);
        assert!(!ctrl.core_state.busy);
        assert!(ctrl.core_state.pending_prompt.is_none());
        assert_eq!(recorded_requests.lock().unwrap_or_else(|poisoned| poisoned.into_inner()).as_slice(), [
            RecordedPromptRequest {
                model: "test-model".to_string(),
                prompt_text,
                system_prompt: Some("You are a test assistant.".to_string()),
                session_id: Some("test-session".to_string()),
            }
        ]);
        let events = ctrl.take_outgoing();
        assert!(matches!(events.first(), Some(DaemonEvent::AgentStart)));
        assert!(matches!(events.last(), Some(DaemonEvent::PromptDone { error: None })));
        assert!(events.iter().any(|event| matches!(event, DaemonEvent::AgentEnd)));
        assert!(!events.iter().any(|event| matches!(event, DaemonEvent::TextDelta { .. })));
    }

    #[tokio::test]
    async fn repeated_daemon_prompts_stream_and_complete_in_session_order() {
        let recorded_requests = Arc::new(Mutex::new(Vec::new()));
        let provider = Arc::new(StreamingPromptProvider {
            requests: Arc::clone(&recorded_requests),
        });
        let mut ctrl = make_test_controller_with_provider(provider);

        ctrl.handle_command(SessionCommand::Prompt {
            text: "first".to_string(),
            images: vec![],
        })
        .await;
        let first_events = ctrl.drain_events();

        ctrl.handle_command(SessionCommand::Prompt {
            text: "second".to_string(),
            images: vec![],
        })
        .await;
        let second_events = ctrl.drain_events();

        assert_prompt_stream_completed_after_delta(&first_events, "stream:first");
        assert_prompt_stream_completed_after_delta(&second_events, "stream:second");
        assert!(!ctrl.busy);
        assert!(!ctrl.core_state.busy);
        assert!(ctrl.core_state.pending_prompt.is_none());
        assert_eq!(recorded_requests.lock().unwrap_or_else(|poisoned| poisoned.into_inner()).as_slice(), [
            RecordedPromptRequest {
                model: "test-model".to_string(),
                prompt_text: "first".to_string(),
                system_prompt: Some("You are a test assistant.".to_string()),
                session_id: Some("test-session".to_string()),
            },
            RecordedPromptRequest {
                model: "test-model".to_string(),
                prompt_text: "second".to_string(),
                system_prompt: Some("You are a test assistant.".to_string()),
                session_id: Some("test-session".to_string()),
            },
        ]);
    }

    #[tokio::test]
    async fn streaming_command_callback_receives_delta_before_provider_returns() {
        let recorded_requests = Arc::new(Mutex::new(Vec::new()));
        let streamed = Arc::new(tokio::sync::Notify::new());
        let release = Arc::new(tokio::sync::Notify::new());
        let returned = Arc::new(AtomicBool::new(false));
        let provider = Arc::new(DelayedStreamingPromptProvider {
            requests: Arc::clone(&recorded_requests),
            streamed: Arc::clone(&streamed),
            release: Arc::clone(&release),
            returned: Arc::clone(&returned),
        });
        let mut ctrl = make_test_controller_with_provider(provider);
        let observed_events: Arc<Mutex<Vec<DaemonEvent>>> = Arc::new(Mutex::new(Vec::new()));
        let callback_events = Arc::clone(&observed_events);

        let task = tokio::spawn(async move {
            let mut callback = move |events: Vec<DaemonEvent>| {
                callback_events.lock().unwrap_or_else(|poisoned| poisoned.into_inner()).extend(events);
            };
            ctrl.handle_command_with_streaming_events(
                SessionCommand::Prompt {
                    text: "delayed".to_string(),
                    images: vec![],
                },
                &mut callback,
            )
            .await;
            ctrl
        });

        tokio::time::timeout(Duration::from_secs(1), streamed.notified())
            .await
            .expect("provider should stream a delta before waiting");
        for _ in 0..100 {
            let saw_delta = observed_events
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .iter()
                .any(|event| matches!(event, DaemonEvent::TextDelta { text } if text == "stream:delayed"));
            if saw_delta {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }

        let before_release = observed_events.lock().unwrap_or_else(|poisoned| poisoned.into_inner()).clone();
        assert!(
            before_release
                .iter()
                .any(|event| matches!(event, DaemonEvent::TextDelta { text } if text == "stream:delayed")),
            "stream delta should be delivered before provider completion: {before_release:?}"
        );
        assert!(!returned.load(Ordering::SeqCst), "provider must still be blocked when the delta is delivered");

        release.notify_waiters();
        let mut ctrl = task.await.expect("streaming command task should finish");
        let trailing_events = ctrl.drain_events();
        assert!(trailing_events.is_empty(), "streaming path should not leave undrained events: {trailing_events:?}");
        let all_events = observed_events.lock().unwrap_or_else(|poisoned| poisoned.into_inner()).clone();
        assert_prompt_stream_completed_after_delta(&all_events, "stream:delayed");
        assert_eq!(recorded_requests.lock().unwrap_or_else(|poisoned| poisoned.into_inner()).len(), 1);
    }

    #[tokio::test]
    async fn rejected_follow_up_does_not_block_later_prompt_streaming() {
        let recorded_requests = Arc::new(Mutex::new(Vec::new()));
        let provider = Arc::new(StreamingPromptProvider {
            requests: Arc::clone(&recorded_requests),
        });
        let mut ctrl = make_test_controller_with_provider(provider);
        ctrl.auto_test_enabled = true;
        ctrl.auto_test_command = Some("cargo test".to_string());

        let pending_work_id = match ctrl.check_post_prompt(false) {
            PostPromptAction::RunAutoTest { pending_work_id, .. } => pending_work_id,
            other => panic!("expected RunAutoTest, got {other:?}"),
        };
        ctrl.ack_follow_up_dispatch(pending_work_id, ShellFollowUpDispatch::rejected("channel closed"));
        let rejection_events = ctrl.drain_events();
        assert!(
            rejection_events
                .iter()
                .any(|event| matches!(event, DaemonEvent::SystemMessage { is_error: true, .. }))
        );
        assert!(!ctrl.busy);
        assert!(!ctrl.core_state.busy);
        assert!(ctrl.core_state.pending_prompt.is_none());
        assert!(ctrl.core_state.pending_follow_up_state.is_none());

        ctrl.handle_command(SessionCommand::Prompt {
            text: "after rejection".to_string(),
            images: vec![],
        })
        .await;
        let prompt_events = ctrl.drain_events();

        assert_prompt_stream_completed_after_delta(&prompt_events, "stream:after rejection");
        assert_eq!(
            recorded_requests
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .last()
                .map(|request| request.prompt_text.as_str()),
            Some("after rejection")
        );
    }

    fn assert_prompt_stream_completed_after_delta(events: &[DaemonEvent], expected_delta: &str) {
        let delta_index = events
            .iter()
            .position(|event| matches!(event, DaemonEvent::TextDelta { text } if text == expected_delta))
            .unwrap_or_else(|| panic!("missing text delta {expected_delta:?}: {events:?}"));
        let done_index = events
            .iter()
            .position(|event| matches!(event, DaemonEvent::PromptDone { error: None }))
            .unwrap_or_else(|| panic!("missing prompt completion after {expected_delta:?}: {events:?}"));
        assert!(
            delta_index < done_index,
            "stream delta must be visible before terminal prompt completion: {events:?}"
        );
    }

    #[tokio::test]
    async fn test_replay_history() {
        let mut ctrl = make_test_controller();
        ctrl.handle_command(SessionCommand::ReplayHistory).await;

        let events = ctrl.drain_events();
        // Should end with HistoryEnd
        assert!(events.last().is_some_and(|e| matches!(e, DaemonEvent::HistoryEnd)));
    }

    #[tokio::test]
    async fn test_confirm_bash_unknown_request() {
        let mut ctrl = make_test_controller();
        ctrl.handle_command(SessionCommand::ConfirmBash {
            request_id: "nonexistent".to_string(),
            approved: true,
        })
        .await;
        // Should just log a warning, not crash
        let events = ctrl.drain_events();
        assert!(events.is_empty());
    }

    #[tokio::test]
    async fn test_set_thinking_level_valid() {
        let mut ctrl = make_test_controller();
        ctrl.handle_command(SessionCommand::SetThinkingLevel {
            level: "high".to_string(),
        })
        .await;

        let events = ctrl.drain_events();
        assert!(
            events
                .iter()
                .any(|e| matches!(e, DaemonEvent::SystemMessage { text, is_error: false } if text.contains("high")))
        );
    }

    #[tokio::test]
    async fn test_set_thinking_level_invalid() {
        let mut ctrl = make_test_controller();
        let previous_state = ctrl.core_state.clone();
        ctrl.handle_command(SessionCommand::SetThinkingLevel {
            level: "bogus".to_string(),
        })
        .await;

        assert_eq!(ctrl.core_state, previous_state);
        let events = ctrl.drain_events();
        assert!(events.iter().any(|e| matches!(e, DaemonEvent::SystemMessage { is_error: true, .. })));
    }

    #[tokio::test]
    async fn test_cycle_thinking_level_order() {
        let mut ctrl = make_test_controller();
        let expected_levels = [
            ThinkingLevel::Low,
            ThinkingLevel::Medium,
            ThinkingLevel::High,
            ThinkingLevel::Max,
            ThinkingLevel::Off,
        ];

        for expected_level in expected_levels {
            ctrl.handle_command(SessionCommand::CycleThinkingLevel).await;

            let agent = ctrl.agent.as_ref().expect("controller should keep owning the agent");
            assert_eq!(agent.thinking_level(), expected_level);

            let events = ctrl.drain_events();
            assert!(matches!(
                events.as_slice(),
                [DaemonEvent::SystemMessage { text, is_error: false }] if text == "Thinking level cycled"
            ));
        }
    }

    #[test]
    fn test_slash_thinking_routes_through_reducer_backed_handlers() {
        let mut ctrl = make_test_controller();

        ctrl.handle_slash_command_sync("thinking", "high");
        let events = ctrl.drain_events();
        assert!(matches!(
            events.as_slice(),
            [DaemonEvent::SystemMessage { text, is_error: false }] if text.contains("Thinking: off → high")
        ));
        let agent = ctrl.agent.as_ref().expect("controller should keep owning the agent");
        assert_eq!(agent.thinking_level(), ThinkingLevel::High);

        ctrl.handle_slash_command_sync("thinking", "");
        let events = ctrl.drain_events();
        assert!(matches!(
            events.as_slice(),
            [DaemonEvent::SystemMessage { text, is_error: false }] if text == "Thinking level cycled"
        ));
        let agent = ctrl.agent.as_ref().expect("controller should keep owning the agent");
        assert_eq!(agent.thinking_level(), ThinkingLevel::Max);
    }

    fn workspace_sources(paths: &[&str]) -> Vec<String> {
        let manifest_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let workspace_crates_dir = manifest_dir.parent().expect("controller crate should have a parent crates dir");
        paths
            .iter()
            .filter_map(|path| std::fs::read_to_string(workspace_crates_dir.join(path)).ok())
            .collect()
    }

    fn assert_sources_do_not_contain_symbols(sources: &[String], symbols: &[String]) {
        if sources.is_empty() {
            return;
        }
        for source in sources {
            for symbol in symbols {
                assert!(
                    !source.contains(symbol),
                    "engine/agent turn sources must not own core lifecycle policy symbol {symbol}"
                );
            }
        }
    }

    #[tokio::test]
    async fn thinking_effects_remain_core_owned() {
        let mut ctrl = make_test_controller();
        let thinking_symbols = [
            ["Set", "ThinkingLevel"].concat(),
            ["Cycle", "ThinkingLevel"].concat(),
            ["Apply", "ThinkingLevel"].concat(),
        ];
        let engine_and_agent_turn_sources = workspace_sources(&[
            "clankers-engine/src/lib.rs",
            "clankers-agent/src/turn/mod.rs",
            "clankers-agent/src/turn/execution.rs",
        ]);

        ctrl.handle_command(SessionCommand::SetThinkingLevel {
            level: "high".to_string(),
        })
        .await;

        assert_eq!(ctrl.core_state.thinking_level, clankers_core::CoreThinkingLevel::High);
        let agent = ctrl.agent.as_ref().expect("controller should retain agent");
        assert_eq!(agent.thinking_level(), ThinkingLevel::High);
        assert!(matches!(
            ctrl.drain_events().as_slice(),
            [DaemonEvent::SystemMessage { text, is_error: false }] if text.contains("Thinking: off → high")
        ));
        assert_sources_do_not_contain_symbols(&engine_and_agent_turn_sources, &thinking_symbols);
    }

    #[tokio::test]
    async fn disabled_tool_effects_remain_core_owned() {
        let mut ctrl = make_test_controller();
        let rebuilder = RecordingRebuilder::default();
        ctrl.set_tool_rebuilder(Arc::new(rebuilder.clone()));
        let tools = vec!["bash".to_string(), "read".to_string()];
        let disabled_tool_symbols = [
            ["Set", "DisabledTools"].concat(),
            ["Apply", "ToolFilter"].concat(),
            ["Tool", "FilterApplied"].concat(),
        ];
        let engine_and_agent_turn_sources = workspace_sources(&[
            "clankers-engine/src/lib.rs",
            "clankers-agent/src/turn/mod.rs",
            "clankers-agent/src/turn/execution.rs",
        ]);

        ctrl.handle_command(SessionCommand::SetDisabledTools { tools: tools.clone() }).await;

        assert_eq!(ctrl.core_state.disabled_tools, tools);
        assert_eq!(rebuilder.take_calls(), vec![tools.clone()]);
        assert!(matches!(
            ctrl.drain_events().as_slice(),
            [
                DaemonEvent::DisabledToolsChanged { tools: changed_tools },
                DaemonEvent::SystemMessage { text, is_error: false },
            ] if changed_tools == &tools && text.contains("Disabled tools updated")
        ));
        assert_sources_do_not_contain_symbols(&engine_and_agent_turn_sources, &disabled_tool_symbols);
    }

    #[tokio::test]
    async fn test_set_disabled_tools_consumes_reducer_effect_and_emits_change_before_ack() {
        let mut ctrl = make_test_controller();
        let rebuilder = RecordingRebuilder::default();
        ctrl.set_tool_rebuilder(Arc::new(rebuilder.clone()));
        let tools = vec!["bash".to_string(), "read".to_string()];
        let expected_outcome = clankers_core::reduce(
            &ctrl.core_state,
            &CoreInput::SetDisabledTools(DisabledToolsUpdate {
                requested_disabled_tools: tools.clone(),
            }),
        );
        assert!(matches!(
            expected_outcome,
            CoreOutcome::Transitioned { next_state, effects }
                if next_state.disabled_tools == tools
                    && next_state.pending_tool_filter
                        == Some(clankers_core::PendingToolFilterState {
                            effect_id: FIRST_EFFECT_ID,
                            requested_disabled_tools: tools.clone(),
                        })
                    && effects
                        == vec![CoreEffect::ApplyToolFilter {
                            effect_id: FIRST_EFFECT_ID,
                            disabled_tools: tools.clone(),
                        }]
        ));

        ctrl.handle_command(SessionCommand::SetDisabledTools { tools: tools.clone() }).await;

        assert_eq!(rebuilder.take_calls(), vec![tools.clone()]);
        assert_eq!(ctrl.disabled_tools, tools);
        assert!(ctrl.core_state.pending_tool_filter.is_none());
        let events = ctrl.drain_events();
        assert!(matches!(
            events.as_slice(),
            [
                DaemonEvent::DisabledToolsChanged { tools: changed_tools },
                DaemonEvent::SystemMessage { text, is_error: false },
            ] if changed_tools == &tools && text.contains("Disabled tools updated")
        ));
    }

    #[tokio::test]
    async fn test_set_disabled_tools_rejects_stale_pending_rebuild_without_change_event() {
        let mut ctrl = make_test_controller();
        let rebuilder = RecordingRebuilder::default();
        ctrl.set_tool_rebuilder(Arc::new(rebuilder.clone()));
        seed_pending_tool_filter(&mut ctrl, vec!["bash".to_string()]);
        let previous_state = ctrl.core_state.clone();
        let requested_tools = vec!["read".to_string()];
        let expected_outcome = clankers_core::reduce(
            &previous_state,
            &CoreInput::SetDisabledTools(DisabledToolsUpdate {
                requested_disabled_tools: requested_tools.clone(),
            }),
        );
        assert!(matches!(
            expected_outcome,
            CoreOutcome::Rejected {
                unchanged_state,
                error: clankers_core::CoreError::ToolFilterStillPending,
            } if unchanged_state == previous_state
        ));

        ctrl.handle_command(SessionCommand::SetDisabledTools { tools: requested_tools }).await;

        assert_eq!(ctrl.core_state, previous_state);
        assert!(rebuilder.take_calls().is_empty());
        assert!(matches!(ctrl.drain_events().as_slice(), [DaemonEvent::SystemMessage { is_error: true, .. }]));
    }

    #[test]
    fn test_tool_filter_feedback_success_emits_disabled_tools_changed_from_reducer_event() {
        let mut ctrl = make_test_controller();
        let disabled_tools = vec!["bash".to_string()];
        seed_pending_tool_filter(&mut ctrl, disabled_tools.clone());
        let expected_outcome = clankers_core::reduce(
            &ctrl.core_state,
            &CoreInput::ToolFilterApplied(ToolFilterApplied {
                effect_id: FIRST_EFFECT_ID,
                applied_disabled_tool_set: disabled_tools.clone(),
            }),
        );
        assert!(matches!(
            expected_outcome,
            CoreOutcome::Transitioned { next_state, effects }
                if next_state.pending_tool_filter.is_none()
                    && effects
                        == vec![CoreEffect::EmitLogicalEvent(CoreLogicalEvent::DisabledToolsChanged {
                            disabled_tools: disabled_tools.clone(),
                        })]
        ));

        let applied = ctrl.apply_tool_filter_feedback(ToolFilterApplied {
            effect_id: FIRST_EFFECT_ID,
            applied_disabled_tool_set: disabled_tools.clone(),
        });

        assert!(applied);
        assert!(ctrl.core_state.pending_tool_filter.is_none());
        assert!(matches!(
            ctrl.drain_events().as_slice(),
            [DaemonEvent::DisabledToolsChanged { tools }] if tools == &disabled_tools
        ));
    }

    #[test]
    fn test_tool_filter_feedback_mismatch_rejection_keeps_state_and_emits_only_error() {
        let mut ctrl = make_test_controller();
        seed_pending_tool_filter(&mut ctrl, vec!["bash".to_string()]);

        assert_tool_filter_feedback_rejected(
            &mut ctrl,
            ToolFilterApplied {
                effect_id: clankers_core::CoreEffectId(FIRST_EFFECT_ID.0 + 1),
                applied_disabled_tool_set: vec!["bash".to_string()],
            },
            clankers_core::CoreError::ToolFilterMismatch {
                effect_id: clankers_core::CoreEffectId(FIRST_EFFECT_ID.0 + 1),
            },
        );
    }

    #[test]
    fn test_tool_filter_feedback_out_of_order_rejection_keeps_state_and_emits_only_error() {
        let mut ctrl = make_test_controller();
        ctrl.core_state.pending_prompt = Some(clankers_core::PendingPromptState {
            effect_id: FIRST_EFFECT_ID,
            prompt_text: "hello".to_string(),
            image_count: 0,
            originating_follow_up_effect_id: None,
        });

        assert_tool_filter_feedback_rejected(
            &mut ctrl,
            ToolFilterApplied {
                effect_id: clankers_core::CoreEffectId(FIRST_EFFECT_ID.0 + 1),
                applied_disabled_tool_set: vec!["bash".to_string()],
            },
            clankers_core::CoreError::OutOfOrderRuntimeResult,
        );
    }

    #[tokio::test]
    async fn test_start_loop_command_sets_active_loop_without_immediate_event() {
        let mut ctrl = make_test_controller();

        ctrl.handle_command(SessionCommand::StartLoop {
            iterations: LOOP_ITERATION_LIMIT,
            prompt: "repeat".to_string(),
            break_condition: None,
        })
        .await;

        assert!(ctrl.has_active_loop());
        assert!(ctrl.drain_events().is_empty());
    }

    #[tokio::test]
    async fn test_stop_loop_command_emits_stop_message() {
        let mut ctrl = make_test_controller();
        ctrl.handle_command(SessionCommand::StartLoop {
            iterations: LOOP_ITERATION_LIMIT,
            prompt: "repeat".to_string(),
            break_condition: None,
        })
        .await;
        assert!(ctrl.has_active_loop());
        assert!(ctrl.drain_events().is_empty());

        ctrl.handle_command(SessionCommand::StopLoop).await;

        assert!(!ctrl.has_active_loop());
        let events = ctrl.drain_events();
        assert!(matches!(
            events.as_slice(),
            [DaemonEvent::SystemMessage { text, is_error: false }] if text.contains("stopped by user")
        ));
    }

    #[tokio::test]
    async fn test_start_loop_rejects_when_already_active() {
        let mut ctrl = make_test_controller();
        ctrl.handle_command(SessionCommand::StartLoop {
            iterations: LOOP_ITERATION_LIMIT,
            prompt: "repeat".to_string(),
            break_condition: None,
        })
        .await;
        let previous_state = ctrl.core_state.clone();

        ctrl.handle_command(SessionCommand::StartLoop {
            iterations: LOOP_ITERATION_LIMIT,
            prompt: "again".to_string(),
            break_condition: None,
        })
        .await;

        assert_eq!(ctrl.core_state, previous_state);
        assert!(matches!(ctrl.drain_events().as_slice(), [DaemonEvent::SystemMessage { is_error: true, .. }]));
    }

    #[tokio::test]
    async fn test_stop_loop_rejects_without_active_loop() {
        let mut ctrl = make_test_controller();
        let previous_state = ctrl.core_state.clone();

        ctrl.handle_command(SessionCommand::StopLoop).await;

        assert_eq!(ctrl.core_state, previous_state);
        assert!(matches!(ctrl.drain_events().as_slice(), [DaemonEvent::SystemMessage { is_error: true, .. }]));
    }

    #[test]
    fn test_slash_stop_routes_through_reducer_backed_handler() {
        let mut ctrl = make_test_controller();
        ctrl.handle_slash_command_sync("stop", "");
        assert!(matches!(ctrl.drain_events().as_slice(), [DaemonEvent::SystemMessage { is_error: true, .. }]));

        ctrl.start_loop(crate::loop_mode::LoopConfig {
            name: "slash-loop".to_string(),
            prompt: Some("repeat".to_string()),
            max_iterations: LOOP_ITERATION_LIMIT,
            break_text: None,
        });
        ctrl.drain_events();

        ctrl.handle_slash_command_sync("stop", "");
        assert!(!ctrl.has_active_loop());
        assert!(matches!(
            ctrl.drain_events().as_slice(),
            [DaemonEvent::SystemMessage { text, is_error: false }] if text.contains("stopped by user")
        ));
    }

    #[tokio::test]
    async fn test_loop_control_rejects_when_follow_up_pending() {
        let mut ctrl = make_test_controller();
        ctrl.core_state.pending_follow_up_state = Some(clankers_core::PendingFollowUpState {
            effect_id: clankers_core::CoreEffectId(1),
            prompt_text: "continue loop".to_string(),
            source: clankers_core::FollowUpSource::LoopContinuation,
            stage: clankers_core::PendingFollowUpStage::AwaitingPromptCompletion,
        });
        let previous_state = ctrl.core_state.clone();

        ctrl.handle_command(SessionCommand::StartLoop {
            iterations: LOOP_ITERATION_LIMIT,
            prompt: "repeat".to_string(),
            break_condition: None,
        })
        .await;
        assert_eq!(ctrl.core_state, previous_state);
        assert!(matches!(ctrl.drain_events().as_slice(), [DaemonEvent::SystemMessage { is_error: true, .. }]));

        ctrl.core_state.pending_follow_up_state = previous_state.pending_follow_up_state.clone();
        ctrl.core_state.active_loop_state = Some(clankers_core::ActiveLoopState {
            loop_id: "loop-test".to_string(),
            prompt_text: "repeat".to_string(),
            current_iteration: 1,
            max_iterations: LOOP_ITERATION_LIMIT,
            break_condition: None,
        });
        let previous_stop_state = ctrl.core_state.clone();

        ctrl.handle_command(SessionCommand::StopLoop).await;

        assert_eq!(ctrl.core_state, previous_stop_state);
        assert!(matches!(ctrl.drain_events().as_slice(), [DaemonEvent::SystemMessage { is_error: true, .. }]));
    }

    #[tokio::test]
    async fn test_seed_messages() {
        let mut ctrl = make_test_controller();
        ctrl.handle_command(SessionCommand::SeedMessages {
            messages: vec![
                clankers_protocol::SerializedMessage {
                    role: "user".to_string(),
                    content: "hello".to_string(),
                    model: None,
                    timestamp: None,
                },
                clankers_protocol::SerializedMessage {
                    role: "assistant".to_string(),
                    content: "hi".to_string(),
                    model: Some("opus".to_string()),
                    timestamp: None,
                },
            ],
        })
        .await;

        let events = ctrl.drain_events();
        assert!(events.iter().any(|e| matches!(e, DaemonEvent::SystemMessage { text, .. } if text.contains("2"))));
        // Agent should have 2 messages now
        assert_eq!(ctrl.agent.as_ref().unwrap().messages().len(), 2);
    }

    // ── SetCapabilities tests ─────────────────────────────────────────

    #[tokio::test]
    async fn test_set_capabilities_no_ceiling_allows_anything() {
        let mut ctrl = make_test_controller();
        // No ceiling (local session) — anything goes
        assert!(ctrl.capability_ceiling.is_none());

        ctrl.handle_command(SessionCommand::SetCapabilities {
            capabilities: Some(vec!["read,grep".to_string()]),
        })
        .await;

        let events = ctrl.drain_events();
        assert!(events.iter().any(|e| matches!(
            e,
            DaemonEvent::SystemMessage { text, is_error: false } if text.contains("updated")
        )));
        assert_eq!(ctrl.capabilities, Some(vec!["read,grep".to_string()]));
    }

    #[tokio::test]
    async fn test_set_capabilities_within_ceiling_succeeds() {
        let mut ctrl = make_test_controller();
        ctrl.capability_ceiling = Some(vec!["read,grep,bash".to_string()]);
        ctrl.capabilities = Some(vec!["read,grep,bash".to_string()]);

        // Narrow to just read,grep — within ceiling
        ctrl.handle_command(SessionCommand::SetCapabilities {
            capabilities: Some(vec!["read,grep".to_string()]),
        })
        .await;

        let events = ctrl.drain_events();
        assert!(events.iter().any(|e| matches!(
            e,
            DaemonEvent::SystemMessage { text, is_error: false } if text.contains("updated")
        )));
        assert_eq!(ctrl.capabilities, Some(vec!["read,grep".to_string()]));
    }

    #[tokio::test]
    async fn test_set_capabilities_exceeding_ceiling_rejected() {
        let mut ctrl = make_test_controller();
        ctrl.capability_ceiling = Some(vec!["read,grep".to_string()]);
        ctrl.capabilities = Some(vec!["read,grep".to_string()]);

        // Try to add bash — exceeds ceiling
        ctrl.handle_command(SessionCommand::SetCapabilities {
            capabilities: Some(vec!["read,grep,bash".to_string()]),
        })
        .await;

        let events = ctrl.drain_events();
        assert!(events.iter().any(|e| matches!(e, DaemonEvent::SystemMessage { is_error: true, .. })));
        // Capabilities unchanged
        assert_eq!(ctrl.capabilities, Some(vec!["read,grep".to_string()]));
    }

    #[tokio::test]
    async fn test_set_capabilities_none_with_ceiling_rejected() {
        let mut ctrl = make_test_controller();
        ctrl.capability_ceiling = Some(vec!["read".to_string()]);
        ctrl.capabilities = Some(vec!["read".to_string()]);

        // Try to remove all restrictions — exceeds ceiling
        ctrl.handle_command(SessionCommand::SetCapabilities { capabilities: None }).await;

        let events = ctrl.drain_events();
        assert!(events.iter().any(|e| matches!(e, DaemonEvent::SystemMessage { is_error: true, .. })));
        // Capabilities unchanged
        assert_eq!(ctrl.capabilities, Some(vec!["read".to_string()]));
    }

    #[tokio::test]
    async fn test_set_capabilities_none_without_ceiling_succeeds() {
        let mut ctrl = make_test_controller();
        // No ceiling, currently restricted
        ctrl.capabilities = Some(vec!["read".to_string()]);

        // Remove restrictions — allowed since no ceiling
        ctrl.handle_command(SessionCommand::SetCapabilities { capabilities: None }).await;

        let events = ctrl.drain_events();
        assert!(events.iter().any(|e| matches!(
            e,
            DaemonEvent::SystemMessage { text, is_error: false } if text.contains("full access")
        )));
        assert!(ctrl.capabilities.is_none());
    }

    #[tokio::test]
    async fn test_set_capabilities_restore_to_ceiling() {
        let mut ctrl = make_test_controller();
        ctrl.capability_ceiling = Some(vec!["read,grep,bash".to_string()]);
        ctrl.capabilities = Some(vec!["read".to_string()]);

        // Restore to full ceiling — allowed
        ctrl.handle_command(SessionCommand::SetCapabilities {
            capabilities: Some(vec!["read,grep,bash".to_string()]),
        })
        .await;

        let events = ctrl.drain_events();
        assert!(events.iter().any(|e| matches!(
            e,
            DaemonEvent::SystemMessage { text, is_error: false } if text.contains("updated")
        )));
        assert_eq!(ctrl.capabilities, Some(vec!["read,grep,bash".to_string()]));
    }
}
