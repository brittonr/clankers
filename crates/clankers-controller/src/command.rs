//! Command handling and prompt execution.
//!
//! Contains the main command dispatch and prompt processing logic.

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
use clankers_message::AgentMessage;
use clankers_message::AssistantMessage;
use clankers_message::Content;
use clankers_message::MessageId;
use clankers_message::StopReason;
use clankers_message::UserMessage;
use clankers_protocol::DaemonEvent;
use clankers_protocol::ImageData;
use clankers_protocol::SerializedMessage;
use clankers_protocol::SessionCommand;
use clankers_provider::message::Content as ProviderContent;
use tracing::info;
use tracing::warn;

use crate::SessionController;

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
                let from = self.model.clone();
                if let Some(ref mut agent) = self.agent {
                    agent.set_model(model.clone());
                }
                self.model = model.clone();
                self.emit(DaemonEvent::ModelChanged {
                    from,
                    to: model,
                    reason: "user request".to_string(),
                });
            }
            SessionCommand::ClearHistory => {
                if let Some(ref mut agent) = self.agent {
                    agent.clear_messages();
                }
                self.emit(DaemonEvent::SystemMessage {
                    text: "History cleared".to_string(),
                    is_error: false,
                });
            }
            SessionCommand::TruncateMessages { count } => {
                if let Some(ref mut agent) = self.agent {
                    agent.truncate_messages(count);
                }
                self.emit(DaemonEvent::SystemMessage {
                    text: format!("Truncated to {count} messages"),
                    is_error: false,
                });
            }
            SessionCommand::SetThinkingLevel { level } => {
                self.handle_set_thinking_level(level);
            }
            SessionCommand::CycleThinkingLevel => {
                self.handle_cycle_thinking_level();
            }
            SessionCommand::SeedMessages { messages } => {
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
                self.handle_set_disabled_tools(tools);
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
                // Remove the last user message and re-prompt
                if let Some(ref mut agent) = self.agent {
                    agent.pop_last_exchange();
                }
                self.handle_prompt(text, vec![]).await;
            }
            SessionCommand::CompactHistory => {
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
                self.handle_start_loop(iterations, prompt, break_condition);
            }
            SessionCommand::StopLoop => {
                self.handle_stop_loop();
            }
            SessionCommand::SetAutoTest { enabled, command } => {
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

    /// Handle a prompt command (daemon mode only).
    #[cfg_attr(
        dylint_lib = "tigerstyle",
        allow(no_unwrap, reason = "agent is always Some when handle_prompt is called")
    )]
    async fn handle_prompt(&mut self, text: String, images: Vec<ImageData>) {
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

        let prompt_effect_id = match clankers_core::reduce(&self.core_state, &prompt_input) {
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

        self.outgoing.push(DaemonEvent::AgentStart);

        // Take the agent out to avoid borrow conflicts with self.
        let mut agent = self.agent.take().unwrap();
        let result = if images.is_empty() {
            agent.prompt(&text).await
        } else {
            let image_content: Vec<ProviderContent> = images
                .into_iter()
                .map(|img| ProviderContent::Image {
                    source: clankers_provider::message::ImageSource::Base64 {
                        media_type: img.media_type,
                        data: img.data,
                    },
                })
                .collect();
            agent.prompt_with_images(&text, image_content).await
        };
        self.agent = Some(agent);

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
                    let from = self.model.clone();
                    if let Some(ref mut agent) = self.agent {
                        agent.set_model(args.to_string());
                    }
                    self.model = args.to_string();
                    self.emit(DaemonEvent::ModelChanged {
                        from,
                        to: args.to_string(),
                        reason: "slash command".to_string(),
                    });
                }
            }
            "clear" => {
                if let Some(ref mut agent) = self.agent {
                    agent.clear_messages();
                }
                self.emit(DaemonEvent::SystemMessage {
                    text: "History cleared".to_string(),
                    is_error: false,
                });
            }
            "compact" => {
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
                    self.handle_cycle_thinking_level();
                } else {
                    self.handle_set_thinking_level(args.to_string());
                }
            }
            "stop" => {
                self.handle_stop_loop();
            }
            "autotest" => {
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

    use clankers_core::CoreEffect;
    use clankers_core::CoreLogicalEvent;
    use clankers_protocol::SessionCommand;
    use clankers_provider::ThinkingLevel;

    use super::*;
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

    #[async_trait::async_trait]
    impl clankers_provider::Provider for RecordingPromptProvider {
        async fn complete(
            &self,
            request: clankers_provider::CompletionRequest,
            _tx: tokio::sync::mpsc::Sender<clankers_provider::streaming::StreamEvent>,
        ) -> clankers_provider::error::Result<()> {
            let prompt_text =
                extract_last_user_prompt_text(&request.messages).expect("prompt request should carry a user message");
            self.requests.lock().unwrap_or_else(|poisoned| poisoned.into_inner()).push(RecordedPromptRequest {
                model: request.model,
                prompt_text,
                system_prompt: request.system_prompt,
                session_id: request
                    .extra_params
                    .get("_session_id")
                    .and_then(|value| value.as_str())
                    .map(str::to_string),
            });
            Ok(())
        }

        fn models(&self) -> &[clankers_provider::Model] {
            &[]
        }

        fn name(&self) -> &str {
            "recording"
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

    fn make_test_controller_with_provider(provider: Arc<dyn clankers_provider::Provider>) -> SessionController {
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
        assert!(matches!(ctrl.take_outgoing().as_slice(), [DaemonEvent::AgentStart, DaemonEvent::PromptDone {
            error: None
        }]));
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
