# Spec: User Config for Leader Menu

## Overview

Users can add, override, rebind, and hide leader menu entries via the settings
file. User config always wins over builtins and plugins (priority 200).

## Config Format

In `settings.toml` (or the clankers settings JSON, whichever format is used):

```toml
[leader_menu]
# Add custom items
[[leader_menu.items]]
key = "g"
label = "git status"
command = "/shell git status"

[[leader_menu.items]]
key = "d"
label = "diff review"
command = "/review"
submenu = "session"

# Hide items you don't use
[[leader_menu.hide]]
key = "?"           # remove help from root

[[leader_menu.hide]]
key = "c"
submenu = "session"  # remove compact from session submenu
```

## Rust Types

```rust
// src/config/settings.rs

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct LeaderMenuConfig {
    #[serde(default)]
    pub items: Vec<LeaderMenuItemConfig>,
    #[serde(default)]
    pub hide: Vec<LeaderMenuHideConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LeaderMenuItemConfig {
    pub key: char,
    pub label: String,
    pub command: String,
    #[serde(default)]
    pub submenu: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LeaderMenuHideConfig {
    pub key: char,
    #[serde(default)]
    pub submenu: Option<String>,
}
```

## MenuContributor Implementation

```rust
impl MenuContributor for LeaderMenuConfig {
    fn menu_items(&self) -> Vec<MenuContribution> {
        self.items.iter().map(|item| {
            MenuContribution {
                key: item.key,
                label: item.label.clone(),
                action: LeaderAction::SlashCommand(item.command.clone()),
                placement: match &item.submenu {
                    Some(name) => MenuPlacement::Submenu(name.clone()),
                    None => MenuPlacement::Root,
                },
                priority: PRIORITY_USER,
                source: "config".into(),
            }
        }).collect()
    }
}
```

The `hide` list is passed separately to `LeaderMenu::build()` as the
`hidden: HashSet<(char, MenuPlacement)>` parameter.

## Capabilities

| What | How |
|------|-----|
| Add new root item | `items` with no `submenu` |
| Add item to existing submenu | `items` with `submenu = "session"` |
| Create new submenu | `items` targeting a new submenu name (auto-created) |
| Override builtin key | `items` with same `(key, placement)` — user priority wins |
| Remove builtin item | `hide` with matching `(key, placement)` |
| Rebind a key | `hide` old key + `items` with new key and same command |

## Limitations

- User config can only trigger `SlashCommand` actions, not raw `KeymapAction`.
  This is intentional — keymap actions should be configured via keybindings,
  not the leader menu config.
- Submenu depth limited to 2 levels. User config cannot create nested submenus.
