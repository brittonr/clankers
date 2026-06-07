//! Modal keymap configuration data.
//!
//! The keymap engine (runtime key event parsing, defaults, and action
//! resolution) lives in the TUI crate. This module owns only the serializable
//! settings-layer shape loaded from configuration files.

use std::collections::HashMap;
use std::fmt;
use std::path::Path;

use serde::Deserialize;
use serde::Serialize;

/// Which keymap preset to select before TUI projection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum KeymapPreset {
    #[default]
    Helix,
    Vim,
}

impl fmt::Display for KeymapPreset {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Helix => write!(f, "helix"),
            Self::Vim => write!(f, "vim"),
        }
    }
}

// ---------------------------------------------------------------------------
// Serialisable config (loaded from settings file)
// ---------------------------------------------------------------------------

/// User-facing keymap configuration (stored in settings.json).
///
/// ```json
/// {
///   "keymap": {
///     "preset": "helix",
///     "normal": { "x": "quit" },
///     "insert": { "Ctrl+K": "delete_word" }
///   }
/// }
/// ```
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KeymapConfig {
    /// Which preset to start from: "helix" (default) or "vim".
    #[serde(default)]
    pub preset: KeymapPreset,

    /// Per-key overrides for normal mode.
    #[serde(default)]
    pub normal: HashMap<String, String>,

    /// Per-key overrides for insert mode.
    #[serde(default)]
    pub insert: HashMap<String, String>,
}

impl KeymapConfig {
    pub fn load(path: &Path) -> Self {
        std::fs::read_to_string(path).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_keymap_config_is_data_only_helix() {
        let config = KeymapConfig::default();

        assert_eq!(config.preset, KeymapPreset::Helix);
        assert!(config.normal.is_empty());
        assert!(config.insert.is_empty());
    }

    #[test]
    fn keymap_config_deserializes_without_tui_keymap_engine() {
        let config: KeymapConfig = serde_json::from_str(
            r#"{
                "preset": "vim",
                "normal": { "x": "quit" },
                "insert": { "Ctrl+K": "delete_word" }
            }"#,
        )
        .unwrap();

        assert_eq!(config.preset, KeymapPreset::Vim);
        assert_eq!(config.normal.get("x").map(String::as_str), Some("quit"));
        assert_eq!(config.insert.get("Ctrl+K").map(String::as_str), Some("delete_word"));
    }

    #[test]
    fn keymap_preset_display_matches_settings_surface() {
        assert_eq!(KeymapPreset::Helix.to_string(), "helix");
        assert_eq!(KeymapPreset::Vim.to_string(), "vim");
    }
}
