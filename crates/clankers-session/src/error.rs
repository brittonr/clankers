//! Session error types.

/// Session error.
#[derive(Debug, Clone)]
pub struct SessionError {
    pub message: String,
}

impl std::fmt::Display for SessionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "session: {}", self.message)
    }
}

impl std::error::Error for SessionError {}

/// Convenience alias.
pub type Result<T> = std::result::Result<T, SessionError>;

/// Convert any displayable error into a `SessionError`.
pub fn session_err(e: impl std::fmt::Display) -> SessionError {
    SessionError { message: e.to_string() }
}
