//! Helix-style leader key (Space) popup menu.
//!
//! Pressing Space in normal mode opens a which-key overlay showing available
//! actions. Single-key press executes an action or opens a submenu.
//! Escape or any unrecognized key dismisses the menu.
//!
//! The menu is built dynamically from [`MenuContributor`] implementations,
//! allowing builtins, plugins, and user config to contribute items.

use std::collections::HashMap;
use std::collections::HashSet;

use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use crossterm::event::KeyModifiers;
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::Color;
use ratatui::style::Modifier;
use ratatui::style::Style;
use ratatui::text::Line;
use ratatui::text::Span;
use ratatui::widgets::Block;
use ratatui::widgets::Borders;
use ratatui::widgets::Clear;
use ratatui::widgets::Paragraph;

use crate::registry::Conflict;

// ---------------------------------------------------------------------------
// Leader actions — things the leader menu can trigger
// ---------------------------------------------------------------------------

/// An action that a leader menu item can trigger.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LeaderAction {
    /// Trigger a normal-mode `Action` (reuses the existing action enum).
    KeymapAction(crate::config::keybindings::Action),
    /// Execute a slash command string (e.g. "/new", "/compact").
    SlashCommand(String),
    /// Open a named submenu.
    Submenu(String),
}

// ---------------------------------------------------------------------------
// Menu item
// ---------------------------------------------------------------------------

/// A single entry in the leader key menu.
#[derive(Debug, Clone)]
pub struct LeaderMenuItem {
    /// The key to press (single char, e.g. 's', 'm').
    pub key: char,
    /// Display label.
    pub label: String,
    /// What happens when this item is selected.
    pub action: LeaderAction,
}

// ---------------------------------------------------------------------------
// Menu definition (a flat level of items)
// ---------------------------------------------------------------------------

/// A named menu (root or submenu).
#[derive(Debug, Clone)]
pub struct LeaderMenuDef {
    pub label: String,
    pub items: Vec<LeaderMenuItem>,
}

// ---------------------------------------------------------------------------
// Dynamic registration types
// ---------------------------------------------------------------------------

/// Where a menu item should appear.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum MenuPlacement {
    /// Top-level root menu.
    Root,
    /// Inside a named submenu (created if it doesn't exist).
    Submenu(String),
}

/// A single contribution to the leader menu from any source.
#[derive(Debug, Clone)]
pub struct MenuContribution {
    /// Key to press (single char).
    pub key: char,
    /// Display label.
    pub label: String,
    /// What happens when selected.
    pub action: LeaderAction,
    /// Where this item appears.
    pub placement: MenuPlacement,
    /// Priority for conflict resolution (higher wins).
    pub priority: u16,
    /// Source identifier for diagnostics ("builtin", plugin name, "config").
    pub source: String,
}

/// Anything that contributes items to the leader menu.
pub trait MenuContributor {
    fn menu_items(&self) -> Vec<MenuContribution>;
}

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
    /// Create a leader menu with the default hardcoded bindings.
    ///
    /// Prefer [`LeaderMenu::build`] for dynamic registration.
    pub fn new() -> Self {
        let slash_contrib = SlashCommandContributor::new(crate::slash_commands::builtin_commands());
        Self::build(&[&BuiltinKeymapContributor, &slash_contrib], &HashSet::new()).0
    }

    /// Build a leader menu from contributors.
    ///
    /// Collects all [`MenuContribution`] items, deduplicates by `(key, placement)`
    /// with highest priority winning, removes hidden entries, and assembles the
    /// menu tree.
    pub fn build(
        contributors: &[&dyn MenuContributor],
        hidden: &HashSet<(char, MenuPlacement)>,
    ) -> (Self, Vec<Conflict>) {
        let mut conflicts = Vec::new();

        // 1. Collect all contributions
        let mut all_items: Vec<MenuContribution> = contributors
            .iter()
            .flat_map(|c| c.menu_items())
            .collect();

        // 2. Sort by priority (lowest first, so highest overwrites)
        all_items.sort_by_key(|i| i.priority);

        // 3. Deduplicate by (key, placement) — last writer wins
        let mut seen: HashMap<(char, MenuPlacement), MenuContribution> = HashMap::new();
        for item in all_items {
            let key = (item.key, item.placement.clone());
            if let Some(existing) = seen.get(&key) {
                conflicts.push(Conflict {
                    registry: "leader_menu",
                    key: format!("'{}' in {:?}", item.key, item.placement),
                    winner: item.source.clone(),
                    loser: existing.source.clone(),
                });
            }
            seen.insert(key, item);
        }

        // 4. Remove hidden entries
        for h in hidden {
            seen.remove(h);
        }

        // 5. Group by placement
        let mut root_items: Vec<MenuContribution> = Vec::new();
        let mut submenu_items: HashMap<String, Vec<MenuContribution>> = HashMap::new();

        for ((_, placement), item) in seen {
            match placement {
                MenuPlacement::Root => root_items.push(item),
                MenuPlacement::Submenu(ref name) => {
                    submenu_items.entry(name.clone()).or_default().push(item);
                }
            }
        }

        // 6. Build submenu defs
        let mut submenus: Vec<LeaderMenuDef> = Vec::new();
        for (name, mut items) in submenu_items {
            // Sort items by key for consistent ordering
            items.sort_by_key(|i| i.key);
            submenus.push(LeaderMenuDef {
                label: name,
                items: items
                    .into_iter()
                    .map(|c| LeaderMenuItem {
                        key: c.key,
                        label: c.label,
                        action: c.action,
                    })
                    .collect(),
            });
        }

        // 7. Build root def — sort items by key for consistent ordering
        root_items.sort_by_key(|i| i.key);
        let root = LeaderMenuDef {
            label: "Leader".into(),
            items: root_items
                .into_iter()
                .map(|c| LeaderMenuItem {
                    key: c.key,
                    label: c.label,
                    action: c.action,
                })
                .collect(),
        };

        let menu = Self {
            visible: false,
            stack: Vec::new(),
            breadcrumb: Vec::new(),
            submenus,
            root,
        };

        (menu, conflicts)
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

    // ── Rendering ────────────────────────────────────────────────────

    /// Render the leader menu overlay.
    pub fn render(&self, frame: &mut Frame, area: Rect) {
        if !self.visible {
            return;
        }

        let current = match self.current() {
            Some(m) => m,
            None => return,
        };

        // Calculate popup dimensions
        let item_count = current.items.len() as u16;
        // width: widest "  k  label…" + padding + borders
        let max_label_w = current
            .items
            .iter()
            .map(|i| {
                let suffix = if matches!(i.action, LeaderAction::Submenu(_)) {
                    1 // "…"
                } else {
                    0
                };
                // "  k  label" = 5 + label.len + suffix
                5 + i.label.len() as u16 + suffix
            })
            .max()
            .unwrap_or(10);
        let content_width = max_label_w + 2; // padding
        let width = (content_width + 2).min(area.width.saturating_sub(4)); // + borders
        let height = (item_count + 4).min(area.height.saturating_sub(4)); // items + title + spacer + hint + borders

        // Center the popup
        let x = (area.width.saturating_sub(width)) / 2;
        let y = (area.height.saturating_sub(height)) / 2;
        let popup_area = Rect::new(x, y, width, height);

        // Clear background
        frame.render_widget(Clear, popup_area);

        // Build title with breadcrumb
        let title = if self.breadcrumb.is_empty() {
            " Space ".to_string()
        } else {
            format!(" Space › {} ", self.breadcrumb.join(" › "))
        };

        let block = Block::default()
            .title(Span::styled(title, Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)))
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::DarkGray));

        let inner = block.inner(popup_area);
        frame.render_widget(block, popup_area);

        if inner.height == 0 || inner.width == 0 {
            return;
        }

        // Render menu items
        let mut lines: Vec<Line> = Vec::new();

        for item in &current.items {
            let is_submenu = matches!(item.action, LeaderAction::Submenu(_));
            let suffix = if is_submenu { "…" } else { "" };

            lines.push(Line::from(vec![
                Span::raw("  "),
                Span::styled(item.key.to_string(), Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
                Span::raw("  "),
                Span::styled(format!("{}{}", item.label, suffix), Style::default().fg(Color::White)),
            ]));
        }

        // Spacer
        lines.push(Line::from(""));

        // Hint
        let hint_text = if self.stack.len() > 1 { "esc back" } else { "esc close" };
        lines.push(Line::from(vec![
            Span::raw("  "),
            Span::styled(hint_text, Style::default().fg(Color::DarkGray)),
        ]));

        let paragraph = Paragraph::new(lines);
        frame.render_widget(paragraph, inner);
    }
}

// ---------------------------------------------------------------------------
// Builtin contributors
// ---------------------------------------------------------------------------

/// Contributes the hardcoded keymap actions and submenu openers that aren't
/// slash commands (model selector, thinking toggle, etc.)
pub struct BuiltinKeymapContributor;

impl MenuContributor for BuiltinKeymapContributor {
    fn menu_items(&self) -> Vec<MenuContribution> {
        use crate::config::keybindings::{Action, CoreAction};
        use crate::registry::PRIORITY_BUILTIN;

        vec![
            // ── Root: submenu openers ──
            MenuContribution {
                key: 's',
                label: "session".into(),
                action: LeaderAction::Submenu("session".into()),
                placement: MenuPlacement::Root,
                priority: PRIORITY_BUILTIN,
                source: "builtin".into(),
            },
            MenuContribution {
                key: 'l',
                label: "layout".into(),
                action: LeaderAction::Submenu("layout".into()),
                placement: MenuPlacement::Root,
                priority: PRIORITY_BUILTIN,
                source: "builtin".into(),
            },
            // ── Root: keymap actions ──
            MenuContribution {
                key: 'm',
                label: "model".into(),
                action: LeaderAction::KeymapAction(Action::Extended("open_model_selector".into())),
                placement: MenuPlacement::Root,
                priority: PRIORITY_BUILTIN,
                source: "builtin".into(),
            },
            MenuContribution {
                key: 'a',
                label: "account".into(),
                action: LeaderAction::KeymapAction(Action::Extended("open_account_selector".into())),
                placement: MenuPlacement::Root,
                priority: PRIORITY_BUILTIN,
                source: "builtin".into(),
            },
            MenuContribution {
                key: 't',
                label: "toggle thinking".into(),
                action: LeaderAction::KeymapAction(Action::Extended("toggle_thinking".into())),
                placement: MenuPlacement::Root,
                priority: PRIORITY_BUILTIN,
                source: "builtin".into(),
            },
            MenuContribution {
                key: 'T',
                label: "show/hide thinking".into(),
                action: LeaderAction::KeymapAction(Action::Extended("toggle_show_thinking".into())),
                placement: MenuPlacement::Root,
                priority: PRIORITY_BUILTIN,
                source: "builtin".into(),
            },
            MenuContribution {
                key: 'f',
                label: "search output".into(),
                action: LeaderAction::KeymapAction(Action::Extended("search_output".into())),
                placement: MenuPlacement::Root,
                priority: PRIORITY_BUILTIN,
                source: "builtin".into(),
            },
            MenuContribution {
                key: '`',
                label: "toggle panel".into(),
                action: LeaderAction::KeymapAction(Action::Extended("toggle_panel_focus".into())),
                placement: MenuPlacement::Root,
                priority: PRIORITY_BUILTIN,
                source: "builtin".into(),
            },
            MenuContribution {
                key: 'o',
                label: "external editor".into(),
                action: LeaderAction::KeymapAction(Action::Extended("open_editor".into())),
                placement: MenuPlacement::Root,
                priority: PRIORITY_BUILTIN,
                source: "builtin".into(),
            },
            MenuContribution {
                key: 'c',
                label: "cancel/abort".into(),
                action: LeaderAction::KeymapAction(Action::Core(CoreAction::Cancel)),
                placement: MenuPlacement::Root,
                priority: PRIORITY_BUILTIN,
                source: "builtin".into(),
            },
            MenuContribution {
                key: 'x',
                label: "clear input".into(),
                action: LeaderAction::KeymapAction(Action::Core(CoreAction::ClearLine)),
                placement: MenuPlacement::Root,
                priority: PRIORITY_BUILTIN,
                source: "builtin".into(),
            },
            // ── Root: slash commands (moved to SlashCommand definitions) ──
            // ── Session submenu ──
            MenuContribution {
                key: 'n',
                label: "new".into(),
                action: LeaderAction::SlashCommand("/new".into()),
                placement: MenuPlacement::Submenu("session".into()),
                priority: PRIORITY_BUILTIN,
                source: "builtin".into(),
            },

            MenuContribution {
                key: 'r',
                label: "resume".into(),
                action: LeaderAction::SlashCommand("/resume".into()),
                placement: MenuPlacement::Submenu("session".into()),
                priority: PRIORITY_BUILTIN,
                source: "builtin".into(),
            },
            MenuContribution {
                key: 'l',
                label: "list sessions".into(),
                action: LeaderAction::SlashCommand("/sessions".into()),
                placement: MenuPlacement::Submenu("session".into()),
                priority: PRIORITY_BUILTIN,
                source: "builtin".into(),
            },
            MenuContribution {
                key: 'c',
                label: "compact".into(),
                action: LeaderAction::SlashCommand("/compact".into()),
                placement: MenuPlacement::Submenu("session".into()),
                priority: PRIORITY_BUILTIN,
                source: "builtin".into(),
            },
            // ── Layout submenu ──
            MenuContribution {
                key: 'd',
                label: "default (3-column)".into(),
                action: LeaderAction::SlashCommand("/layout default".into()),
                placement: MenuPlacement::Submenu("layout".into()),
                priority: PRIORITY_BUILTIN,
                source: "builtin".into(),
            },
            MenuContribution {
                key: 'w',
                label: "wide chat".into(),
                action: LeaderAction::SlashCommand("/layout wide".into()),
                placement: MenuPlacement::Submenu("layout".into()),
                priority: PRIORITY_BUILTIN,
                source: "builtin".into(),
            },
            MenuContribution {
                key: 'f',
                label: "focused (no panels)".into(),
                action: LeaderAction::SlashCommand("/layout focused".into()),
                placement: MenuPlacement::Submenu("layout".into()),
                priority: PRIORITY_BUILTIN,
                source: "builtin".into(),
            },
            MenuContribution {
                key: 'r',
                label: "right-heavy".into(),
                action: LeaderAction::SlashCommand("/layout right".into()),
                placement: MenuPlacement::Submenu("layout".into()),
                priority: PRIORITY_BUILTIN,
                source: "builtin".into(),
            },
            MenuContribution {
                key: '1',
                label: "toggle Todo".into(),
                action: LeaderAction::SlashCommand("/layout toggle todo".into()),
                placement: MenuPlacement::Submenu("layout".into()),
                priority: PRIORITY_BUILTIN,
                source: "builtin".into(),
            },
            MenuContribution {
                key: '2',
                label: "toggle Files".into(),
                action: LeaderAction::SlashCommand("/layout toggle files".into()),
                placement: MenuPlacement::Submenu("layout".into()),
                priority: PRIORITY_BUILTIN,
                source: "builtin".into(),
            },
            MenuContribution {
                key: '3',
                label: "toggle Subagents".into(),
                action: LeaderAction::SlashCommand("/layout toggle subagents".into()),
                placement: MenuPlacement::Submenu("layout".into()),
                priority: PRIORITY_BUILTIN,
                source: "builtin".into(),
            },
            MenuContribution {
                key: '4',
                label: "toggle Peers".into(),
                action: LeaderAction::SlashCommand("/layout toggle peers".into()),
                placement: MenuPlacement::Submenu("layout".into()),
                priority: PRIORITY_BUILTIN,
                source: "builtin".into(),
            },
            MenuContribution {
                key: '5',
                label: "toggle Processes".into(),
                action: LeaderAction::SlashCommand("/layout toggle processes".into()),
                placement: MenuPlacement::Submenu("layout".into()),
                priority: PRIORITY_BUILTIN,
                source: "builtin".into(),
            },
            MenuContribution {
                key: '6',
                label: "toggle Branches".into(),
                action: LeaderAction::SlashCommand("/layout toggle branches".into()),
                placement: MenuPlacement::Submenu("layout".into()),
                priority: PRIORITY_BUILTIN,
                source: "builtin".into(),
            },
            // ── Tiling submenu opener (from root) ──
            MenuContribution {
                key: 'p',
                label: "pane".into(),
                action: LeaderAction::Submenu("pane".into()),
                placement: MenuPlacement::Root,
                priority: PRIORITY_BUILTIN,
                source: "builtin".into(),
            },
            // ── Pane submenu items ──
            MenuContribution {
                key: 'z',
                label: "zoom toggle".into(),
                action: LeaderAction::KeymapAction(Action::Extended("pane_zoom".into())),
                placement: MenuPlacement::Submenu("pane".into()),
                priority: PRIORITY_BUILTIN,
                source: "builtin".into(),
            },
            MenuContribution {
                key: 'v',
                label: "split vertical".into(),
                action: LeaderAction::KeymapAction(Action::Extended("pane_split_vertical".into())),
                placement: MenuPlacement::Submenu("pane".into()),
                priority: PRIORITY_BUILTIN,
                source: "builtin".into(),
            },
            MenuContribution {
                key: 'h',
                label: "split horizontal".into(),
                action: LeaderAction::KeymapAction(Action::Extended("pane_split_horizontal".into())),
                placement: MenuPlacement::Submenu("pane".into()),
                priority: PRIORITY_BUILTIN,
                source: "builtin".into(),
            },
            MenuContribution {
                key: 'x',
                label: "close pane".into(),
                action: LeaderAction::KeymapAction(Action::Extended("pane_close".into())),
                placement: MenuPlacement::Submenu("pane".into()),
                priority: PRIORITY_BUILTIN,
                source: "builtin".into(),
            },
            MenuContribution {
                key: '=',
                label: "equalize size".into(),
                action: LeaderAction::KeymapAction(Action::Extended("pane_equalize".into())),
                placement: MenuPlacement::Submenu("pane".into()),
                priority: PRIORITY_BUILTIN,
                source: "builtin".into(),
            },
            MenuContribution {
                key: '+',
                label: "grow pane".into(),
                action: LeaderAction::KeymapAction(Action::Extended("pane_grow".into())),
                placement: MenuPlacement::Submenu("pane".into()),
                priority: PRIORITY_BUILTIN,
                source: "builtin".into(),
            },
            MenuContribution {
                key: '-',
                label: "shrink pane".into(),
                action: LeaderAction::KeymapAction(Action::Extended("pane_shrink".into())),
                placement: MenuPlacement::Submenu("pane".into()),
                priority: PRIORITY_BUILTIN,
                source: "builtin".into(),
            },
            MenuContribution {
                key: 'H',
                label: "move left".into(),
                action: LeaderAction::KeymapAction(Action::Extended("pane_move_left".into())),
                placement: MenuPlacement::Submenu("pane".into()),
                priority: PRIORITY_BUILTIN,
                source: "builtin".into(),
            },
            MenuContribution {
                key: 'L',
                label: "move right".into(),
                action: LeaderAction::KeymapAction(Action::Extended("pane_move_right".into())),
                placement: MenuPlacement::Submenu("pane".into()),
                priority: PRIORITY_BUILTIN,
                source: "builtin".into(),
            },
            MenuContribution {
                key: 'J',
                label: "move down".into(),
                action: LeaderAction::KeymapAction(Action::Extended("pane_move_down".into())),
                placement: MenuPlacement::Submenu("pane".into()),
                priority: PRIORITY_BUILTIN,
                source: "builtin".into(),
            },
            MenuContribution {
                key: 'K',
                label: "move up".into(),
                action: LeaderAction::KeymapAction(Action::Extended("pane_move_up".into())),
                placement: MenuPlacement::Submenu("pane".into()),
                priority: PRIORITY_BUILTIN,
                source: "builtin".into(),
            },
        ]
    }
}

/// Convert slash commands with `leader_key` bindings into menu contributions.
pub fn slash_command_contributions(commands: &[crate::slash_commands::SlashCommand]) -> Vec<MenuContribution> {
    use crate::registry::PRIORITY_BUILTIN;

    commands
        .iter()
        .filter_map(|cmd| {
            let binding = cmd.leader_key.as_ref()?;
            Some(MenuContribution {
                key: binding.key,
                label: binding
                    .label
                    .unwrap_or(cmd.description)
                    .to_string(),
                action: LeaderAction::SlashCommand(format!("/{}", cmd.name)),
                placement: binding.placement.clone(),
                priority: PRIORITY_BUILTIN,
                source: "builtin".into(),
            })
        })
        .collect()
}

/// Wrapper to make slash commands act as a MenuContributor.
pub struct SlashCommandContributor {
    commands: Vec<crate::slash_commands::SlashCommand>,
}

impl SlashCommandContributor {
    pub fn new(commands: Vec<crate::slash_commands::SlashCommand>) -> Self {
        Self { commands }
    }
}

impl MenuContributor for SlashCommandContributor {
    fn menu_items(&self) -> Vec<MenuContribution> {
        slash_command_contributions(&self.commands)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use crossterm::event::KeyEventKind;

    use super::*;
    use crate::registry::PRIORITY_BUILTIN;
    use crate::registry::PRIORITY_PLUGIN;
    use crate::registry::PRIORITY_USER;

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
                assert_eq!(a, crate::config::keybindings::Action::Extended("open_model_selector".to_string()));
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
                assert_eq!(a, crate::config::keybindings::Action::Extended("toggle_show_thinking".to_string()));
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

    #[test]
    fn default_menu_has_expected_structure() {
        // Verify the default menu built from BuiltinKeymapContributor
        // matches the old hardcoded structure
        let menu = LeaderMenu::new();

        // Root should have all the expected items
        let root_keys: Vec<char> = menu.root.items.iter().map(|i| i.key).collect();
        assert!(root_keys.contains(&'s'), "missing session submenu");
        assert!(root_keys.contains(&'m'), "missing model");
        assert!(root_keys.contains(&'a'), "missing account");
        assert!(root_keys.contains(&'t'), "missing thinking");
        assert!(root_keys.contains(&'T'), "missing show thinking");
        assert!(root_keys.contains(&'l'), "missing layout");
        assert!(root_keys.contains(&'?'), "missing help");
        assert!(root_keys.contains(&'C'), "missing compact");

        // Session submenu should exist
        let session = menu.submenus.iter().find(|s| s.label == "session").unwrap();
        let session_keys: Vec<char> = session.items.iter().map(|i| i.key).collect();
        assert!(session_keys.contains(&'n'), "missing new in session");
        assert!(session_keys.contains(&'f'), "missing fork in session");
        assert!(session_keys.contains(&'r'), "missing resume in session");

        // Layout submenu should exist
        let layout = menu.submenus.iter().find(|s| s.label == "layout").unwrap();
        let layout_keys: Vec<char> = layout.items.iter().map(|i| i.key).collect();
        assert!(layout_keys.contains(&'d'), "missing default layout");
        assert!(layout_keys.contains(&'w'), "missing wide layout");
        assert!(layout_keys.contains(&'1'), "missing toggle todo");
    }
}
