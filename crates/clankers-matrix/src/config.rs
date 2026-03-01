//! Matrix configuration — homeserver URL, credentials, session persistence.

use std::path::Path;
use std::path::PathBuf;

use serde::Deserialize;
use serde::Serialize;

/// Persisted Matrix configuration.
///
/// Stored at `~/.clankers/matrix.json` (global) or `.clankers/matrix.json` (project).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MatrixConfig {
    /// Homeserver base URL (e.g. `https://matrix.org`)
    pub homeserver: String,

    /// Full Matrix user ID (e.g. `@alice:matrix.org`)
    pub user_id: String,

    /// Device display name (defaults to `clankers-<short-hostname>`)
    #[serde(default = "default_device_name")]
    pub device_name: String,

    /// Access token from a previous login (for session restore)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub access_token: Option<String>,

    /// Device ID from a previous login
    #[serde(skip_serializing_if = "Option::is_none")]
    pub device_id: Option<String>,

    /// Rooms to auto-join on startup
    #[serde(default)]
    pub auto_join_rooms: Vec<String>,

    /// Whether to announce capabilities on join
    #[serde(default = "default_true")]
    pub announce_on_join: bool,

    /// Whether to respond to RPC requests from other clankers instances
    #[serde(default = "default_true")]
    pub accept_rpc: bool,

    /// Allowed user IDs for RPC (empty = allow all room members)
    #[serde(default)]
    pub rpc_allowlist: Vec<String>,

    /// Directory for Matrix SDK state/crypto store
    #[serde(skip_serializing_if = "Option::is_none")]
    pub store_path: Option<PathBuf>,
}

fn default_device_name() -> String {
    let hostname = hostname::get().ok().and_then(|h| h.into_string().ok()).unwrap_or_else(|| "unknown".to_string());
    format!("clankers-{}", hostname)
}

fn default_true() -> bool {
    true
}

impl Default for MatrixConfig {
    fn default() -> Self {
        Self {
            homeserver: "https://matrix.org".to_string(),
            user_id: String::new(),
            device_name: default_device_name(),
            access_token: None,
            device_id: None,
            auto_join_rooms: Vec::new(),
            announce_on_join: true,
            accept_rpc: true,
            rpc_allowlist: Vec::new(),
            store_path: None,
        }
    }
}

impl MatrixConfig {
    /// Load configuration from a JSON file, returning `None` if missing.
    pub fn load(path: &Path) -> Option<Self> {
        let content = std::fs::read_to_string(path).ok()?;
        serde_json::from_str(&content).ok()
    }

    /// Save configuration to a JSON file.
    pub fn save(&self, path: &Path) -> Result<(), ConfigError> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| ConfigError::Io(e.to_string()))?;
        }
        let json = serde_json::to_string_pretty(self).map_err(|e| ConfigError::Serialize(e.to_string()))?;
        std::fs::write(path, json).map_err(|e| ConfigError::Io(e.to_string()))?;
        Ok(())
    }

    /// Resolve the Matrix SDK store path (for crypto/state persistence).
    pub fn resolve_store_path(&self, clankers_config_dir: &Path) -> PathBuf {
        self.store_path.clone().unwrap_or_else(|| clankers_config_dir.join("matrix_store"))
    }

    /// Check if we have a saved session that can be restored.
    pub fn has_session(&self) -> bool {
        self.access_token.is_some() && self.device_id.is_some()
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("IO error: {0}")]
    Io(String),
    #[error("Serialization error: {0}")]
    Serialize(String),
}
