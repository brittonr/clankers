use snafu::prelude::*;

#[derive(Debug, Snafu)]
#[snafu(visibility(pub))]
pub enum Error {
    #[snafu(display("configuration error: {message}"))]
    Config { message: String },

    #[snafu(display("I/O error: {source}"))]
    Io { source: std::io::Error },

    #[snafu(display("JSON error: {source}"))]
    Json { source: serde_json::Error },

    #[snafu(display("provider error: {message}"))]
    Provider { message: String },

    #[snafu(display("provider authentication error: {message}"))]
    ProviderAuth { message: String },

    #[snafu(display("provider streaming error: {message}"))]
    ProviderStreaming { message: String },

    #[snafu(display("session error: {message}"))]
    Session { message: String },

    #[snafu(display("session store error: {message}"))]
    SessionStore { message: String, source: std::io::Error },

    #[snafu(display("tool error ({tool_name}): {message}"))]
    Tool { tool_name: String, message: String },

    #[snafu(display("tool execution error ({tool_name}): {source}"))]
    ToolExecution { tool_name: String, source: std::io::Error },

    #[snafu(display("agent error: {message}"))]
    Agent { message: String },

    #[snafu(display("agent context error: {message}"))]
    AgentContext { message: String },

    #[snafu(display("worktree error: {message}"))]
    Worktree { message: String },

    #[snafu(display("plugin error ({plugin_name}): {message}"))]
    Plugin { plugin_name: String, message: String },

    #[snafu(display("skill error: {message}"))]
    Skill { message: String },

    #[snafu(display("spec error: {message}"))]
    Spec { message: String },

    #[snafu(display("TUI error: {message}"))]
    Tui { message: String },

    #[snafu(display("zellij error: {message}"))]
    Zellij { message: String },

    #[snafu(display("database error: {message}"))]
    Database { message: String },

    #[snafu(display("operation cancelled"))]
    Cancelled,
}

impl From<clankers_router::Error> for Error {
    fn from(e: clankers_router::Error) -> Self {
        match e {
            clankers_router::Error::Auth { message } => Error::ProviderAuth { message },
            clankers_router::Error::Provider { message, .. } => Error::Provider { message },
            clankers_router::Error::Streaming { message } => Error::ProviderStreaming { message },
            clankers_router::Error::Io(source) => Error::Io { source },
            clankers_router::Error::Json(source) => Error::Json { source },
            other => Error::Provider {
                message: other.to_string(),
            },
        }
    }
}

impl From<clankers_db::DbError> for Error {
    fn from(e: clankers_db::DbError) -> Self {
        Error::Database { message: e.message }
    }
}

impl From<clankers_session::error::SessionError> for Error {
    fn from(e: clankers_session::error::SessionError) -> Self {
        Error::Session { message: e.message }
    }
}

#[cfg(feature = "zellij-share")]
impl From<clankers_zellij::ZellijError> for Error {
    fn from(e: clankers_zellij::ZellijError) -> Self {
        Error::Zellij { message: e.message }
    }
}

pub type Result<T, E = Error> = std::result::Result<T, E>;

// ── Error codes ─────────────────────────────────────────────────────

/// Stable, machine-readable error classification.
///
/// Every [`Error`] variant maps to an `ErrorCode`. Subagents and daemon
/// consumers match on these instead of parsing error strings. Codes are
/// intentionally coarser than `Error` variants — multiple variants can
/// share a code when the appropriate response is the same.
///
/// # Stability
///
/// Codes and their `as_str()` values are part of the public API.
/// Don't rename or remove them without a migration period.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ErrorCode {
    /// Bad or missing configuration.
    Config,
    /// File system I/O failure.
    Io,
    /// JSON parse or serialization failure.
    Json,
    /// Provider returned an error (model error, bad request, etc.).
    Provider,
    /// Authentication or credential failure.
    ProviderAuth,
    /// Streaming connection dropped or failed.
    ProviderStreaming,
    /// Session not found or corrupt.
    Session,
    /// Tool returned an error result.
    ToolFailed,
    /// Tool exceeded its timeout.
    ToolTimeout,
    /// Agent loop or orchestration error.
    Agent,
    /// Agent definition file invalid or missing.
    AgentDefinition,
    /// Worktree or git operation failed.
    Worktree,
    /// Plugin load, execution, or sandbox error.
    Plugin,
    /// Skill discovery or loading error.
    Skill,
    /// Spec validation or merge error.
    Spec,
    /// Terminal UI error.
    Tui,
    /// Zellij IPC or layout error.
    Zellij,
    /// Database open, read, or write error.
    Database,
    /// User or signal cancelled the operation.
    Cancelled,
}

impl ErrorCode {
    /// Stable string identifier for serialization and matching.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Config => "config",
            Self::Io => "io",
            Self::Json => "json",
            Self::Provider => "provider",
            Self::ProviderAuth => "provider_auth",
            Self::ProviderStreaming => "provider_streaming",
            Self::Session => "session",
            Self::ToolFailed => "tool_failed",
            Self::ToolTimeout => "tool_timeout",
            Self::Agent => "agent",
            Self::AgentDefinition => "agent_definition",
            Self::Worktree => "worktree",
            Self::Plugin => "plugin",
            Self::Skill => "skill",
            Self::Spec => "spec",
            Self::Tui => "tui",
            Self::Zellij => "zellij",
            Self::Database => "database",
            Self::Cancelled => "cancelled",
        }
    }

    /// Process exit code for this error class.
    ///
    /// Groups related errors into a small set of exit codes so scripts
    /// can branch without matching every string:
    ///
    /// - `1` — general / internal error
    /// - `2` — configuration or usage error
    /// - `3` — authentication / credential error
    /// - `4` — I/O or database error
    /// - `5` — provider / network error
    /// - `6` — timeout
    /// - `7` — cancelled by user
    pub const fn exit_code(self) -> i32 {
        match self {
            Self::Config | Self::AgentDefinition | Self::Skill => 2,
            Self::ProviderAuth => 3,
            Self::Io | Self::Json | Self::Database | Self::Session => 4,
            Self::Provider | Self::ProviderStreaming => 5,
            Self::ToolTimeout => 6,
            Self::Cancelled => 7,
            Self::Agent | Self::ToolFailed | Self::Plugin | Self::Spec | Self::Tui | Self::Zellij | Self::Worktree => 1,
        }
    }

    /// Whether this error is likely transient and retrying may succeed.
    ///
    /// Subagents use this to decide whether to retry a failed delegation
    /// or report the error immediately.
    pub const fn is_retryable(self) -> bool {
        matches!(self, Self::Provider | Self::ProviderStreaming | Self::ToolTimeout | Self::Io)
    }

    /// Whether a human can fix this without code changes.
    pub const fn is_user_fixable(self) -> bool {
        matches!(self, Self::Config | Self::ProviderAuth | Self::AgentDefinition | Self::Skill)
    }
}

impl std::fmt::Display for ErrorCode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl Error {
    /// Classify this error into a stable [`ErrorCode`].
    pub const fn code(&self) -> ErrorCode {
        match self {
            Self::Config { .. } => ErrorCode::Config,
            Self::Io { .. } => ErrorCode::Io,
            Self::Json { .. } => ErrorCode::Json,
            Self::Provider { .. } => ErrorCode::Provider,
            Self::ProviderAuth { .. } => ErrorCode::ProviderAuth,
            Self::ProviderStreaming { .. } => ErrorCode::ProviderStreaming,
            Self::Session { .. } | Self::SessionStore { .. } => ErrorCode::Session,
            Self::Tool { .. } | Self::ToolExecution { .. } => ErrorCode::ToolFailed,
            Self::Agent { .. } | Self::AgentContext { .. } => ErrorCode::Agent,
            Self::Worktree { .. } => ErrorCode::Worktree,
            Self::Plugin { .. } => ErrorCode::Plugin,
            Self::Skill { .. } => ErrorCode::Skill,
            Self::Spec { .. } => ErrorCode::Spec,
            Self::Tui { .. } => ErrorCode::Tui,
            Self::Zellij { .. } => ErrorCode::Zellij,
            Self::Database { .. } => ErrorCode::Database,
            Self::Cancelled => ErrorCode::Cancelled,
        }
    }

    /// Human-readable recovery suggestion, if one exists.
    ///
    /// Returns `None` for errors where there's no obvious user action.
    pub fn suggestion(&self) -> Option<&'static str> {
        match self {
            Self::Config { .. } => Some("check ~/.config/clankers/config.toml"),
            Self::ProviderAuth { .. } => Some("check your API key or run `clankers auth login`"),
            Self::Provider { .. } => Some("try a different model or check provider status"),
            Self::ProviderStreaming { .. } => Some("retry — the connection may have dropped"),
            Self::Session { .. } | Self::SessionStore { .. } => {
                Some("try `clankers session list` to see available sessions")
            }
            Self::Skill { .. } => Some("check the SKILL.md file exists and is valid"),
            Self::Database { .. } => Some("the database may be corrupt — try removing ~/.clankers/agent/clankers.db"),
            Self::Plugin { .. } => Some("rebuild the plugin with `cargo build --target wasm32-unknown-unknown`"),
            Self::Cancelled => Some("operation was cancelled — rerun to try again"),
            _ => None,
        }
    }

    /// Process exit code derived from the error code.
    pub const fn exit_code(&self) -> i32 {
        self.code().exit_code()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_variant_has_a_code() {
        // Construct one of each variant and verify code() doesn't panic.
        let cases: Vec<Error> = vec![
            Error::Config { message: String::new() },
            Error::Io {
                source: std::io::Error::other("test"),
            },
            Error::Json {
                source: serde_json::from_str::<()>("bad").unwrap_err(),
            },
            Error::Provider { message: String::new() },
            Error::ProviderAuth { message: String::new() },
            Error::ProviderStreaming { message: String::new() },
            Error::Session { message: String::new() },
            Error::SessionStore {
                message: String::new(),
                source: std::io::Error::other("test"),
            },
            Error::Tool {
                tool_name: String::new(),
                message: String::new(),
            },
            Error::ToolExecution {
                tool_name: String::new(),
                source: std::io::Error::other("test"),
            },
            Error::Agent { message: String::new() },
            Error::AgentContext { message: String::new() },
            Error::Worktree { message: String::new() },
            Error::Plugin {
                plugin_name: String::new(),
                message: String::new(),
            },
            Error::Skill { message: String::new() },
            Error::Spec { message: String::new() },
            Error::Tui { message: String::new() },
            Error::Zellij { message: String::new() },
            Error::Database { message: String::new() },
            Error::Cancelled,
        ];

        for err in &cases {
            let code = err.code();
            // as_str is non-empty
            assert!(!code.as_str().is_empty(), "empty code string for {:?}", code);
            // exit_code is valid
            assert!(err.exit_code() >= 1 && err.exit_code() <= 7, "bad exit code for {:?}", code);
        }
    }

    #[test]
    fn code_as_str_roundtrip() {
        use ErrorCode::*;
        let all = [
            Config,
            Io,
            Json,
            Provider,
            ProviderAuth,
            ProviderStreaming,
            Session,
            ToolFailed,
            ToolTimeout,
            Agent,
            AgentDefinition,
            Worktree,
            Plugin,
            Skill,
            Spec,
            Tui,
            Zellij,
            Database,
            Cancelled,
        ];
        // All strings are unique.
        let mut seen = std::collections::HashSet::new();
        for code in all {
            assert!(seen.insert(code.as_str()), "duplicate code string: {}", code.as_str());
        }
    }

    #[test]
    fn retryable_codes() {
        assert!(ErrorCode::Provider.is_retryable());
        assert!(ErrorCode::ProviderStreaming.is_retryable());
        assert!(ErrorCode::ToolTimeout.is_retryable());
        assert!(ErrorCode::Io.is_retryable());

        assert!(!ErrorCode::ProviderAuth.is_retryable());
        assert!(!ErrorCode::Config.is_retryable());
        assert!(!ErrorCode::Cancelled.is_retryable());
        assert!(!ErrorCode::Database.is_retryable());
    }

    #[test]
    fn user_fixable_codes() {
        assert!(ErrorCode::Config.is_user_fixable());
        assert!(ErrorCode::ProviderAuth.is_user_fixable());
        assert!(ErrorCode::AgentDefinition.is_user_fixable());
        assert!(ErrorCode::Skill.is_user_fixable());

        assert!(!ErrorCode::Agent.is_user_fixable());
        assert!(!ErrorCode::Provider.is_user_fixable());
        assert!(!ErrorCode::Database.is_user_fixable());
    }

    #[test]
    fn exit_codes_are_grouped() {
        // Config-class errors share exit code 2.
        assert_eq!(ErrorCode::Config.exit_code(), 2);
        assert_eq!(ErrorCode::AgentDefinition.exit_code(), 2);
        assert_eq!(ErrorCode::Skill.exit_code(), 2);

        // Auth is 3.
        assert_eq!(ErrorCode::ProviderAuth.exit_code(), 3);

        // I/O class is 4.
        assert_eq!(ErrorCode::Io.exit_code(), 4);
        assert_eq!(ErrorCode::Database.exit_code(), 4);
        assert_eq!(ErrorCode::Session.exit_code(), 4);

        // Provider/network is 5.
        assert_eq!(ErrorCode::Provider.exit_code(), 5);
        assert_eq!(ErrorCode::ProviderStreaming.exit_code(), 5);

        // Timeout is 6.
        assert_eq!(ErrorCode::ToolTimeout.exit_code(), 6);

        // Cancelled is 7.
        assert_eq!(ErrorCode::Cancelled.exit_code(), 7);
    }

    #[test]
    fn suggestions_exist_for_user_facing_errors() {
        let auth_err = Error::ProviderAuth {
            message: "bad token".into(),
        };
        assert!(auth_err.suggestion().is_some());
        assert!(auth_err.suggestion().unwrap().contains("API key"));

        let config_err = Error::Config {
            message: "missing field".into(),
        };
        assert!(config_err.suggestion().is_some());

        let db_err = Error::Database {
            message: "corrupt".into(),
        };
        assert!(db_err.suggestion().is_some());
    }

    #[test]
    fn suggestion_is_none_for_internal_errors() {
        let err = Error::Agent {
            message: "internal".into(),
        };
        assert!(err.suggestion().is_none());

        let err = Error::Io {
            source: std::io::Error::other("test"),
        };
        assert!(err.suggestion().is_none());
    }

    #[test]
    fn error_code_display() {
        assert_eq!(format!("{}", ErrorCode::ProviderAuth), "provider_auth");
        assert_eq!(format!("{}", ErrorCode::ToolTimeout), "tool_timeout");
        assert_eq!(format!("{}", ErrorCode::Cancelled), "cancelled");
    }

    #[test]
    fn session_variants_share_code() {
        let err1 = Error::Session { message: String::new() };
        let err2 = Error::SessionStore {
            message: String::new(),
            source: std::io::Error::other("test"),
        };
        assert_eq!(err1.code(), err2.code());
        assert_eq!(err1.code(), ErrorCode::Session);
    }

    #[test]
    fn tool_variants_share_code() {
        let err1 = Error::Tool {
            tool_name: "bash".into(),
            message: String::new(),
        };
        let err2 = Error::ToolExecution {
            tool_name: "bash".into(),
            source: std::io::Error::other("test"),
        };
        assert_eq!(err1.code(), err2.code());
        assert_eq!(err1.code(), ErrorCode::ToolFailed);
    }
}
