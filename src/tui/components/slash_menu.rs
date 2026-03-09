//! Slash command autocomplete menu
//!
//! Renders a floating popup above the editor showing matching slash commands
//! as the user types.

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
use ratatui::widgets::List;
use ratatui::widgets::ListItem;

use clankers_tui_types::CompletionItem;
use crate::tui::theme::Theme;

/// State for the slash command autocomplete menu
#[derive(Debug, Clone)]
pub struct SlashMenu {
    /// Currently matching commands
    pub items: Vec<SlashMenuItem>,
    /// Currently selected index
    pub selected: usize,
    /// Whether the menu is visible
    pub visible: bool,
}

#[derive(Debug, Clone)]
pub struct SlashMenuItem {
    /// Display name (e.g. "account" or "switch <name>")
    pub display: String,
    /// Description shown in menu
    pub description: String,
    /// Full text to insert (without leading `/`)
    pub insert_text: String,
    /// Whether to add trailing space after accepting
    pub trailing_space: bool,
}

impl From<CompletionItem> for SlashMenuItem {
    fn from(item: CompletionItem) -> Self {
        Self {
            display: item.display,
            description: item.description.to_string(),
            insert_text: item.insert_text,
            trailing_space: item.trailing_space,
        }
    }
}

impl SlashMenu {
    pub fn new() -> Self {
        Self {
            items: Vec::new(),
            selected: 0,
            visible: false,
        }
    }

    /// Update the menu with new completions based on the current input
    pub fn update(&mut self, source: &dyn clankers_tui_types::CompletionSource, input: &str) {
        let completions = source.completions(input);
        if completions.is_empty() || !input.starts_with('/') {
            self.visible = false;
            self.items.clear();
            self.selected = 0;
            return;
        }

        self.items = completions.into_iter().map(SlashMenuItem::from).collect();
        self.visible = true;

        // Clamp selected index
        if self.selected >= self.items.len() {
            self.selected = self.items.len().saturating_sub(1);
        }
    }

    /// Move selection up
    pub fn select_prev(&mut self) {
        if !self.items.is_empty() {
            self.selected = self.selected.checked_sub(1).unwrap_or(self.items.len() - 1);
        }
    }

    /// Move selection down
    pub fn select_next(&mut self) {
        if !self.items.is_empty() {
            self.selected = (self.selected + 1) % self.items.len();
        }
    }

    /// Accept the currently selected item. Returns (insert_text, trailing_space).
    pub fn accept(&mut self) -> Option<(String, bool)> {
        if self.visible && self.selected < self.items.len() {
            let insert_text = self.items[self.selected].insert_text.clone();
            let trailing_space = self.items[self.selected].trailing_space;
            self.hide();
            Some((insert_text, trailing_space))
        } else {
            None
        }
    }

    /// Hide the menu
    pub fn hide(&mut self) {
        self.visible = false;
        self.items.clear();
        self.selected = 0;
    }
}

impl Default for SlashMenu {
    fn default() -> Self {
        Self::new()
    }
}

/// Render the slash menu popup above the editor area.
/// `editor_area` is the Rect of the editor widget — the menu renders just above it.
/// `cursor_x` is the absolute screen X position of the cursor so the menu follows it.
pub fn render_slash_menu(frame: &mut Frame, menu: &SlashMenu, theme: &Theme, editor_area: Rect, cursor_x: u16) {
    if !menu.visible || menu.items.is_empty() {
        return;
    }

    let max_visible = 10.min(menu.items.len());
    let menu_height = max_visible as u16 + 2; // +2 for borders
    let menu_width = 50.min(editor_area.width);

    // Position above the editor, aligned to the cursor X
    let y = editor_area.y.saturating_sub(menu_height);
    let screen_width = frame.area().width;
    // Clamp so the menu doesn't overflow the right edge
    let x = cursor_x.min(screen_width.saturating_sub(menu_width));
    let popup = Rect::new(x, y, menu_width, menu_height);

    frame.render_widget(Clear, popup);

    // Compute the visible window so the selected item is always on screen
    let scroll_offset = if menu.selected >= max_visible {
        menu.selected - max_visible + 1
    } else {
        0
    };

    let items: Vec<ListItem> = menu
        .items
        .iter()
        .skip(scroll_offset)
        .take(max_visible)
        .enumerate()
        .map(|(i, item)| {
            let actual_index = scroll_offset + i;
            let style = if actual_index == menu.selected {
                Style::default().fg(Color::White).bg(theme.highlight).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(theme.fg)
            };

            let line = Line::from(vec![
                Span::styled(format!(" {}", item.display), style.add_modifier(Modifier::BOLD)),
                Span::styled(format!("  {}", item.description), style),
            ]);
            ListItem::new(line)
        })
        .collect();

    let list = List::new(items).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(theme.highlight))
            .title(" Commands "),
    );

    frame.render_widget(list, popup);
}
