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

    #[snafu(display("tool timeout ({tool_name}): exceeded {timeout_secs}s"))]
    ToolTimeout { tool_name: String, timeout_secs: u64 },

    #[snafu(display("agent error: {message}"))]
    Agent { message: String },

    #[snafu(display("agent loop error: {message}"))]
    AgentLoop { message: String },

    #[snafu(display("agent context error: {message}"))]
    AgentContext { message: String },

    #[snafu(display("agent definition error ({path}): {message}", path = path.display()))]
    AgentDefinition { path: std::path::PathBuf, message: String },

    #[snafu(display("worktree error: {message}"))]
    Worktree { message: String },

    #[snafu(display("worktree git error: {message}"))]
    WorktreeGit { message: String, source: std::io::Error },

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

pub type Result<T, E = Error> = std::result::Result<T, E>;
