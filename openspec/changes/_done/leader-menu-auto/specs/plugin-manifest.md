# Spec: Plugin Manifest Leader Menu Entries

## Overview

Plugins declare leader menu items in `plugin.json`. When a plugin loads, its
entries are contributed to the leader menu. When it unloads, they're removed.

## Manifest Schema

```json
{
  "name": "calendar",
  "version": "0.2.0",
  "tools": ["cal_list_events", "cal_create_event"],
  "commands": ["/cal"],
  "leader_menu": [
    {
      "key": "k",
      "label": "calendar",
      "command": "/cal",
      "submenu": "plugins"
    },
    {
      "key": "l",
      "label": "list events",
      "command": "/cal list",
      "submenu": "calendar"
    },
    {
      "key": "c",
      "label": "create event",
      "command": "/cal create",
      "submenu": "calendar"
    }
  ]
}
```

## Rust Types

```rust
// src/plugin/manifest.rs

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginManifest {
    // ... existing fields ...

    /// Optional leader menu entries contributed by this plugin.
    #[serde(default)]
    pub leader_menu: Vec<PluginLeaderEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginLeaderEntry {
    /// Key to press.
    pub key: char,
    /// Display label.
    pub label: String,
    /// Slash command to execute (e.g. "/cal list").
    pub command: String,
    /// Submenu name. If omitted, goes to root.
    /// "plugins" is conventional for plugin top-level items.
    #[serde(default)]
    pub submenu: Option<String>,
}
```

## PluginManager as MenuContributor

```rust
// src/plugin/mod.rs

impl MenuContributor for PluginManager {
    fn menu_items(&self) -> Vec<MenuContribution> {
        self.loaded_plugins()
            .flat_map(|plugin| {
                plugin.manifest.leader_menu.iter().map(move |entry| {
                    MenuContribution {
                        key: entry.key,
                        label: entry.label.clone(),
                        action: LeaderAction::SlashCommand(entry.command.clone()),
                        placement: match &entry.submenu {
                            Some(name) => MenuPlacement::Submenu(name.clone()),
                            None => MenuPlacement::Root,
                        },
                        priority: PRIORITY_PLUGIN,
                        source: plugin.manifest.name.clone(),
                    }
                })
            })
            .collect()
    }
}
```

## Auto-Created "plugins" Submenu

By convention, plugins place their top-level entry in `"submenu": "plugins"`.
The builder auto-creates this submenu if any plugin targets it. The root-level
entry for the "plugins" submenu uses key `p` (if available) and label
"plugins…".

If no plugins declare `leader_menu`, the "plugins" submenu doesn't appear.

## Rebuild on Load/Unload

After `PluginManager::load_wasm()` or unload, the interactive loop calls:

```rust
app.leader_menu = LeaderMenu::build(&contributors, &hidden).0;
```

This is called from the existing plugin-load path in `interactive.rs`. The
build is cheap (iterating a small vec of items).

## Validation

On manifest parse, warn and skip entries where:
- `key` is not a printable ASCII character
- `label` is empty
- `command` doesn't start with "/"
- `command` references a slash command that doesn't exist (warning only, not
  a hard error — the command might be from another plugin)
