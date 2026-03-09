//! Database error types.

/// Database error.
#[derive(Debug, Clone)]
pub struct DbError {
    pub message: String,
}

impl std::fmt::Display for DbError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "database: {}", self.message)
    }
}

impl std::error::Error for DbError {}

/// Convenience alias.
pub type Result<T> = std::result::Result<T, DbError>;

/// Convert any displayable error into a `DbError`.
pub fn db_err(e: impl std::fmt::Display) -> DbError {
    DbError { message: e.to_string() }
}
