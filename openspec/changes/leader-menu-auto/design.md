# Design: Dynamic Leader Menu Registration

## Architecture

```
┌──────────────────┐    ┌──────────────────┐    ┌──────────────────┐
│  Builtin Slash   │    │  PluginManager   │    │   User Config    │
│   Commands       │    │  (WASM manifests)│    │ (settings.toml)  │
└────────┬─────────┘    └────────┬─────────┘    └────────┬─────────┘
         │                       │                       │
         │ impl MenuContributor  │ impl MenuContributor  │ impl MenuContributor
         │                       │                       │
         ▼                       ▼                       ▼
    ┌────────────────────────────────────────────────────────┐
    │              LeaderMenu::build(contributors)           │
    │                                                        │
    │  1. Collect all MenuContribution items                  │
    │  2. Group by submenu path                              │
    │  3. Resolve key conflicts (user > plugin > builtin)    │
    │  4. Build LeaderMenuDef tree                           │
    │                                                        │
    └──────────────────────┬─────────────────────────────────┘
                           │
                           ▼
                    ┌──────────────┐
                    │  LeaderMenu  │
                    │  (runtime)   │
                    └──────────────┘
```

## Core Types

```rust
/// Where a menu item should appear.
#[derive(Debug, Clone, PartialEq, Eq)]
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
    /// Builtins: 0, plugins: 100, user config: 200.
    pub priority: u16,
    /// Source identifier for diagnostics ("builtin", plugin name, "config").
    pub source: String,
}

/// Anything that contributes items to the leader menu.
pub trait MenuContributor {
    fn menu_items(&self) -> Vec<MenuContribution>;
}
```

## Contributor Implementations

### 1. Builtin Slash Commands

Add an optional `leader_menu` field to `SlashCommand`:

```rust
pub struct SlashCommand {
    pub name: &'static str,
    pub description: &'static str,
    // ... existing fields ...

    /// Optional leader menu placement. When set, this command appears
    /// in the leader menu automatically.
    pub leader_key: Option<LeaderBinding>,
}

pub struct LeaderBinding {
    pub key: char,
    pub placement: MenuPlacement,
    /// Override label (defaults to SlashCommand.description).
    pub label: Option<&'static str>,
}
```

A free function converts all slash commands with `leader_key` into
`MenuContribution` items:

```rust
fn slash_command_contributions(commands: &[SlashCommand]) -> Vec<MenuContribution> {
    commands.iter()
        .filter_map(|cmd| {
            let binding = cmd.leader_key.as_ref()?;
            Some(MenuContribution {
                key: binding.key,
                label: binding.label.unwrap_or(cmd.description).to_string(),
                action: LeaderAction::SlashCommand(format!("/{}", cmd.name)),
                placement: binding.placement.clone(),
                priority: 0,
                source: "builtin".into(),
            })
        })
        .collect()
}
```

For actions that map to `LeaderAction::KeymapAction` (model selector, thinking
toggle, etc.), a separate `builtin_keymap_contributions()` function contributes
those since they aren't slash commands.

### 2. Plugin Manifest

Add optional `leader_menu` to `PluginManifest`:

```rust
// In plugin.json:
// {
//   "name": "calendar",
//   "leader_menu": [
//     { "key": "k", "label": "calendar", "command": "/cal", "submenu": "plugins" }
//   ]
// }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginLeaderEntry {
    pub key: char,
    pub label: String,
    pub command: String,
    #[serde(default)]
    pub submenu: Option<String>,
}
```

`PluginManager` implements `MenuContributor`, iterating loaded plugins and
converting their `leader_menu` entries.

### 3. User Config

New `[leader_menu]` section in settings:

```toml
# Override or add items
[[leader_menu.items]]
key = "g"
label = "git status"
command = "/shell git status"
placement = "root"

# Hide a builtin item
[[leader_menu.hide]]
key = "?"
placement = "root"

# Rebind a key
[[leader_menu.items]]
key = "n"
label = "new session"
command = "/new"
placement = "session"    # goes into session submenu
```

## Menu Build Process

```rust
impl LeaderMenu {
    pub fn build(contributors: &[&dyn MenuContributor], hidden: &HashSet<(char, MenuPlacement)>) -> Self {
        // 1. Collect all contributions
        let mut items: Vec<MenuContribution> = contributors
            .iter()
            .flat_map(|c| c.menu_items())
            .collect();

        // 2. Sort by priority (highest last = wins)
        items.sort_by_key(|i| i.priority);

        // 3. Deduplicate by (key, placement) — last writer wins
        let mut seen: HashMap<(char, MenuPlacement), MenuContribution> = HashMap::new();
        for item in items {
            seen.insert((item.key, item.placement.clone()), item);
        }

        // 4. Remove hidden entries
        for h in hidden {
            seen.remove(h);
        }

        // 5. Group into submenu defs and build the tree
        // ...
    }
}
```

## Rebuild Trigger

The menu must rebuild when plugins load/unload. `PluginManager` emits an event
(or the interactive loop calls `rebuild_leader_menu()`) after
`load_wasm`/`unload`. This is cheap — just re-collecting and sorting a small
vec.

## Key Conflict Resolution

When two sources want the same `(key, placement)`:

| Conflict | Resolution |
|----------|-----------|
| builtin vs builtin | Compile-time error (developer mistake) |
| plugin vs builtin | Plugin wins (priority 100 > 0) |
| plugin vs plugin | Last-loaded wins; warn on stderr |
| user vs anything | User wins (priority 200) |

Diagnostics: `LeaderMenu::build()` returns `(LeaderMenu, Vec<KeyConflict>)` so
the caller can log conflicts.

## File Changes

| File | Change |
|------|--------|
| `src/tui/components/leader_menu.rs` | Add `MenuContributor` trait, `MenuContribution`, `build()` method |
| `src/slash_commands/mod.rs` | Add `leader_key` field to `SlashCommand`, populate for builtins |
| `src/plugin/manifest.rs` | Add `leader_menu: Vec<PluginLeaderEntry>` to `PluginManifest` |
| `src/plugin/mod.rs` | Implement `MenuContributor` for `PluginManager` |
| `src/config/settings.rs` | Add `leader_menu` config section |
| `src/modes/interactive.rs` | Wire up `build()` at init, rebuild on plugin load |
| `src/tui/app.rs` | Change `LeaderMenu::new()` to `LeaderMenu::build(...)` |
