//! Helix-style leader key (Space) popup menu.
//!
//! Pressing Space in normal mode opens a which-key overlay showing available
//! actions. Single-key press executes an action or opens a submenu.
//! Escape or any unrecognized key dismisses the menu.
//!
//! The menu is built dynamically from [`MenuContributor`] implementations,
//! allowing builtins, plugins, and user config to contribute items.

use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use crossterm::event::KeyModifiers;

pub mod builder;
pub mod render;
pub mod types;

// Re-export public types
// Re-export public builder items
pub use builder::BuiltinKeymapContributor;
pub use builder::SlashCommandContributor;
pub use types::BuildResult;
pub use types::HiddenSet;
pub use types::LeaderAction;
pub use types::LeaderMenuDef;
pub use types::LeaderMenuItem;
pub use types::MenuContribution;
pub use types::MenuContributor;
pub use types::MenuPlacement;

// ---------------------------------------------------------------------------
// Runtime state
// ---------------------------------------------------------------------------

/// Tracks the leader menu's open/navigation state.
pub struct LeaderMenu {
    /// Whether the leader menu overlay is visible.
    pub visible: bool,
    /// Stack of menu definitions (root at bottom, current at top).
    stack: Vec<LeaderMenuDef>,
    /// Breadcrumb labels for the title bar.
    breadcrumb: Vec<String>,
    /// All defined submenus, keyed by name.
    submenus: Vec<LeaderMenuDef>,
    /// The root menu definition.
    root: LeaderMenuDef,
}

impl Default for LeaderMenu {
    fn default() -> Self {
        Self::new()
    }
}

impl LeaderMenu {
    /// Create a leader menu with the default hardcoded bindings (no slash commands).
    ///
    /// Prefer [`LeaderMenu::build`] for dynamic registration with slash commands.
    pub fn new() -> Self {
        use std::collections::HashSet;
        Self::build(&[&BuiltinKeymapContributor], &HashSet::new()).0
    }

    /// Build a leader menu from contributors.
    ///
    /// Collects all [`MenuContribution`] items, deduplicates by `(key, placement)`
    /// with highest priority winning, removes hidden entries, and assembles the
    /// menu tree.
    pub fn build(contributors: &[&dyn MenuContributor], hidden: &HiddenSet) -> BuildResult {
        builder::build(contributors, hidden)
    }

    /// Get the root menu definition.
    pub fn root_def(&self) -> &LeaderMenuDef {
        &self.root
    }

    /// Get all submenu definitions.
    pub fn submenu_defs(&self) -> &[LeaderMenuDef] {
        &self.submenus
    }

    /// Open the leader menu (shows root level).
    pub fn open(&mut self) {
        self.visible = true;
        self.stack.clear();
        self.breadcrumb.clear();
        self.stack.push(self.root.clone());
    }

    /// Close the leader menu entirely.
    pub fn close(&mut self) {
        self.visible = false;
        self.stack.clear();
        self.breadcrumb.clear();
    }

    /// The currently displayed menu.
    fn current(&self) -> Option<&LeaderMenuDef> {
        self.stack.last()
    }

    /// Handle a key press while the leader menu is visible.
    ///
    /// Returns `Some(action)` if an action should be executed,
    /// `None` if the key was consumed internally (submenu nav, close).
    pub fn handle_key(&mut self, key: &KeyEvent) -> Option<LeaderAction> {
        if !self.visible {
            return None;
        }

        // Escape → go back one level, or close
        if key.code == KeyCode::Esc {
            if self.stack.len() > 1 {
                self.stack.pop();
                self.breadcrumb.pop();
            } else {
                self.close();
            }
            return None;
        }

        // Match single character keys
        let ch = match key.code {
            KeyCode::Char(c) if key.modifiers.is_empty() || key.modifiers == KeyModifiers::SHIFT => c,
            _ => {
                // Unknown non-char key → dismiss
                self.close();
                return None;
            }
        };

        let current = match self.current() {
            Some(m) => m,
            None => {
                self.close();
                return None;
            }
        };

        // Find matching item
        if let Some(item) = current.items.iter().find(|i| i.key == ch) {
            match &item.action {
                LeaderAction::Submenu(name) => {
                    // Push the submenu onto the stack
                    if let Some(sub) = self.submenus.iter().find(|s| s.label == *name) {
                        self.breadcrumb.push(item.label.clone());
                        self.stack.push(sub.clone());
                    } else {
                        self.close();
                    }
                    None
                }
                action => {
                    let result = action.clone();
                    self.close();
                    Some(result)
                }
            }
        } else {
            // Unknown key → dismiss (Helix behavior)
            self.close();
            None
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use clankers_tui_types::PRIORITY_BUILTIN;
    use clankers_tui_types::PRIORITY_PLUGIN;
    use clankers_tui_types::PRIORITY_USER;
    use crossterm::event::KeyEventKind;

    use super::*;

    fn key(c: char) -> KeyEvent {
        KeyEvent::new_with_kind(KeyCode::Char(c), KeyModifiers::NONE, KeyEventKind::Press)
    }

    fn shift_key(c: char) -> KeyEvent {
        KeyEvent::new_with_kind(KeyCode::Char(c), KeyModifiers::SHIFT, KeyEventKind::Press)
    }

    fn esc() -> KeyEvent {
        KeyEvent::new_with_kind(KeyCode::Esc, KeyModifiers::NONE, KeyEventKind::Press)
    }

    // ── Original behavior tests (unchanged) ──

    #[test]
    fn opens_and_closes() {
        let mut menu = LeaderMenu::new();
        assert!(!menu.visible);

        menu.open();
        assert!(menu.visible);

        menu.close();
        assert!(!menu.visible);
    }

    #[test]
    fn esc_closes_root() {
        let mut menu = LeaderMenu::new();
        menu.open();

        let result = menu.handle_key(&esc());
        assert!(result.is_none());
        assert!(!menu.visible);
    }

    #[test]
    fn unknown_key_dismisses() {
        let mut menu = LeaderMenu::new();
        menu.open();

        // 'z' is not bound to anything
        let result = menu.handle_key(&key('z'));
        assert!(result.is_none());
        assert!(!menu.visible);
    }

    #[test]
    fn direct_action_returns_and_closes() {
        let mut menu = LeaderMenu::new();
        menu.open();

        // 'm' → model selector
        let result = menu.handle_key(&key('m'));
        assert!(result.is_some());
        assert!(!menu.visible);

        match result.unwrap() {
            LeaderAction::KeymapAction(a) => {
                assert_eq!(
                    a,
                    clankers_tui_types::Action::Extended(clankers_tui_types::ExtendedAction::OpenModelSelector)
                );
            }
            _ => panic!("Expected KeymapAction"),
        }
    }

    #[test]
    fn submenu_navigation() {
        let mut menu = LeaderMenu::new();
        menu.open();

        // 's' → session submenu (should not return an action)
        let result = menu.handle_key(&key('s'));
        assert!(result.is_none());
        assert!(menu.visible);
        assert_eq!(menu.stack.len(), 2); // root + session

        // 'n' → new session (slash command)
        let result = menu.handle_key(&key('n'));
        assert!(result.is_some());
        assert!(!menu.visible);

        match result.unwrap() {
            LeaderAction::SlashCommand(cmd) => assert_eq!(cmd, "/new"),
            _ => panic!("Expected SlashCommand"),
        }
    }

    #[test]
    fn esc_goes_back_from_submenu() {
        let mut menu = LeaderMenu::new();
        menu.open();

        // Enter session submenu
        menu.handle_key(&key('s'));
        assert_eq!(menu.stack.len(), 2);

        // Esc → back to root (not closed)
        let result = menu.handle_key(&esc());
        assert!(result.is_none());
        assert!(menu.visible);
        assert_eq!(menu.stack.len(), 1);

        // Esc again → close
        menu.handle_key(&esc());
        assert!(!menu.visible);
    }

    #[test]
    fn shift_key_matches_uppercase() {
        let mut menu = LeaderMenu::new();
        menu.open();

        // 'T' (Shift+t) → show/hide thinking
        let result = menu.handle_key(&shift_key('T'));
        assert!(result.is_some());

        match result.unwrap() {
            LeaderAction::KeymapAction(a) => {
                assert_eq!(
                    a,
                    clankers_tui_types::Action::Extended(clankers_tui_types::ExtendedAction::ToggleShowThinking)
                );
            }
            _ => panic!("Expected KeymapAction"),
        }
    }

    #[test]
    fn handles_not_visible() {
        let mut menu = LeaderMenu::new();
        // Should return None without panicking when not visible
        let result = menu.handle_key(&key('m'));
        assert!(result.is_none());
    }

    // ── Dynamic registration tests ──

    struct TestContributor {
        items: Vec<MenuContribution>,
    }

    impl MenuContributor for TestContributor {
        fn menu_items(&self) -> Vec<MenuContribution> {
            self.items.clone()
        }
    }

    #[test]
    fn build_with_single_contributor() {
        let contrib = TestContributor {
            items: vec![
                MenuContribution {
                    key: 'a',
                    label: "alpha".into(),
                    action: LeaderAction::SlashCommand("/alpha".into()),
                    placement: MenuPlacement::Root,
                    priority: PRIORITY_BUILTIN,
                    source: "test".into(),
                },
                MenuContribution {
                    key: 'b',
                    label: "beta".into(),
                    action: LeaderAction::SlashCommand("/beta".into()),
                    placement: MenuPlacement::Root,
                    priority: PRIORITY_BUILTIN,
                    source: "test".into(),
                },
            ],
        };

        let (menu, conflicts) = LeaderMenu::build(&[&contrib], &HashSet::new());
        assert!(conflicts.is_empty());
        assert_eq!(menu.root.items.len(), 2);
        assert_eq!(menu.root.items[0].key, 'a');
        assert_eq!(menu.root.items[1].key, 'b');
    }

    #[test]
    fn higher_priority_wins_conflict() {
        let builtin = TestContributor {
            items: vec![MenuContribution {
                key: 'x',
                label: "builtin-x".into(),
                action: LeaderAction::SlashCommand("/builtin".into()),
                placement: MenuPlacement::Root,
                priority: PRIORITY_BUILTIN,
                source: "builtin".into(),
            }],
        };
        let plugin = TestContributor {
            items: vec![MenuContribution {
                key: 'x',
                label: "plugin-x".into(),
                action: LeaderAction::SlashCommand("/plugin".into()),
                placement: MenuPlacement::Root,
                priority: PRIORITY_PLUGIN,
                source: "my-plugin".into(),
            }],
        };

        let (menu, conflicts) = LeaderMenu::build(&[&builtin, &plugin], &HashSet::new());
        assert_eq!(conflicts.len(), 1);
        assert_eq!(conflicts[0].winner, "my-plugin");
        assert_eq!(conflicts[0].loser, "builtin");

        // Plugin wins
        assert_eq!(menu.root.items.len(), 1);
        assert_eq!(menu.root.items[0].label, "plugin-x");
    }

    #[test]
    fn user_overrides_everything() {
        let builtin = TestContributor {
            items: vec![MenuContribution {
                key: 'z',
                label: "builtin-z".into(),
                action: LeaderAction::SlashCommand("/builtin".into()),
                placement: MenuPlacement::Root,
                priority: PRIORITY_BUILTIN,
                source: "builtin".into(),
            }],
        };
        let user = TestContributor {
            items: vec![MenuContribution {
                key: 'z',
                label: "user-z".into(),
                action: LeaderAction::SlashCommand("/user".into()),
                placement: MenuPlacement::Root,
                priority: PRIORITY_USER,
                source: "config".into(),
            }],
        };

        let (menu, _) = LeaderMenu::build(&[&builtin, &user], &HashSet::new());
        assert_eq!(menu.root.items[0].label, "user-z");
    }

    #[test]
    fn hidden_entries_excluded() {
        let contrib = TestContributor {
            items: vec![
                MenuContribution {
                    key: 'a',
                    label: "keep".into(),
                    action: LeaderAction::SlashCommand("/keep".into()),
                    placement: MenuPlacement::Root,
                    priority: PRIORITY_BUILTIN,
                    source: "test".into(),
                },
                MenuContribution {
                    key: 'b',
                    label: "hide-me".into(),
                    action: LeaderAction::SlashCommand("/hide".into()),
                    placement: MenuPlacement::Root,
                    priority: PRIORITY_BUILTIN,
                    source: "test".into(),
                },
            ],
        };

        let mut hidden = HashSet::new();
        hidden.insert(('b', MenuPlacement::Root));

        let (menu, _) = LeaderMenu::build(&[&contrib], &hidden);
        assert_eq!(menu.root.items.len(), 1);
        assert_eq!(menu.root.items[0].key, 'a');
    }

    #[test]
    fn submenu_auto_creation() {
        let contrib = TestContributor {
            items: vec![
                MenuContribution {
                    key: 'p',
                    label: "plugins".into(),
                    action: LeaderAction::Submenu("plugins".into()),
                    placement: MenuPlacement::Root,
                    priority: PRIORITY_BUILTIN,
                    source: "test".into(),
                },
                MenuContribution {
                    key: 'c',
                    label: "calendar".into(),
                    action: LeaderAction::SlashCommand("/cal".into()),
                    placement: MenuPlacement::Submenu("plugins".into()),
                    priority: PRIORITY_PLUGIN,
                    source: "calendar".into(),
                },
            ],
        };

        let (menu, _) = LeaderMenu::build(&[&contrib], &HashSet::new());

        // Root should have the submenu opener
        assert_eq!(menu.root.items.len(), 1);
        assert_eq!(menu.root.items[0].key, 'p');

        // Submenu should exist with one item
        let plugins_sub = menu.submenus.iter().find(|s| s.label == "plugins").unwrap();
        assert_eq!(plugins_sub.items.len(), 1);
        assert_eq!(plugins_sub.items[0].key, 'c');
        assert_eq!(plugins_sub.items[0].label, "calendar");
    }

    #[test]
    fn empty_contributors_produce_empty_menu() {
        let (menu, conflicts) = LeaderMenu::build(&[], &HashSet::new());
        assert!(conflicts.is_empty());
        assert!(menu.root.items.is_empty());
        assert!(menu.submenus.is_empty());
    }

    #[test]
    fn same_key_different_placement_no_conflict() {
        let contrib = TestContributor {
            items: vec![
                MenuContribution {
                    key: 'a',
                    label: "root-a".into(),
                    action: LeaderAction::SlashCommand("/root".into()),
                    placement: MenuPlacement::Root,
                    priority: PRIORITY_BUILTIN,
                    source: "test".into(),
                },
                MenuContribution {
                    key: 'a',
                    label: "sub-a".into(),
                    action: LeaderAction::SlashCommand("/sub".into()),
                    placement: MenuPlacement::Submenu("foo".into()),
                    priority: PRIORITY_BUILTIN,
                    source: "test".into(),
                },
            ],
        };

        let (menu, conflicts) = LeaderMenu::build(&[&contrib], &HashSet::new());
        assert!(conflicts.is_empty());
        assert_eq!(menu.root.items.len(), 1);
        let foo_sub = menu.submenus.iter().find(|s| s.label == "foo").unwrap();
        assert_eq!(foo_sub.items.len(), 1);
    }

    // NOTE: default_menu_has_expected_structure test lives in main crate
    // (needs crate::slash_commands::builtin_command_infos)
}
