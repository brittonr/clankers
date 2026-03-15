# Spec: Extensible Keybinding Actions

## Overview

Split the 52-variant `Action` enum into a stable core (editor/navigation) and
an extensible layer (feature-specific actions). Plugins and features register
extended actions at runtime instead of adding enum variants.

## Current Pain

```rust
// src/config/keybindings.rs — 52 variants
pub enum Action {
    ScrollUp, ScrollDown, PageUp, PageDown,         // core
    InsertMode, NormalMode,                          // core
    ToggleBlockIds, ToggleSessionPopup,              // feature-specific
    OpenModelSelector, OpenAccountSelector,          // feature-specific
    OpenLeaderMenu, TogglePanelFocus,               // feature-specific
    // ... 40 more
}
```

`parse_action()` is a 55-arm match mapping strings to variants, maintained
manually. Adding a feature action means editing the enum, `parse_action()`,
and any preset keybinding functions.

## Split

### `CoreAction` — stable, exhaustive

~20 actions that are fundamental to the editor/TUI and rarely change:

```rust
pub enum CoreAction {
    // Scrolling
    ScrollUp, ScrollDown, PageUp, PageDown,
    HalfPageUp, HalfPageDown,
    ScrollToTop, ScrollToBottom,

    // Cursor movement
    MoveLeft, MoveRight, MoveToStart, MoveToEnd,
    MoveWordForward, MoveWordBackward,

    // Mode switching
    InsertMode, NormalMode,

    // Input
    SubmitInput, Cancel,
    DeleteChar, DeleteWord, DeleteToStart, DeleteToEnd,

    // Clipboard
    Yank, Paste,

    // App
    Quit,
}
```

These use an exhaustive `match` — the compiler catches missing arms.

### `ExtendedAction` — string-keyed, open

Everything else:

```rust
pub struct ExtendedActionDef {
    /// Action name (e.g. "open_leader_menu", "toggle_thinking").
    pub name: String,
    /// Human-readable description.
    pub description: String,
    /// Handler function.
    pub handler: Box<dyn Fn(&mut ActionContext) + Send + Sync>,
    pub source: String,
}

pub struct ActionContext<'a> {
    pub app: &'a mut App,
    pub cmd_tx: &'a UnboundedSender<AgentCommand>,
    // ... similar to SlashContext
}
```

### Unified `Action` type

```rust
pub enum Action {
    Core(CoreAction),
    Extended(String),  // name, looked up in registry
}
```

### `ActionRegistry`

```rust
pub struct ActionRegistry {
    actions: HashMap<String, ExtendedActionDef>,
}

impl ActionRegistry {
    pub fn register(&mut self, def: ExtendedActionDef) { ... }

    pub fn dispatch(&self, name: &str, ctx: &mut ActionContext) -> bool {
        if let Some(def) = self.actions.get(name) {
            (def.handler)(ctx);
            true
        } else {
            false
        }
    }
}
```

## `parse_action()` Migration

```rust
pub fn parse_action(s: &str) -> Option<Action> {
    // Try core first
    match s {
        "scroll_up" => Some(Action::Core(CoreAction::ScrollUp)),
        "scroll_down" => Some(Action::Core(CoreAction::ScrollDown)),
        // ... ~20 stable mappings
        _ => {
            // Treat as extended action name
            Some(Action::Extended(s.to_string()))
        }
    }
}
```

Unknown strings become `Extended` — validation happens at dispatch time, not
parse time. This means keybinding config can reference actions that haven't
been registered yet (plugin actions loaded after config parse).

## Builtin Extended Actions

Feature code registers its own actions at init:

```rust
// In interactive.rs init, or each feature's module
registry.register(ExtendedActionDef {
    name: "open_leader_menu".into(),
    description: "Open the Space key popup menu".into(),
    handler: Box::new(|ctx| ctx.app.leader_menu.open()),
    source: "builtin".into(),
});

registry.register(ExtendedActionDef {
    name: "toggle_thinking".into(),
    description: "Cycle thinking level".into(),
    handler: Box::new(|ctx| {
        // Move thinking toggle logic here
    }),
    source: "builtin".into(),
});
```

## Plugin Actions

Plugins can register actions via their manifest:

```json
{
  "actions": [
    {
      "name": "calendar_quick_add",
      "description": "Quick-add a calendar event"
    }
  ]
}
```

The handler calls into the plugin's WASM `handle_action` export.

## Keybinding Config

Users bind keys to any action name — core or extended:

```toml
[keybindings.normal]
"g g" = "scroll_to_top"         # core
"Space" = "open_leader_menu"    # extended (builtin)
"C-k" = "calendar_quick_add"   # extended (plugin)
```

## Scope Note

This is the lowest-priority phase because:
1. The core actions are stable and rarely change.
2. The extended actions are mostly internal (not user/plugin facing yet).
3. The refactor touches keybinding parsing, which is delicate.

Ship after slash commands and panels are stable.
