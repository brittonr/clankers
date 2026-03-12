//! Command handling and prompt execution.
//!
//! Contains the main command dispatch and prompt processing logic.

use clankers_agent::AgentError;
use clankers_message::{
    AgentMessage, AssistantMessage, Content, MessageId, StopReason, UserMessage,
};
use clankers_protocol::{DaemonEvent, ImageData, SerializedMessage, SessionCommand};
use clankers_provider::message::Content as ProviderContent;
use tracing::{info, warn};

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
                let prompt = self
                    .agent
                    .as_ref()
                    .map(|a| a.system_prompt().to_string())
                    .unwrap_or_default();
                self.emit(DaemonEvent::SystemPromptResponse { prompt });
            }
            SessionCommand::SwitchAccount { account } => {
                self.emit(DaemonEvent::SystemMessage {
                    text: format!("Account switch to '{account}' requested"),
                    is_error: false,
                });
            }
            SessionCommand::SetDisabledTools { tools } => {
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
            SessionCommand::SlashCommand { command, args } => {
                info!("slash command: /{command} {args}");
                self.emit(DaemonEvent::SystemMessage {
                    text: format!("Slash command /{command} {args}"),
                    is_error: false,
                });
            }
            SessionCommand::ReplayHistory => {
                self.replay_history();
            }
            SessionCommand::GetCapabilities => {
                self.emit(DaemonEvent::Capabilities {
                    capabilities: self.capabilities.clone(),
                });
            }
            SessionCommand::Disconnect => {
                tracing::debug!("client disconnected");
            }
        }
    }

    /// Handle a prompt command (daemon mode only).
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
    fn replay_history(&mut self) {
        if let Some(ref agent) = self.agent {
            for msg in agent.messages() {
                let block = serde_json::to_value(msg).unwrap_or_default();
                self.outgoing.push(DaemonEvent::HistoryBlock { block });
            }
        }
        self.emit(DaemonEvent::HistoryEnd);
    }

    /// Convert serialized messages to agent messages for seeding.
    fn convert_seed_messages(
        &self,
        messages: &[SerializedMessage],
    ) -> Vec<AgentMessage> {
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
    use super::*;
    use crate::test_helpers::make_test_controller;
    use clankers_protocol::SessionCommand;

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
        assert!(events.iter().any(
            |e| matches!(e, DaemonEvent::SystemMessage { text, is_error: false } if text.contains("high"))
        ));
    }

    #[tokio::test]
    async fn test_set_thinking_level_invalid() {
        let mut ctrl = make_test_controller();
        ctrl.handle_command(SessionCommand::SetThinkingLevel {
            level: "bogus".to_string(),
        })
        .await;

        let events = ctrl.drain_events();
        assert!(events
            .iter()
            .any(|e| matches!(e, DaemonEvent::SystemMessage { is_error: true, .. })));
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
        assert!(events.iter().any(
            |e| matches!(e, DaemonEvent::SystemMessage { text, .. } if text.contains("2"))
        ));
        // Agent should have 2 messages now
        assert_eq!(ctrl.agent.as_ref().unwrap().messages().len(), 2);
    }
}