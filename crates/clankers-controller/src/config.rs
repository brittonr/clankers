//! Configuration for SessionController creation.

use std::sync::Arc;

use clankers_hooks::HookPipeline;
use clankers_session::SessionManager;

/// Configuration needed to create a SessionController.
#[derive(Default)]
pub struct ControllerConfig {
    /// Session ID.
    pub session_id: String,
    /// Initial model name.
    pub model: String,
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
    /// Hook pipeline for lifecycle events.
    pub hook_pipeline: Option<Arc<HookPipeline>>,
    /// Auto-test command from settings.
    pub auto_test_command: Option<String>,
    /// Whether auto-test is enabled on startup.
    pub auto_test_enabled: bool,
}
