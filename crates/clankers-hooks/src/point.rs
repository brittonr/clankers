use serde::{Serialize, Deserialize};

/// All hook points in the system.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HookPoint {
    // Agent lifecycle
    PrePrompt,
    PostPrompt,
    SessionStart,
    SessionEnd,
    OnError,
    // Tool lifecycle
    PreTool,
    PostTool,
    // Git lifecycle
    PreCommit,
    PostCommit,
    // Turn lifecycle
    TurnStart,
    TurnEnd,
    // Model
    ModelChange,
}

impl HookPoint {
    /// Kebab-case filename for script hooks (e.g., "pre-tool").
    pub fn to_filename(&self) -> &'static str {
        match self {
            Self::PrePrompt => "pre-prompt",
            Self::PostPrompt => "post-prompt",
            Self::SessionStart => "session-start",
            Self::SessionEnd => "session-end",
            Self::OnError => "on-error",
            Self::PreTool => "pre-tool",
            Self::PostTool => "post-tool",
            Self::PreCommit => "pre-commit",
            Self::PostCommit => "post-commit",
            Self::TurnStart => "turn-start",
            Self::TurnEnd => "turn-end",
            Self::ModelChange => "model-change",
        }
    }

    /// Whether this hook point can deny/modify the operation.
    pub fn is_pre_hook(&self) -> bool {
        matches!(self, Self::PrePrompt | Self::PreTool | Self::PreCommit)
    }

    /// All hook points (for iteration).
    pub fn all() -> &'static [HookPoint] {
        &[
            Self::PrePrompt, Self::PostPrompt,
            Self::SessionStart, Self::SessionEnd,
            Self::OnError,
            Self::PreTool, Self::PostTool,
            Self::PreCommit, Self::PostCommit,
            Self::TurnStart, Self::TurnEnd,
            Self::ModelChange,
        ]
    }
}

impl std::fmt::Display for HookPoint {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.to_filename())
    }
}
