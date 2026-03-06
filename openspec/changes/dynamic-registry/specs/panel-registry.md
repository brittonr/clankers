# Spec: Dynamic Panel Registry

## Overview

Replace the dual `PanelId` enum + `PanelTab` enum with a dynamic panel
registry. Panels register at init (or when plugins load) and are stored in an
ordered map. The 250-line nested match in `handle_action()` is replaced by
`Panel::handle_key_event()` dispatch.

## Current Pain

Adding one panel requires edits to:
1. `PanelId` enum + `PanelId::ALL` array (`src/tui/panel.rs`)
2. `PanelTab` enum + `is_left()` / `is_right()` / `to_panel_id()` (`src/tui/app.rs`)
3. Named field in `App` struct (`src/tui/app.rs`)
4. `PanelLayout::default()` presets (`src/tui/layout.rs`)
5. Panel-focused key dispatch in `handle_action()` (`src/modes/interactive.rs`)
6. Leader menu layout submenu (`src/tui/components/leader_menu.rs`)
7. Render function

`PanelTab` has 5 variants, `PanelId` has 6 — they're already out of sync
(`Environment` is in `PanelId` but not `PanelTab`).

## Panel Trait (extended)

The existing `Panel` trait in `src/tui/panel.rs` is extended:

```rust
pub trait Panel: Send {
    /// Unique identifier string.
    fn id(&self) -> &str;

    /// Display label for tab bar / leader menu.
    fn label(&self) -> &str;

    /// Preferred column (left or right).
    fn default_column(&self) -> PanelColumn;

    /// Handle a key event when this panel is focused.
    /// Returns whether the event was consumed.
    fn handle_key_event(&mut self, key: &KeyEvent) -> PanelKeyResult;

    /// Render the panel into the given area.
    fn render(&self, frame: &mut Frame, area: Rect, focused: bool);

    /// Whether this panel should be visible by default.
    fn default_visible(&self) -> bool { true }

    /// Optional: provide leader menu items for this panel.
    fn leader_menu_items(&self) -> Vec<MenuContribution> { vec![] }
}

pub enum PanelColumn { Left, Right }

pub enum PanelKeyResult {
    /// Key was consumed by the panel.
    Consumed,
    /// Key was not handled — bubble up to app.
    Ignored,
    /// Panel wants to trigger an app-level action.
    Action(Action),
}
```

## Panel Manager

```rust
/// Manages registered panels, their visibility, and focus state.
pub struct PanelManager {
    /// Ordered map of panels. Order determines tab cycling.
    panels: IndexMap<String, Box<dyn Panel>>,
    /// Which panels are currently visible.
    visible: HashSet<String>,
    /// Currently focused panel (if panel focus is active).
    focused: Option<String>,
}

impl PanelManager {
    pub fn new() -> Self { ... }

    /// Register a panel.
    pub fn register(&mut self, panel: Box<dyn Panel>) {
        let visible = panel.default_visible();
        let id = panel.id().to_string();
        self.panels.insert(id.clone(), panel);
        if visible {
            self.visible.insert(id);
        }
    }

    /// Toggle a panel's visibility.
    pub fn toggle(&mut self, id: &str) {
        if self.visible.contains(id) {
            self.visible.remove(id);
            if self.focused.as_deref() == Some(id) {
                self.focused = None;
            }
        } else {
            self.visible.insert(id.to_string());
        }
    }

    /// Cycle focus to the next panel in the same column.
    pub fn focus_next(&mut self) { ... }
    pub fn focus_prev(&mut self) { ... }

    /// Dispatch a key event to the focused panel.
    pub fn handle_key(&mut self, key: &KeyEvent) -> PanelKeyResult {
        if let Some(id) = &self.focused {
            if let Some(panel) = self.panels.get_mut(id) {
                return panel.handle_key_event(key);
            }
        }
        PanelKeyResult::Ignored
    }

    /// Get panels for a column, in order, filtered to visible.
    pub fn left_panels(&self) -> Vec<&dyn Panel> { ... }
    pub fn right_panels(&self) -> Vec<&dyn Panel> { ... }

    /// Get a panel by id (for direct state access).
    pub fn get<T: Panel + 'static>(&self, id: &str) -> Option<&T> {
        self.panels.get(id)?.downcast_ref()
    }

    pub fn get_mut<T: Panel + 'static>(&mut self, id: &str) -> Option<&mut T> {
        self.panels.get_mut(id)?.downcast_mut()
    }
}
```

Note: `downcast_ref` / `downcast_mut` require the `Panel` trait to be
`Any`-bounded (or use a separate mechanism). The alternative is typed accessor
methods on `App` that wrap the downcast.

## Changes to `App`

```rust
pub struct App {
    // DELETE these:
    // pub subagent_panel: SubagentPanel,
    // pub todo_panel: TodoPanel,
    // pub file_activity_panel: FileActivityPanel,
    // pub peers_panel: PeersPanel,
    // pub process_panel: ProcessPanel,

    // ADD this:
    pub panels: PanelManager,

    // Typed accessors for common panels (convenience, avoids downcast at every call site):
    // These call self.panels.get::<TodoPanel>("todo") internally.
}

impl App {
    pub fn todo_panel(&self) -> &TodoPanel {
        self.panels.get::<TodoPanel>("todo").expect("todo panel registered")
    }
    pub fn todo_panel_mut(&mut self) -> &mut TodoPanel {
        self.panels.get_mut::<TodoPanel>("todo").expect("todo panel registered")
    }
    // ... same for other builtin panels
}
```

## DELETE `PanelTab`

The `PanelTab` enum is deleted entirely. All code referencing it migrates to
`PanelManager` methods:

| Before | After |
|--------|-------|
| `app.panel_tab == PanelTab::Todo` | `app.panels.focused == Some("todo")` |
| `match app.panel_tab { ... }` | `app.panels.handle_key(key)` |
| `PanelTab::is_left()` | `panel.default_column() == PanelColumn::Left` |
| Cycling with hardcoded match | `app.panels.focus_next()` |

## Panel-Focused Key Dispatch

The 250-line nested match in `handle_action()` collapses to:

```rust
if app.panel_focused {
    match app.panels.handle_key(&key) {
        PanelKeyResult::Consumed => return,
        PanelKeyResult::Action(action) => {
            handle_action(app, action, ...);
            return;
        }
        PanelKeyResult::Ignored => {
            // Fall through to global handlers
        }
    }
}
```

## Leader Menu Integration

`PanelManager` implements `MenuContributor`:

```rust
impl MenuContributor for PanelManager {
    fn menu_items(&self) -> Vec<MenuContribution> {
        self.panels.values().enumerate().map(|(i, panel)| {
            MenuContribution {
                key: char::from_digit((i + 1) as u32, 10).unwrap_or('?'),
                label: format!("toggle {}", panel.label()),
                action: LeaderAction::SlashCommand(
                    format!("/layout toggle {}", panel.id())
                ),
                placement: MenuPlacement::Submenu("layout".into()),
                priority: PRIORITY_BUILTIN,
                source: "panel".into(),
            }
        }).collect()
    }
}
```

This auto-generates layout toggle entries (currently hardcoded as
`'1' toggle Todo`, `'2' toggle Files`, etc.).

## Layout Presets

Layout presets change from hardcoded `PanelId` enum references to string lists:

```rust
pub fn default_three_column() -> PanelLayout {
    PanelLayout {
        visible: vec!["todo", "files", "subagents", "peers"]
            .into_iter().map(String::from).collect(),
        // ...
    }
}
```

Unknown panel IDs in a preset are silently ignored (allows plugins that may
not be loaded).

## Migration Strategy

1. Implement `PanelManager` alongside existing fields (both exist temporarily).
2. Migrate one panel at a time — start with `ProcessPanel` (least coupled).
3. Add typed accessors to `App` for each migrated panel.
4. After all panels migrated, delete `PanelTab` enum and `PanelId` enum.
5. Collapse the `handle_action` match block.
