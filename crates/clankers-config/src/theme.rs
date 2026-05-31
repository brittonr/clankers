//! Theme configuration data loaded from `.ncl` / `.json` files.
//!
//! Theme files live in `~/.clankers/agent/themes/<name>.ncl` (or `.json`).
//! The Nickel contract at `theme-contract.ncl` provides type checking and
//! defaults — users only need to override the colors they care about.
//!
//! This module intentionally stays display-neutral. It loads and validates RGB
//! arrays; TUI projection into terminal colors lives in the product shell/TUI
//! adapter.
//!
//! # Example theme file (`~/.clankers/agent/themes/monokai.ncl`)
//!
//! ```nickel
//! (import "clankers://theme") & {
//!   bg = [39, 40, 34],
//!   fg = [248, 248, 242],
//!   highlight = [166, 226, 46],
//!   userMsg = [166, 226, 46],
//!   error = [249, 38, 114],
//! }
//! ```

use std::path::Path;

use serde::Deserialize;
use serde::Serialize;

// ── Serializable theme definition ───────────────────────────────────────────

/// Intermediate theme representation that maps 1:1 to the Nickel contract.
///
/// Every field defaults to the dark-theme value so that partial theme
/// files (both `.ncl` and `.json`) work without specifying every color.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ThemeDef {
    // ── Base colors ──────────────────────────────────
    #[serde(default = "d_bg")]
    pub bg: [u8; 3],
    #[serde(default = "d_fg")]
    pub fg: [u8; 3],
    #[serde(default = "d_border")]
    pub border: [u8; 3],
    #[serde(default = "d_highlight")]
    pub highlight: [u8; 3],
    #[serde(default = "d_error")]
    pub error: [u8; 3],

    // ── Message role colors ──────────────────────────
    #[serde(default = "d_user_msg")]
    pub user_msg: [u8; 3],
    #[serde(default = "d_assistant_msg")]
    pub assistant_msg: [u8; 3],
    #[serde(default = "d_system_msg")]
    pub system_msg: [u8; 3],
    #[serde(default = "d_thinking_msg")]
    pub thinking_msg: [u8; 3],

    // ── Block chrome ─────────────────────────────────
    #[serde(default = "d_block_border")]
    pub block_border: [u8; 3],
    #[serde(default = "d_block_border_focused")]
    pub block_border_focused: [u8; 3],
    #[serde(default = "d_block_timestamp")]
    pub block_timestamp: [u8; 3],

    // ── Markdown rendering ───────────────────────────
    #[serde(default = "d_md_code_block")]
    pub md_code_block: [u8; 3],
    #[serde(default = "d_md_code_fence")]
    pub md_code_fence: [u8; 3],
    #[serde(default = "d_md_inline_code_fg")]
    pub md_inline_code_fg: [u8; 3],
    #[serde(default = "d_md_inline_code_bg")]
    pub md_inline_code_bg: [u8; 3],
    #[serde(default = "d_md_list_marker")]
    pub md_list_marker: [u8; 3],
    #[serde(default = "d_md_blockquote")]
    pub md_blockquote: [u8; 3],
    #[serde(default = "d_md_hrule")]
    pub md_hrule: [u8; 3],

    // ── Search highlights ────────────────────────────
    #[serde(default = "d_search_match")]
    pub search_match: [u8; 3],
    #[serde(default = "d_search_current")]
    pub search_current: [u8; 3],
}

// ── Defaults (dark theme) ───────────────────────────────────────────────────

fn d_bg() -> [u8; 3] {
    [30, 30, 30]
}
fn d_fg() -> [u8; 3] {
    [220, 220, 220]
}
fn d_border() -> [u8; 3] {
    [80, 80, 80]
}
fn d_highlight() -> [u8; 3] {
    [100, 180, 255]
}
fn d_error() -> [u8; 3] {
    [255, 100, 100]
}
fn d_user_msg() -> [u8; 3] {
    [120, 200, 120]
}
fn d_assistant_msg() -> [u8; 3] {
    [200, 200, 200]
}
fn d_system_msg() -> [u8; 3] {
    [150, 150, 150]
}
fn d_thinking_msg() -> [u8; 3] {
    [150, 130, 200]
}
fn d_block_border() -> [u8; 3] {
    [60, 60, 60]
}
fn d_block_border_focused() -> [u8; 3] {
    [100, 180, 255]
}
fn d_block_timestamp() -> [u8; 3] {
    [100, 100, 100]
}
fn d_md_code_block() -> [u8; 3] {
    [180, 220, 140]
}
fn d_md_code_fence() -> [u8; 3] {
    [100, 100, 100]
}
fn d_md_inline_code_fg() -> [u8; 3] {
    [230, 190, 80]
}
fn d_md_inline_code_bg() -> [u8; 3] {
    [45, 45, 45]
}
fn d_md_list_marker() -> [u8; 3] {
    [100, 100, 100]
}
fn d_md_blockquote() -> [u8; 3] {
    [160, 160, 160]
}
fn d_md_hrule() -> [u8; 3] {
    [80, 80, 80]
}
fn d_search_match() -> [u8; 3] {
    [120, 90, 30]
}
fn d_search_current() -> [u8; 3] {
    [220, 180, 40]
}

impl Default for ThemeDef {
    fn default() -> Self {
        Self {
            bg: d_bg(),
            fg: d_fg(),
            border: d_border(),
            highlight: d_highlight(),
            error: d_error(),
            user_msg: d_user_msg(),
            assistant_msg: d_assistant_msg(),
            system_msg: d_system_msg(),
            thinking_msg: d_thinking_msg(),
            block_border: d_block_border(),
            block_border_focused: d_block_border_focused(),
            block_timestamp: d_block_timestamp(),
            md_code_block: d_md_code_block(),
            md_code_fence: d_md_code_fence(),
            md_inline_code_fg: d_md_inline_code_fg(),
            md_inline_code_bg: d_md_inline_code_bg(),
            md_list_marker: d_md_list_marker(),
            md_blockquote: d_md_blockquote(),
            md_hrule: d_md_hrule(),
            search_match: d_search_match(),
            search_current: d_search_current(),
        }
    }
}

// ── Loading ─────────────────────────────────────────────────────────────────

/// Returns `true` when the theme name enables auto-detection.
pub fn is_auto_theme(name: Option<&str>) -> bool {
    matches!(name, Some("auto"))
}

/// Load a custom theme definition by name from the themes directory.
///
/// Built-in names (`dark`, `light`, `auto`) are selected and projected by the
/// display edge; this function only loads user theme files.
pub fn load_theme_def(name: &str, themes_dir: &Path) -> Result<ThemeDef, String> {
    load_by_name(name, themes_dir)
}

fn load_by_name(name: &str, themes_dir: &Path) -> Result<ThemeDef, String> {
    // Try .ncl first (nickel feature)
    #[cfg(feature = "nickel")]
    {
        let ncl_path = themes_dir.join(format!("{name}.ncl"));
        if ncl_path.exists() {
            return load_ncl(&ncl_path);
        }
    }

    // Try .json
    let json_path = themes_dir.join(format!("{name}.json"));
    if json_path.exists() {
        return load_json(&json_path);
    }

    Err(format!("theme '{name}' not found in {}", themes_dir.display()))
}

#[cfg(feature = "nickel")]
fn load_ncl(path: &Path) -> Result<ThemeDef, String> {
    let value =
        crate::nickel::eval_ncl_file(path).map_err(|e| format!("failed to evaluate {}: {e}", path.display()))?;
    serde_json::from_value(value).map_err(|e| format!("invalid theme in {}: {e}", path.display()))
}

fn load_json(path: &Path) -> Result<ThemeDef, String> {
    let content = std::fs::read_to_string(path).map_err(|e| format!("failed to read {}: {e}", path.display()))?;
    serde_json::from_str(&content).map_err(|e| format!("invalid theme in {}: {e}", path.display()))
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_theme_def_matches_documented_dark_rgb_values() {
        let def = ThemeDef::default();

        assert_eq!(def.bg, [30, 30, 30]);
        assert_eq!(def.fg, [220, 220, 220]);
        assert_eq!(def.highlight, [100, 180, 255]);
        assert_eq!(def.error, [255, 100, 100]);
        assert_eq!(def.user_msg, [120, 200, 120]);
        assert_eq!(def.md_code_block, [180, 220, 140]);
        assert_eq!(def.search_current, [220, 180, 40]);
    }

    #[test]
    fn partial_json_fills_defaults() {
        let json = r#"{ "bg": [0, 0, 0], "error": [255, 0, 0] }"#;
        let def: ThemeDef = serde_json::from_str(json).unwrap();

        assert_eq!(def.bg, [0, 0, 0]);
        assert_eq!(def.error, [255, 0, 0]);
        // Unset fields get dark-theme defaults
        assert_eq!(def.fg, [220, 220, 220]);
        assert_eq!(def.highlight, [100, 180, 255]);
    }

    #[test]
    fn load_nonexistent_theme_returns_error() {
        let dir = std::path::PathBuf::from("/tmp/clankers-test-no-themes");
        let error = load_theme_def("nonexistent", &dir).unwrap_err();

        assert!(error.contains("theme 'nonexistent' not found"));
    }

    #[test]
    fn auto_theme_detection_is_data_only_name_check() {
        assert!(is_auto_theme(Some("auto")));
        assert!(!is_auto_theme(None));
        assert!(!is_auto_theme(Some("dark")));
    }

    #[test]
    fn roundtrip_json() {
        let def = ThemeDef::default();
        let json = serde_json::to_string(&def).unwrap();
        let parsed: ThemeDef = serde_json::from_str(&json).unwrap();
        assert_eq!(def, parsed);
    }

    #[test]
    fn load_json_theme_from_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.json");
        std::fs::write(&path, r#"{ "bg": [10, 20, 30], "userMsg": [255, 128, 0] }"#).unwrap();

        let def = load_theme_def("test", dir.path()).unwrap();
        assert_eq!(def.bg, [10, 20, 30]);
        assert_eq!(def.user_msg, [255, 128, 0]);
        // Defaults for unset fields
        assert_eq!(def.fg, [220, 220, 220]);
    }

    #[cfg(feature = "nickel")]
    #[test]
    fn load_ncl_theme_from_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("custom.ncl");
        std::fs::write(&path, r#"(import "clankers://theme") & { bg = [5, 10, 15] }"#).unwrap();

        let def = load_theme_def("custom", dir.path()).unwrap();
        assert_eq!(def.bg, [5, 10, 15]);
        // Contract default for unset field
        assert_eq!(def.fg, [220, 220, 220]);
    }
}
