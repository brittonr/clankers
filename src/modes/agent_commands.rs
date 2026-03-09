//! Agent command and task result types for the interactive event loop.

/// Commands sent from the TUI event loop to the background agent task.
pub(crate) enum AgentCommand {
    Prompt(String),
    PromptWithImages {
        text: String,
        images: Vec<crate::tui::app::PendingImage>,
    },
    Abort,
    ResetCancel,
    SetModel(String),
    ClearHistory,
    TruncateMessages(usize),
    SetThinkingLevel(crate::provider::ThinkingLevel),
    CycleThinkingLevel,
    SeedMessages(Vec<crate::provider::message::AgentMessage>),
    Quit,
    Login {
        code: String,
        state: String,
        verifier: String,
        account: String,
    },
    /// Replace the agent's system prompt
    SetSystemPrompt(String),
    /// Get the current system prompt
    GetSystemPrompt(tokio::sync::oneshot::Sender<String>),
    /// Switch the active account (hot-swap credentials)
    SwitchAccount(String),
    /// Update the set of disabled tools (rebuilds the agent's tool set)
    SetDisabledTools(std::collections::HashSet<String>),
}

/// Results sent back from the background agent task to the event loop.
pub(crate) enum TaskResult {
    PromptDone(Option<crate::error::Error>),
    LoginDone(std::result::Result<String, String>),
    ThinkingToggled(String, crate::provider::ThinkingLevel),
    AccountSwitched(std::result::Result<String, String>),
}
