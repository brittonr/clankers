//! Modal keymap configuration — thin wrapper around `clankers_tui::keymap`.
//!
//! The keymap engine (Keymap, KeyCombo, presets, defaults) lives in the TUI
//! crate. This module re-exports those types and provides the settings-layer
//! `KeymapConfig` for loading from the config file.

// Re-export everything from the TUI keymap module.
use std::collections::HashMap;
use std::path::Path;

pub use clankers_tui::keymap::*;
// Re-export action types (canonical home is clanker-tui-types).
pub use clanker_tui_types::Action;
pub use clanker_tui_types::ActionRegistry;
pub use clanker_tui_types::CoreAction;
pub use clanker_tui_types::ExtendedAction;
pub use clanker_tui_types::ExtendedActionDef;
pub use clanker_tui_types::InputMode;
pub use clanker_tui_types::parse_action;
use serde::Deserialize;
use serde::Serialize;

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
    /// Which preset to start from: "helix" (default) or "vim"
    #[serde(default)]
    pub preset: KeymapPreset,

    /// Per-key overrides for normal mode
    #[serde(default)]
    pub normal: HashMap<String, String>,

    /// Per-key overrides for insert mode
    #[serde(default)]
    pub insert: HashMap<String, String>,
}

impl KeymapConfig {
    pub fn load(path: &Path) -> Self {
        std::fs::read_to_string(path).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default()
    }

    pub fn into_keymap(self) -> Keymap {
        Keymap::build(self.preset, &self.normal, &self.insert)
    }
}
