//! Configuration for SessionController creation.

use std::sync::Arc;

use clankers_core::CoreThinkingLevel;
use clankers_session::SessionManager;

use crate::ControllerHookService;
use crate::ControllerPersistenceService;

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
    /// Session persistence manager.
    pub session_manager: Option<SessionManager>,
    /// Hook service for lifecycle events.
    pub hook_service: Option<Arc<dyn ControllerHookService>>,
    /// Optional host persistence side effects.
    pub persistence_service: Option<Arc<dyn ControllerPersistenceService>>,
    /// Auto-test command from settings.
    pub auto_test_command: Option<String>,
    /// Whether auto-test is enabled on startup.
    pub auto_test_enabled: bool,
}
