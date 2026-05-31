//! TUI projection adapters for display-neutral configuration data.
//!
//! `clankers-config` owns serde/config schemas. This product-shell edge turns
//! those schemas into concrete TUI theme and keymap types.

use std::path::Path;

use ratatui::style::Color;
use terminal_colorsaurus::QueryOptions;
use terminal_colorsaurus::ThemeMode;

use clankers_tui::keymap::Keymap;
use clankers_tui::keymap::KeymapPreset as TuiKeymapPreset;
use clankers_tui::theme::Theme;

fn rgb(c: [u8; 3]) -> Color {
    Color::Rgb(c[0], c[1], c[2])
}

/// Project a display-neutral [`clankers_config::theme::ThemeDef`] into the
/// concrete TUI theme used by ratatui rendering.
pub fn theme_from_def(def: clankers_config::theme::ThemeDef) -> Theme {
    Theme {
        bg: rgb(def.bg),
        fg: rgb(def.fg),
        border: rgb(def.border),
        highlight: rgb(def.highlight),
        error: rgb(def.error),
        user_msg: rgb(def.user_msg),
        assistant_msg: rgb(def.assistant_msg),
        system_msg: rgb(def.system_msg),
        thinking_msg: rgb(def.thinking_msg),
        block_border: rgb(def.block_border),
        block_border_focused: rgb(def.block_border_focused),
        block_timestamp: rgb(def.block_timestamp),
        md_code_block: rgb(def.md_code_block),
        md_code_fence: rgb(def.md_code_fence),
        md_inline_code_fg: rgb(def.md_inline_code_fg),
        md_inline_code_bg: rgb(def.md_inline_code_bg),
        md_list_marker: rgb(def.md_list_marker),
        md_blockquote: rgb(def.md_blockquote),
        md_hrule: rgb(def.md_hrule),
        search_match: rgb(def.search_match),
        search_current: rgb(def.search_current),
    }
}

/// Load and project a theme by name from the themes directory.
///
/// Resolution order:
///   1. `None` or `"dark"` → built-in dark theme (no file I/O).
///   2. `"light"` → built-in light theme.
///   3. `"auto"` → detect terminal background via OSC 11, pick dark/light.
///   4. `themes_dir/<name>.ncl` (when the `nickel` feature is enabled).
///   5. `themes_dir/<name>.json`.
///   6. Falls back to dark theme with a warning on stderr.
pub fn load_theme(name: Option<&str>, themes_dir: &Path) -> Theme {
    match name {
        None | Some("dark") => Theme::dark(),
        Some("light") => Theme::light(),
        Some("auto") => detect_theme(),
        Some(name) => match clankers_config::theme::load_theme_def(name, themes_dir) {
            Ok(def) => theme_from_def(def),
            Err(msg) => {
                eprintln!("warning: {msg} — using dark theme");
                Theme::dark()
            }
        },
    }
}

/// Detect the terminal's color scheme and return the matching built-in theme.
pub fn detect_theme() -> Theme {
    match detect_theme_mode() {
        ThemeMode::Light => Theme::light(),
        ThemeMode::Dark => Theme::dark(),
    }
}

/// Raw theme-mode detection. Exposed so callers can compare before/after
/// without constructing a full Theme.
pub fn detect_theme_mode() -> ThemeMode {
    terminal_colorsaurus::theme_mode(QueryOptions::default()).unwrap_or(ThemeMode::Dark)
}

fn tui_keymap_preset(preset: clankers_config::keybindings::KeymapPreset) -> TuiKeymapPreset {
    match preset {
        clankers_config::keybindings::KeymapPreset::Helix => TuiKeymapPreset::Helix,
        clankers_config::keybindings::KeymapPreset::Vim => TuiKeymapPreset::Vim,
    }
}

/// Project data-only keymap settings into the concrete TUI keymap engine.
pub fn keymap_from_config(config: &clankers_config::keybindings::KeymapConfig) -> Keymap {
    Keymap::build(tui_keymap_preset(config.preset), &config.normal, &config.insert)
}

#[cfg(test)]
mod tests {
    use crossterm::event::KeyCode;
    use crossterm::event::KeyEvent;
    use crossterm::event::KeyModifiers;

    use super::*;

    #[test]
    fn theme_def_projects_to_tui_theme() {
        let def = clankers_config::theme::ThemeDef {
            bg: [10, 20, 30],
            user_msg: [255, 128, 0],
            ..Default::default()
        };

        let theme = theme_from_def(def);

        assert_eq!(theme.bg, Color::Rgb(10, 20, 30));
        assert_eq!(theme.user_msg, Color::Rgb(255, 128, 0));
        assert_eq!(theme.fg, Theme::dark().fg);
    }

    #[test]
    fn keymap_config_projects_to_tui_keymap() {
        let config = clankers_config::keybindings::KeymapConfig {
            preset: clankers_config::keybindings::KeymapPreset::Helix,
            normal: std::collections::HashMap::from([("x".to_string(), "quit".to_string())]),
            insert: std::collections::HashMap::new(),
        };
        let keymap = keymap_from_config(&config);
        let event = KeyEvent::new(KeyCode::Char('x'), KeyModifiers::NONE);

        assert_eq!(
            keymap.resolve(clanker_tui_types::InputMode::Normal, &event),
            Some(clanker_tui_types::Action::Core(clanker_tui_types::CoreAction::Quit))
        );
    }
}
