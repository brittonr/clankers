//! Helix-style leader key (Space) popup menu.
//!
//! Pressing Space in normal mode opens a which-key overlay showing available
//! actions. Single-key press executes an action or opens a submenu.
//! Escape or any unrecognized key dismisses the menu.

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
    /// Create a leader menu with the default Helix-style bindings.
    pub fn new() -> Self {
        let session_menu = LeaderMenuDef {
            label: "session".into(),
            items: vec![
                LeaderMenuItem {
                    key: 'n',
                    label: "new".into(),
                    action: LeaderAction::SlashCommand("/new".into()),
                },
                LeaderMenuItem {
                    key: 'f',
                    label: "fork".into(),
                    action: LeaderAction::SlashCommand("/fork".into()),
                },
                LeaderMenuItem {
                    key: 'r',
                    label: "resume".into(),
                    action: LeaderAction::SlashCommand("/resume".into()),
                },
                LeaderMenuItem {
                    key: 'l',
                    label: "list sessions".into(),
                    action: LeaderAction::SlashCommand("/sessions".into()),
                },
                LeaderMenuItem {
                    key: 'c',
                    label: "compact".into(),
                    action: LeaderAction::SlashCommand("/compact".into()),
                },
            ],
        };

        let root = LeaderMenuDef {
            label: "Leader".into(),
            items: vec![
                LeaderMenuItem {
                    key: 's',
                    label: "session".into(),
                    action: LeaderAction::Submenu("session".into()),
                },
                LeaderMenuItem {
                    key: 'm',
                    label: "model".into(),
                    action: LeaderAction::KeymapAction(crate::config::keybindings::Action::OpenModelSelector),
                },
                LeaderMenuItem {
                    key: 'a',
                    label: "account".into(),
                    action: LeaderAction::KeymapAction(crate::config::keybindings::Action::OpenAccountSelector),
                },
                LeaderMenuItem {
                    key: 't',
                    label: "toggle thinking".into(),
                    action: LeaderAction::KeymapAction(crate::config::keybindings::Action::ToggleThinking),
                },
                LeaderMenuItem {
                    key: 'T',
                    label: "show/hide thinking".into(),
                    action: LeaderAction::KeymapAction(crate::config::keybindings::Action::ToggleShowThinking),
                },
                LeaderMenuItem {
                    key: 'f',
                    label: "search output".into(),
                    action: LeaderAction::KeymapAction(crate::config::keybindings::Action::SearchOutput),
                },
                LeaderMenuItem {
                    key: '`',
                    label: "toggle panel".into(),
                    action: LeaderAction::KeymapAction(crate::config::keybindings::Action::TogglePanelFocus),
                },
                LeaderMenuItem {
                    key: 'l',
                    label: "layout".into(),
                    action: LeaderAction::Submenu("layout".into()),
                },
                LeaderMenuItem {
                    key: 'o',
                    label: "external editor".into(),
                    action: LeaderAction::KeymapAction(crate::config::keybindings::Action::OpenEditor),
                },
                LeaderMenuItem {
                    key: '?',
                    label: "help".into(),
                    action: LeaderAction::SlashCommand("/help".into()),
                },
            ],
        };

        let layout_menu = LeaderMenuDef {
            label: "layout".into(),
            items: vec![
                LeaderMenuItem {
                    key: 'd',
                    label: "default (3-column)".into(),
                    action: LeaderAction::SlashCommand("/layout default".into()),
                },
                LeaderMenuItem {
                    key: 'w',
                    label: "wide chat".into(),
                    action: LeaderAction::SlashCommand("/layout wide".into()),
                },
                LeaderMenuItem {
                    key: 'f',
                    label: "focused (no panels)".into(),
                    action: LeaderAction::SlashCommand("/layout focused".into()),
                },
                LeaderMenuItem {
                    key: 'r',
                    label: "right-heavy".into(),
                    action: LeaderAction::SlashCommand("/layout right".into()),
                },
                LeaderMenuItem {
                    key: '1',
                    label: "toggle Todo".into(),
                    action: LeaderAction::SlashCommand("/layout toggle todo".into()),
                },
                LeaderMenuItem {
                    key: '2',
                    label: "toggle Files".into(),
                    action: LeaderAction::SlashCommand("/layout toggle files".into()),
                },
                LeaderMenuItem {
                    key: '3',
                    label: "toggle Subagents".into(),
                    action: LeaderAction::SlashCommand("/layout toggle subagents".into()),
                },
                LeaderMenuItem {
                    key: '4',
                    label: "toggle Peers".into(),
                    action: LeaderAction::SlashCommand("/layout toggle peers".into()),
                },
            ],
        };

        Self {
            visible: false,
            stack: Vec::new(),
            breadcrumb: Vec::new(),
            submenus: vec![session_menu, layout_menu],
            root,
        }
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
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
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
                assert_eq!(a, crate::config::keybindings::Action::OpenModelSelector);
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
                assert_eq!(a, crate::config::keybindings::Action::ToggleShowThinking);
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
}
