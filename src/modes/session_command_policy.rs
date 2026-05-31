//! Shared session command/effect/ack policy for standalone, daemon attach, and remote attach.
//!
//! Transport shells deliver commands and events; this module owns the reusable
//! policy for local parity effects, daemon commands, acknowledgement suppression,
//! and user-visible messages.

use clankers_protocol::DaemonEvent;
use clankers_protocol::SessionCommand;

use clankers_provider::ThinkingLevel;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum LocalSessionEffect {
    ThinkingLevel { level: ThinkingLevel, message: String },
    DisabledTools { tools: Vec<String> },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SessionAckPolicy {
    ThinkingLevel,
    DisabledTools,
    ManualCompaction,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct SessionCommandEffect {
    pub(crate) local: Option<LocalSessionEffect>,
    pub(crate) command: Option<SessionCommand>,
    pub(crate) ack: SessionAckPolicy,
}

pub(crate) fn set_thinking_level_effect(level: ThinkingLevel) -> SessionCommandEffect {
    SessionCommandEffect {
        local: Some(LocalSessionEffect::ThinkingLevel {
            level,
            message: thinking_level_message(level),
        }),
        command: Some(SessionCommand::SetThinkingLevel {
            level: level.label().to_string(),
        }),
        ack: SessionAckPolicy::ThinkingLevel,
    }
}

pub(crate) fn cycle_thinking_level_effect(current: ThinkingLevel) -> SessionCommandEffect {
    let level = current.next();
    SessionCommandEffect {
        local: Some(LocalSessionEffect::ThinkingLevel {
            level,
            message: thinking_level_message(level),
        }),
        command: Some(SessionCommand::CycleThinkingLevel),
        ack: SessionAckPolicy::ThinkingLevel,
    }
}

pub(crate) fn disabled_tools_effect(disabled: impl IntoIterator<Item = String>) -> SessionCommandEffect {
    let mut tools: Vec<String> = disabled.into_iter().collect();
    tools.sort();
    SessionCommandEffect {
        local: Some(LocalSessionEffect::DisabledTools { tools: tools.clone() }),
        command: Some(SessionCommand::SetDisabledTools { tools }),
        ack: SessionAckPolicy::DisabledTools,
    }
}

pub(crate) fn manual_compaction_effect() -> SessionCommandEffect {
    SessionCommandEffect {
        local: None,
        command: Some(SessionCommand::CompactHistory),
        ack: SessionAckPolicy::ManualCompaction,
    }
}

pub(crate) fn thinking_level_message(level: ThinkingLevel) -> String {
    match level.budget_tokens() {
        Some(tokens) => format!("Thinking: {} ({} tokens)", level.label(), tokens),
        None => "Thinking: off".to_string(),
    }
}

pub(crate) fn ack_matches(policy: SessionAckPolicy, event: &DaemonEvent) -> bool {
    match policy {
        SessionAckPolicy::ThinkingLevel => is_thinking_ack_message(event),
        SessionAckPolicy::DisabledTools => is_disabled_tools_ack_message(event),
        SessionAckPolicy::ManualCompaction => matches!(event, DaemonEvent::SessionCompaction { .. }),
    }
}

pub(crate) fn is_thinking_ack_message(event: &DaemonEvent) -> bool {
    matches!(event, DaemonEvent::SystemMessage { text, is_error: false } if text.starts_with("Thinking"))
}

pub(crate) fn is_disabled_tools_ack_message(event: &DaemonEvent) -> bool {
    matches!(
        event,
        DaemonEvent::SystemMessage { text, is_error: false } if text.starts_with("Disabled tools updated:")
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn thinking_effect_projects_local_message_command_and_ack_policy() {
        let effect = set_thinking_level_effect(ThinkingLevel::High);

        assert_eq!(
            effect.local,
            Some(LocalSessionEffect::ThinkingLevel {
                level: ThinkingLevel::High,
                message: "Thinking: high (32000 tokens)".to_string(),
            })
        );
        assert_eq!(
            effect.command,
            Some(SessionCommand::SetThinkingLevel {
                level: "high".to_string(),
            })
        );
        assert_eq!(effect.ack, SessionAckPolicy::ThinkingLevel);
    }

    #[test]
    fn disabled_tools_effect_sorts_tools_and_expects_ack() {
        let effect = disabled_tools_effect(["web".to_string(), "bash".to_string()]);

        assert_eq!(
            effect.local,
            Some(LocalSessionEffect::DisabledTools {
                tools: vec!["bash".to_string(), "web".to_string()],
            })
        );
        assert_eq!(
            effect.command,
            Some(SessionCommand::SetDisabledTools {
                tools: vec!["bash".to_string(), "web".to_string()],
            })
        );
        assert_eq!(effect.ack, SessionAckPolicy::DisabledTools);
    }

    #[test]
    fn ack_policy_matches_only_expected_daemon_ack_shape() {
        let thinking = DaemonEvent::SystemMessage {
            text: "Thinking: high (31999 tokens)".to_string(),
            is_error: false,
        };
        let disabled = DaemonEvent::SystemMessage {
            text: "Disabled tools updated: bash".to_string(),
            is_error: false,
        };
        let error = DaemonEvent::SystemMessage {
            text: "Thinking failed".to_string(),
            is_error: true,
        };

        assert!(ack_matches(SessionAckPolicy::ThinkingLevel, &thinking));
        assert!(ack_matches(SessionAckPolicy::DisabledTools, &disabled));
        assert!(!ack_matches(SessionAckPolicy::ThinkingLevel, &disabled));
        assert!(!ack_matches(SessionAckPolicy::ThinkingLevel, &error));
    }
}
