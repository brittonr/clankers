//! Configuration for SessionController creation.

use std::sync::Arc;

pub use clankers_core::CoreThinkingLevel;

use crate::ControllerHookService;
use crate::ControllerPersistenceService;
use crate::ControllerSessionLedger;

/// Convert public message-layer thinking settings to the controller's reducer level.
#[must_use]
pub fn thinking_level_from_message(level: clanker_message::ThinkingLevel) -> CoreThinkingLevel {
    match level {
        clanker_message::ThinkingLevel::Off => CoreThinkingLevel::Off,
        clanker_message::ThinkingLevel::Low => CoreThinkingLevel::Low,
        clanker_message::ThinkingLevel::Medium => CoreThinkingLevel::Medium,
        clanker_message::ThinkingLevel::High => CoreThinkingLevel::High,
        clanker_message::ThinkingLevel::Max => CoreThinkingLevel::Max,
    }
}

/// Configuration needed to create a SessionController.
#[derive(Default)]
pub struct ControllerConfig {
    /// Session ID.
    pub session_id: String,
    /// Initial model name.
    pub model: String,
    /// Initial thinking/reasoning level.
    pub initial_thinking_level: CoreThinkingLevel,
    /// System prompt (set on the agent before passing to controller).
    pub system_prompt: Option<String>,
    /// Capability restrictions (None = full access).
    pub capabilities: Option<Vec<String>>,
    /// Capability ceiling — the maximum capabilities this session can have.
    /// Set from the UCAN token + settings at creation time. Immutable.
    /// `None` = no ceiling (local owner, full access).
    pub capability_ceiling: Option<Vec<String>>,
    /// Session persistence ledger.
    pub session_ledger: Option<Box<dyn ControllerSessionLedger>>,
    /// Hook service for lifecycle events.
    pub hook_service: Option<Arc<dyn ControllerHookService>>,
    /// Optional host persistence side effects.
    pub persistence_service: Option<Arc<dyn ControllerPersistenceService>>,
    /// Auto-test command from settings.
    pub auto_test_command: Option<String>,
    /// Whether auto-test is enabled on startup.
    pub auto_test_enabled: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn thinking_level_from_message_matches_core_reducer_levels() {
        assert_eq!(
            thinking_level_from_message(clanker_message::ThinkingLevel::Off),
            CoreThinkingLevel::Off
        );
        assert_eq!(
            thinking_level_from_message(clanker_message::ThinkingLevel::Low),
            CoreThinkingLevel::Low
        );
        assert_eq!(
            thinking_level_from_message(clanker_message::ThinkingLevel::Medium),
            CoreThinkingLevel::Medium
        );
        assert_eq!(
            thinking_level_from_message(clanker_message::ThinkingLevel::High),
            CoreThinkingLevel::High
        );
        assert_eq!(
            thinking_level_from_message(clanker_message::ThinkingLevel::Max),
            CoreThinkingLevel::Max
        );
    }
}
