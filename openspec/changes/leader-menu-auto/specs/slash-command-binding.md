# Spec: Slash Command Leader Bindings

## Overview

Each `SlashCommand` gains an optional `leader_key` field. When present, the
command automatically appears in the leader menu without any additional
registration code.

## Changes to SlashCommand

```rust
// src/slash_commands/mod.rs

pub struct SlashCommand {
    pub name: &'static str,
    pub description: &'static str,
    pub help: &'static str,
    pub accepts_args: bool,
    pub action: SlashAction,
    pub subcommands: Vec<(&'static str, &'static str)>,

    /// When set, this command appears in the leader menu.
    pub leader_key: Option<LeaderBinding>,
}

pub struct LeaderBinding {
    /// Key to press in the leader menu.
    pub key: char,
    /// Where in the menu this appears.
    pub placement: MenuPlacement,
    /// Override label. Defaults to `description` if None.
    pub label: Option<&'static str>,
}
```

## Default Bindings

These replicate the current hardcoded leader menu. Existing behavior is
preserved exactly.

### Root Level

| Slash Command | Key | Label | Notes |
|---------------|-----|-------|-------|
| `/compact` | `C` | "compact" | |
| `/help` | `?` | "help" | |

### Session Submenu

| Slash Command | Key | Label |
|---------------|-----|-------|
| `/new` (via `/session new`) | `n` | "new" |
| `/fork` | `f` | "fork" |
| `/resume` (via `/session resume`) | `r` | "resume" |
| `/sessions` (via `/session list`) | `l` | "list sessions" |
| `/compact` | `c` | "compact" |

### Layout Submenu

| Slash Command | Key | Label |
|---------------|-----|-------|
| `/layout default` | `d` | "default (3-column)" |
| `/layout wide` | `w` | "wide chat" |
| `/layout focused` | `f` | "focused (no panels)" |
| `/layout right` | `r` | "right-heavy" |
| `/layout toggle todo` | `1` | "toggle Todo" |
| `/layout toggle files` | `2` | "toggle Files" |
| `/layout toggle subagents` | `3` | "toggle Subagents" |
| `/layout toggle peers` | `4` | "toggle Peers" |

### Non-Slash-Command Root Items

These aren't slash commands — they map to `LeaderAction::KeymapAction` and are
contributed by a separate `builtin_keymap_contributions()` function:

| Key | Label | Action |
|-----|-------|--------|
| `m` | "model" | `OpenModelSelector` |
| `a` | "account" | `OpenAccountSelector` |
| `t` | "toggle thinking" | `ToggleThinking` |
| `T` | "show/hide thinking" | `ToggleShowThinking` |
| `f` | "search output" | `SearchOutput` |
| `` ` `` | "toggle panel" | `TogglePanelFocus` |
| `o` | "external editor" | `OpenEditor` |
| `c` | "cancel/abort" | `Cancel` |
| `x` | "clear input" | `ClearLine` |

Plus submenu openers:

| Key | Label | Action |
|-----|-------|--------|
| `s` | "session" | `Submenu("session")` |
| `l` | "layout" | `Submenu("layout")` |

## Adapter Function

```rust
/// Convert slash commands into menu contributions.
pub fn slash_command_contributions(commands: &[SlashCommand]) -> Vec<MenuContribution> {
    commands.iter()
        .filter_map(|cmd| {
            let b = cmd.leader_key.as_ref()?;
            Some(MenuContribution {
                key: b.key,
                label: b.label.unwrap_or(cmd.description).to_string(),
                action: LeaderAction::SlashCommand(format!("/{}", cmd.name)),
                placement: b.placement.clone(),
                priority: PRIORITY_BUILTIN,
                source: "builtin".into(),
            })
        })
        .collect()
}

/// Keymap actions and submenu openers that aren't slash commands.
pub fn builtin_keymap_contributions() -> Vec<MenuContribution> {
    vec![
        MenuContribution {
            key: 'm',
            label: "model".into(),
            action: LeaderAction::KeymapAction(Action::OpenModelSelector),
            placement: MenuPlacement::Root,
            priority: PRIORITY_BUILTIN,
            source: "builtin".into(),
        },
        // ... rest of non-slash items
    ]
}
```

## Migration

The existing `LeaderMenu::new()` is replaced by `LeaderMenu::build()`. The
default menu is assembled from `slash_command_contributions(builtin_commands())`
+ `builtin_keymap_contributions()`. No user-visible behavior changes.
