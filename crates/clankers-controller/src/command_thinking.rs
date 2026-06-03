//! Thinking-level command policy and core-input translation.
//!
//! This module owns the thinking command cluster for controller command handling:
//! parsing client labels, constructing thinking-related core inputs, and applying
//! thinking runtime-control effects. User-visible projection remains in
//! `command.rs`/`convert.rs` callers via `SessionController::emit`.

use clanker_message::SemanticErrorClass;
use clankers_core::CoreInput;
use clankers_core::CoreOutcome;
use clankers_core::CoreThinkingLevel;
use clankers_core::CoreThinkingLevelInput;
use clankers_protocol::DaemonEvent;

use crate::SessionController;
use crate::convert::semantic_error_message_to_daemon_event;
use crate::runtime_adapter::ControllerRuntimeAdapter;
use crate::runtime_adapter::RuntimeControlRequest;

impl SessionController {
    pub(crate) fn handle_set_thinking_level(&mut self, level: String) {
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
                let event = semantic_error_message_to_daemon_event(
                    &self.session_id,
                    format!("Unknown thinking level: {raw}"),
                    SemanticErrorClass::InvalidInput,
                );
                self.emit(event);
            }
            CoreOutcome::Rejected { .. } => unreachable!("thinking-level input should only reject as invalid"),
        }
    }

    pub(crate) fn handle_cycle_thinking_level(&mut self) {
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

    pub(crate) fn apply_adapter_thinking_level(
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

    fn parse_core_thinking_level_input(level: &str) -> CoreThinkingLevelInput {
        match Self::parse_core_thinking_level(level) {
            Some(parsed) => CoreThinkingLevelInput::Level(parsed),
            None => CoreThinkingLevelInput::Invalid(level.to_string()),
        }
    }

    pub(crate) fn parse_core_thinking_level(level: &str) -> Option<CoreThinkingLevel> {
        match level.trim().to_lowercase().as_str() {
            "off" | "none" | "disable" | "disabled" => Some(CoreThinkingLevel::Off),
            "low" | "lo" | "l" => Some(CoreThinkingLevel::Low),
            "medium" | "med" | "m" => Some(CoreThinkingLevel::Medium),
            "high" | "hi" | "h" => Some(CoreThinkingLevel::High),
            "xhigh" | "x-high" | "extra-high" | "max" | "maximum" | "full" | "default" => Some(CoreThinkingLevel::Max),
            _ => None,
        }
    }

    pub(crate) fn provider_thinking_level(level: CoreThinkingLevel) -> clanker_message::ThinkingLevel {
        match level {
            CoreThinkingLevel::Off => clanker_message::ThinkingLevel::Off,
            CoreThinkingLevel::Low => clanker_message::ThinkingLevel::Low,
            CoreThinkingLevel::Medium => clanker_message::ThinkingLevel::Medium,
            CoreThinkingLevel::High => clanker_message::ThinkingLevel::High,
            CoreThinkingLevel::Max => clanker_message::ThinkingLevel::Max,
        }
    }

    fn thinking_label(level: CoreThinkingLevel) -> &'static str {
        Self::provider_thinking_level(level).label()
    }
}

#[cfg(test)]
mod tests {
    use clankers_core::CoreInput;
    use clankers_core::CoreThinkingLevel;
    use clankers_core::CoreThinkingLevelInput;

    use super::*;

    #[test]
    fn parser_uses_core_levels_without_tui_dto() {
        assert_eq!(SessionController::parse_core_thinking_level("off"), Some(CoreThinkingLevel::Off));
        assert_eq!(SessionController::parse_core_thinking_level("medium"), Some(CoreThinkingLevel::Medium));
        assert_eq!(SessionController::parse_core_thinking_level("xhigh"), Some(CoreThinkingLevel::Max));
        assert_eq!(SessionController::parse_core_thinking_level("bogus"), None);
    }

    #[test]
    fn set_thinking_input_preserves_invalid_label_for_reducer_error() {
        let input = CoreInput::SetThinkingLevel {
            requested: SessionController::parse_core_thinking_level_input("surprising"),
        };
        assert!(matches!(
            input,
            CoreInput::SetThinkingLevel {
                requested: CoreThinkingLevelInput::Invalid(raw),
            } if raw == "surprising"
        ));
    }
}
