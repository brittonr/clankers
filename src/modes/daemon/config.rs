//! Daemon configuration types.

use crate::config::settings::Settings;

/// Chat ALPN — conversational sessions with persistent memory.
pub const ALPN_CHAT: &[u8] = b"clankers/chat/1";

/// Daemon configuration.
#[derive(Debug, Clone)]
pub struct DaemonConfig {
    /// Model to use
    pub model: String,
    /// System prompt
    pub system_prompt: String,
    /// Settings
    pub settings: Settings,
    /// Capability tags for announcements
    pub tags: Vec<String>,
    /// Allow all iroh peers (no ACL)
    pub allow_all: bool,
    /// Enable Matrix bridge
    pub enable_matrix: bool,
    /// Heartbeat interval (0 = disabled)
    pub heartbeat_secs: u64,
    /// Maximum concurrent sessions
    pub max_sessions: usize,
    /// Idle session timeout in seconds (0 = disabled)
    pub idle_timeout_secs: u64,
    /// Matrix user allowlist (empty = allow all). Overridden by
    /// `CLANKERS_MATRIX_ALLOWED_USERS` env var (comma-separated).
    pub matrix_allowed_users: Vec<String>,
    /// Per-session heartbeat interval in seconds (0 = disabled).
    /// The daemon periodically reads each session's `HEARTBEAT.md`
    /// and prompts the agent with its contents. If the agent responds
    /// with "HEARTBEAT_OK", the response is suppressed.
    pub session_heartbeat_secs: u64,
    /// Prompt text prepended to HEARTBEAT.md contents.
    pub heartbeat_prompt: String,
    /// Enable per-session trigger pipes. When enabled, a FIFO at
    /// `{session_dir}/trigger.pipe` lets external processes inject
    /// prompts into the agent session.
    pub trigger_pipe_enabled: bool,
    /// Drain timeout in seconds for graceful shutdown/restart (default: 10).
    pub drain_timeout_secs: u64,
}

impl Default for DaemonConfig {
    fn default() -> Self {
        Self {
            model: "claude-sonnet-4-5".to_string(),
            system_prompt: crate::agent::system_prompt::default_system_prompt(
                &crate::agent::system_prompt::PromptFeatures {
                    nix_available: crate::agent::system_prompt::detect_nix(),
                    multi_model: true,
                    daemon_mode: true,
                    process_monitor: true,
                },
            ),
            settings: Settings::default(),
            tags: Vec::new(),
            allow_all: false,
            enable_matrix: false,
            heartbeat_secs: 60,
            max_sessions: 32,
            idle_timeout_secs: 1800, // 30 minutes
            matrix_allowed_users: Vec::new(),
            session_heartbeat_secs: 300, // 5 minutes
            heartbeat_prompt: "Check your HEARTBEAT.md for pending tasks. \
                If nothing needs attention, respond with HEARTBEAT_OK."
                .to_string(),
            trigger_pipe_enabled: true,
            drain_timeout_secs: 10,
        }
    }
}

/// Config subset passed to the Matrix bridge for proactive agent features.
#[derive(Debug, Clone)]
pub(crate) struct ProactiveConfig {
    pub(crate) session_heartbeat_secs: u64,
    pub(crate) heartbeat_prompt: String,
    pub(crate) trigger_pipe_enabled: bool,
}
