//! Command handling and prompt execution.
//!
//! Contains the main command dispatch and prompt processing logic.

use clankers_agent::AgentError;
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
                if let Some(parsed) = clankers_tui_types::ThinkingLevel::from_str_or_budget(&level) {
                    if let Some(ref mut agent) = self.agent {
                        let prev = agent.set_thinking_level(parsed);
                        self.emit(DaemonEvent::SystemMessage {
                            text: format!("Thinking: {} → {}", prev.label(), parsed.label()),
                            is_error: false,
                        });
                    }
                } else {
                    self.emit(DaemonEvent::SystemMessage {
                        text: format!("Unknown thinking level: {level}"),
                        is_error: true,
                    });
                }
            }
            SessionCommand::CycleThinkingLevel => {
                if let Some(ref mut agent) = self.agent {
                    agent.cycle_thinking_level();
                }
                self.emit(DaemonEvent::SystemMessage {
                    text: "Thinking level cycled".to_string(),
                    is_error: false,
                });
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
                self.disabled_tools = tools.clone();
                // Rebuild tools with new disabled set if we have a tool rebuilder
                if let Some(ref rebuilder) = self.tool_rebuilder {
                    let filtered = rebuilder.rebuild_filtered(&tools);
                    if let Some(ref mut agent) = self.agent {
                        agent.set_tools(filtered);
                    }
                }
                self.emit(DaemonEvent::DisabledToolsChanged { tools: tools.clone() });
                self.emit(DaemonEvent::SystemMessage {
                    text: format!("Disabled tools updated: {}", tools.join(", ")),
                    is_error: false,
                });
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
                    let before = agent.messages().len();
                    agent.compact_messages();
                    let after = agent.messages().len();
                    self.emit(DaemonEvent::SessionCompaction {
                        compacted_count: before.saturating_sub(after),
                        tokens_saved: 0,
                    });
                }
            }
            SessionCommand::StartLoop {
                iterations,
                prompt,
                break_condition,
            } => {
                let config = crate::loop_mode::LoopConfig {
                    name: format!("loop-{}", self.session_id),
                    prompt: Some(prompt),
                    max_iterations: iterations,
                    break_text: break_condition,
                };
                self.start_loop(config);
            }
            SessionCommand::StopLoop => {
                self.stop_loop();
            }
            SessionCommand::SetAutoTest { enabled, command } => {
                self.auto_test_enabled = enabled;
                if let Some(cmd) = command.clone() {
                    self.auto_test_command = Some(cmd);
                }
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

        if self.busy {
            self.outgoing.push(DaemonEvent::SystemMessage {
                text: "A prompt is already in progress".to_string(),
                is_error: true,
            });
            return;
        }

        self.busy = true;
        self.outgoing.push(DaemonEvent::AgentStart);

        // Take the agent out to avoid borrow conflicts with self
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

        // Put the agent back
        self.agent = Some(agent);
        self.busy = false;

        match result {
            Ok(()) => {
                self.emit(DaemonEvent::PromptDone { error: None });
            }
            Err(AgentError::Cancelled) => {
                self.emit(DaemonEvent::PromptDone {
                    error: Some("cancelled".to_string()),
                });
            }
            Err(e) => {
                self.emit(DaemonEvent::PromptDone {
                    error: Some(e.to_string()),
                });
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
                    let before = agent.messages().len();
                    agent.compact_messages();
                    let after = agent.messages().len();
                    self.emit(DaemonEvent::SessionCompaction {
                        compacted_count: before.saturating_sub(after),
                        tokens_saved: 0,
                    });
                }
            }
            "thinking" => {
                if let Some(ref mut agent) = self.agent {
                    if args.is_empty() {
                        let level = agent.cycle_thinking_level();
                        self.emit(DaemonEvent::SystemMessage {
                            text: format!("Thinking: {}", level.label()),
                            is_error: false,
                        });
                    } else if let Some(parsed) = clankers_tui_types::ThinkingLevel::from_str_or_budget(args) {
                        let prev = agent.set_thinking_level(parsed);
                        self.emit(DaemonEvent::ThinkingLevelChanged {
                            from: prev.label().to_string(),
                            to: parsed.label().to_string(),
                        });
                    } else {
                        self.emit(DaemonEvent::SystemMessage {
                            text: format!("Unknown thinking level: {args}"),
                            is_error: true,
                        });
                    }
                }
            }
            "stop" => {
                self.stop_loop();
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
    use clankers_protocol::SessionCommand;

    use super::*;
    use crate::test_helpers::make_test_controller;

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
    async fn test_reject_concurrent_prompt() {
        let mut ctrl = make_test_controller();
        ctrl.busy = true;

        ctrl.handle_command(SessionCommand::Prompt {
            text: "hello".to_string(),
            images: vec![],
        })
        .await;

        let events = ctrl.drain_events();
        assert!(events.iter().any(|e| matches!(e, DaemonEvent::SystemMessage { is_error: true, .. })));
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
        ctrl.handle_command(SessionCommand::SetThinkingLevel {
            level: "bogus".to_string(),
        })
        .await;

        let events = ctrl.drain_events();
        assert!(events.iter().any(|e| matches!(e, DaemonEvent::SystemMessage { is_error: true, .. })));
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
