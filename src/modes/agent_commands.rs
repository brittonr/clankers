//! Agent command and task result types for the interactive event loop.

/// Commands sent from the TUI event loop to the background agent task.
pub(crate) enum AgentCommand {
    Prompt(String),
    PromptWithImages {
        text: String,
        images: Vec<crate::tui::app::PendingImage>,
    },
    /// Rewrite/improve the prompt before sending it to the agent.
    RewriteAndPrompt(String),
    /// Rewrite/improve the prompt (with images) before sending it to the agent.
    RewriteAndPromptWithImages {
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
    SetSessionId(String),
    Quit,
    Login {
        code: String,
        state: String,
        verifier: String,
        provider: String,
        account: String,
    },
    /// Replace the agent's system prompt
    SetSystemPrompt(String),
    /// Get the current system prompt
    GetSystemPrompt(tokio::sync::oneshot::Sender<String>),
    /// Switch the active Anthropic account (hot-swap credentials)
    SwitchAccount(String),
    /// Switch the active account for an explicit provider.
    SwitchProviderAccount {
        provider: String,
        account: String,
    },
    /// Reload provider credentials from disk after auth-store changes.
    ReloadCredentials,
    /// Update the set of disabled tools (rebuilds the agent's tool set)
    SetDisabledTools(std::collections::HashSet<String>),
    /// Request context compression (summarize older messages)
    CompressContext,
}

/// Results sent back from the background agent task to the event loop.
pub(crate) enum TaskResult {
    PromptDone(Option<crate::error::Error>),
    LoginDone(std::result::Result<String, String>),
    ThinkingToggled(String, crate::provider::ThinkingLevel),
    AccountSwitched(std::result::Result<String, String>),
}
