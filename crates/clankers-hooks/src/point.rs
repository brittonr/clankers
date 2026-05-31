use serde::Deserialize;
use serde::Serialize;

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
    /// Blocking prompt-level turn gate fired before model/tool execution.
    PreTurn,
    /// Non-blocking transcript/model-turn notification fired when a model turn starts.
    TurnStart,
    /// Non-blocking transcript/model-turn notification fired when a model turn ends.
    TurnEnd,
    /// Observational prompt-level turn hook fired after the agent turn outcome is known.
    PostTurn,
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
            Self::PreTurn => "pre-turn",
            Self::TurnStart => "turn-start",
            Self::TurnEnd => "turn-end",
            Self::PostTurn => "post-turn",
            Self::ModelChange => "model-change",
        }
    }

    /// Whether this hook point can deny the operation and fail closed on hook errors.
    pub fn is_pre_hook(&self) -> bool {
        matches!(self, Self::PrePrompt | Self::PreTool | Self::PreCommit | Self::PreTurn)
    }

    /// Whether this pre-hook has an explicit payload mutation contract.
    pub fn allows_modify(&self) -> bool {
        matches!(self, Self::PrePrompt | Self::PreTool | Self::PreCommit)
    }

    /// Snake-case plugin event kind for hooks exposed through the plugin event protocol.
    pub fn plugin_event_kind(&self) -> Option<&'static str> {
        match self {
            Self::PrePrompt | Self::PostPrompt => Some("user_input"),
            Self::SessionStart => Some("session_start"),
            Self::SessionEnd => Some("session_end"),
            Self::OnError => None,
            Self::PreTool => Some("tool_call"),
            Self::PostTool => Some("tool_result"),
            Self::PreCommit | Self::PostCommit => None,
            Self::PreTurn => Some("pre_turn"),
            Self::TurnStart => Some("turn_start"),
            Self::TurnEnd => Some("turn_end"),
            Self::PostTurn => Some("post_turn"),
            Self::ModelChange => Some("model_change"),
        }
    }

    /// All hook points (for iteration).
    pub fn all() -> &'static [HookPoint] {
        &[
            Self::PrePrompt,
            Self::PostPrompt,
            Self::SessionStart,
            Self::SessionEnd,
            Self::OnError,
            Self::PreTool,
            Self::PostTool,
            Self::PreCommit,
            Self::PostCommit,
            Self::PreTurn,
            Self::TurnStart,
            Self::TurnEnd,
            Self::PostTurn,
            Self::ModelChange,
        ]
    }
}

impl std::fmt::Display for HookPoint {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.to_filename())
    }
}

#[cfg(test)]
mod tests {
    use super::HookPoint;

    #[test]
    fn turn_hook_filenames_distinguish_gates_from_notifications() {
        assert_eq!(HookPoint::PreTurn.to_filename(), "pre-turn");
        assert_eq!(HookPoint::TurnStart.to_filename(), "turn-start");
        assert_eq!(HookPoint::TurnEnd.to_filename(), "turn-end");
        assert_eq!(HookPoint::PostTurn.to_filename(), "post-turn");
    }

    #[test]
    fn pre_turn_is_blocking_but_not_mutating() {
        assert!(HookPoint::PreTurn.is_pre_hook());
        assert!(!HookPoint::PreTurn.allows_modify());
        assert!(!HookPoint::PostTurn.is_pre_hook());
        assert!(!HookPoint::TurnStart.is_pre_hook());
        assert!(!HookPoint::TurnEnd.is_pre_hook());
    }

    #[test]
    fn prompt_and_tool_pre_hooks_keep_modify_contracts() {
        assert!(HookPoint::PrePrompt.is_pre_hook());
        assert!(HookPoint::PrePrompt.allows_modify());
        assert!(HookPoint::PreTool.allows_modify());
        assert!(!HookPoint::PostPrompt.allows_modify());
    }

    #[test]
    fn plugin_event_mapping_is_owned_by_hook_point() {
        assert_eq!(HookPoint::PreTurn.plugin_event_kind(), Some("pre_turn"));
        assert_eq!(HookPoint::TurnStart.plugin_event_kind(), Some("turn_start"));
        assert_eq!(HookPoint::TurnEnd.plugin_event_kind(), Some("turn_end"));
        assert_eq!(HookPoint::PostTurn.plugin_event_kind(), Some("post_turn"));
        assert_eq!(HookPoint::PreCommit.plugin_event_kind(), None);
        assert_eq!(HookPoint::PostCommit.plugin_event_kind(), None);
        assert_eq!(HookPoint::OnError.plugin_event_kind(), None);
    }
}
