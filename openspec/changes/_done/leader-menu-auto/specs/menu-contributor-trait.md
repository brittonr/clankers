# Spec: MenuContributor Trait

## Overview

The `MenuContributor` trait is the single extension point for adding items to
the leader menu. All sources — builtins, plugins, user config — implement this
trait.

## Trait Definition

```rust
// src/tui/components/leader_menu.rs

/// Where a menu item should appear.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum MenuPlacement {
    Root,
    Submenu(String),
}

/// A single contribution to the leader menu.
#[derive(Debug, Clone)]
pub struct MenuContribution {
    pub key: char,
    pub label: String,
    pub action: LeaderAction,
    pub placement: MenuPlacement,
    pub priority: u16,
    pub source: String,
}

/// Anything that contributes items to the leader menu.
pub trait MenuContributor {
    fn menu_items(&self) -> Vec<MenuContribution>;
}
```

## Priority Constants

```rust
pub const PRIORITY_BUILTIN: u16 = 0;
pub const PRIORITY_PLUGIN: u16 = 100;
pub const PRIORITY_USER: u16 = 200;
```

## Submenu Auto-Creation

When a `MenuContribution` targets `Submenu("foo")` and no submenu named "foo"
exists yet, the builder automatically:

1. Creates the submenu definition
2. Adds a root-level entry pointing to it (key = first char of name, label =
   name)
3. If the auto-assigned root key conflicts, the submenu entry is skipped (user
   must add it manually via config)

Builtins that create submenus (session, layout) contribute an explicit root
entry with `LeaderAction::Submenu(name)` — they don't rely on auto-creation.

## Conflict Diagnostics

```rust
#[derive(Debug)]
pub struct KeyConflict {
    pub key: char,
    pub placement: MenuPlacement,
    pub winner: String,   // source that won
    pub loser: String,    // source that lost
}

impl LeaderMenu {
    pub fn build(
        contributors: &[&dyn MenuContributor],
        hidden: &HashSet<(char, MenuPlacement)>,
    ) -> (Self, Vec<KeyConflict>) { ... }
}
```

## Invariants

- A `(key, placement)` pair is unique within a built menu. No duplicate keys
  in the same level.
- `key` must be a printable ASCII character (a-z, A-Z, 0-9, punctuation).
  The builder silently drops contributions with non-printable keys.
- `label` must not be empty. Empty labels are replaced with the source name.
- Submenu depth is limited to 2 (root → submenu → items). Deeper nesting is
  flattened.

## Testing

- Unit test: two contributors with conflicting `(key, placement)` — higher
  priority wins, conflict is reported.
- Unit test: `MenuPlacement::Submenu("new-thing")` auto-creates the submenu.
- Unit test: hidden entries are excluded.
- Unit test: empty contributors produce a minimal default menu (just the
  builtin keymap actions).
