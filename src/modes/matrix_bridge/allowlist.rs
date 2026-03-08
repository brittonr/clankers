//! User allowlist resolution and validation.

/// Resolve the Matrix user allowlist from (in priority order):
/// 1. `CLANKERS_MATRIX_ALLOWED_USERS` env var (comma-separated)
/// 2. `allowed_users` from `matrix.json`
/// 3. `matrix_allowed_users` from `DaemonConfig`
///
/// Empty = allow all.
pub(crate) fn resolve_matrix_allowlist(
    matrix_config: &clankers_matrix::MatrixConfig,
    daemon_allowed: &[String],
) -> Vec<String> {
    if let Ok(env_val) = std::env::var("CLANKERS_MATRIX_ALLOWED_USERS") {
        let users: Vec<String> = env_val.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect();
        if !users.is_empty() {
            return users;
        }
    }
    if !matrix_config.allowed_users.is_empty() {
        return matrix_config.allowed_users.clone();
    }
    daemon_allowed.to_vec()
}

/// Check if a user is in the allowlist (empty = allow all).
pub(crate) fn is_user_allowed(allowlist: &[String], user_id: &str) -> bool {
    allowlist.is_empty() || allowlist.iter().any(|u| u == user_id)
}
