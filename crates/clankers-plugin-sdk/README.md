# clankers-plugin-sdk

SDK for building [clankers](https://github.com/brittonr/clankers) WASM plugins.

Eliminates the boilerplate of hand-rolling protocol types, JSON parsing,
tool routing, and event dispatch. Focus on your plugin's logic.

## Quick start

**Cargo.toml:**
```toml
[package]
name = "my-plugin"
version = "0.1.0"
edition = "2021"

[lib]
crate-type = ["cdylib"]

[dependencies]
clankers-plugin-sdk = { path = "../../crates/clankers-plugin-sdk" }
extism-pdk = "1"
# Add serde if you define #[derive(Serialize/Deserialize)] structs:
# serde = { version = "1", features = ["derive"] }

[workspace]
```

**src/lib.rs:**
```rust
use clankers_plugin_sdk::prelude::*;

fn handle_greet(args: &Value) -> Result<String, String> {
    let name = args.require_str("name")?;
    let excited = args.get_bool_or("excited", false);
    let suffix = if excited { "!!!" } else { "." };
    Ok(format!("Hello, {name}{suffix}"))
}

#[plugin_fn]
pub fn handle_tool_call(input: String) -> FnResult<String> {
    dispatch_tools(&input, &[
        ("greet", handle_greet),
    ])
}

#[plugin_fn]
pub fn on_event(input: String) -> FnResult<String> {
    dispatch_events(&input, "my-plugin", &[
        ("agent_start", |_| "Plugin ready".to_string()),
        ("agent_end",   |_| "Shutting down".to_string()),
    ])
}

#[plugin_fn]
pub fn describe(Json(_): Json<()>) -> FnResult<Json<PluginMeta>> {
    Ok(Json(PluginMeta::new("my-plugin", "0.1.0", &[
        ("greet", "Greet someone by name"),
    ], &[])))
}
```

**plugin.json:**
```json
{
  "name": "my-plugin",
  "version": "0.1.0",
  "description": "A friendly greeting plugin",
  "wasm": "my_plugin.wasm",
  "kind": "extism",
  "permissions": [],
  "tools": ["greet"],
  "commands": [],
  "events": ["agent_start", "agent_end"],
  "tool_definitions": [
    {
      "name": "greet",
      "description": "Greet someone by name",
      "handler": "handle_tool_call",
      "input_schema": {
        "type": "object",
        "properties": {
          "name": { "type": "string", "description": "Name to greet" },
          "excited": { "type": "boolean", "description": "Add excitement" }
        },
        "required": ["name"]
      }
    }
  ],
  "leader_menu": [
    {
      "key": "g",
      "label": "greet",
      "command": "/greet",
      "submenu": "plugins"
    }
  ]
}
```

**Build:**
```bash
cargo build --release --target wasm32-unknown-unknown
cp target/wasm32-unknown-unknown/release/my_plugin.wasm .
```

## What the SDK provides

### Protocol types (`types`)
- `ToolCall` — inbound tool invocation from the host
- `ToolResult` — outbound response (with `ok()`, `error()`, `unknown()` constructors)
- `Event` — inbound lifecycle event
- `EventResult` — outbound response (with `handled()`, `unhandled()` constructors)
- `PluginMeta` / `ToolMeta` — plugin self-description

### Dispatch helpers (`dispatch`)
- `dispatch_tools(input, &[("name", handler_fn)])` — parse, route, serialize
- `dispatch_events(input, plugin_name, &[("event", handler_fn)])` — same for events

### Arg extraction (`args`)
- `args.require_str("key")` → `Result<&str, String>`
- `args.get_str_or("key", "default")` → `&str`
- `args.get_u64_or("key", 0)` → `u64`
- `args.get_bool_or("key", false)` → `bool`
- `args.get_f64_or("key", 0.0)` → `f64`
- `args.get_str_array("key")` → `Vec<String>`
- `args.get_array("key")` → `Option<&Vec<Value>>`

### Prelude
```rust
use clankers_plugin_sdk::prelude::*;
```
Brings in all types, dispatch functions, arg helpers, and essential
`extism_pdk` / `serde` re-exports.

## Leader menu integration

Plugins can add entries to the leader menu (Space key in normal mode) by
declaring `leader_menu` in `plugin.json`:

```json
{
  "leader_menu": [
    {
      "key": "c",
      "label": "calendar",
      "command": "/cal",
      "submenu": "plugins"
    },
    {
      "key": "t",
      "label": "today's events",
      "command": "/cal today"
    }
  ]
}
```

| Field | Required | Description |
|-------|----------|-------------|
| `key` | yes | Single character key to press (must be printable ASCII) |
| `label` | yes | Display label shown in the menu |
| `command` | yes | Slash command to execute (must start with `/`) |
| `submenu` | no | Submenu name. Omit for root level. `"plugins"` is conventional. |

Plugin entries have priority 100, overriding builtins (0) but not user
config (200). If two plugins claim the same key at the same placement, the
last-loaded plugin wins and a warning is logged.

Use `/leader` in clankers to inspect the current menu structure.

## Dependencies note

The `#[plugin_fn]` and `#[derive(Serialize)]` proc macros require their
crates to be direct dependencies (they generate code referencing crate
names). So your Cargo.toml needs:

- `extism-pdk = "1"` — always (for `#[plugin_fn]`)
- `serde = { version = "1", features = ["derive"] }` — only if you define
  custom `#[derive(Serialize/Deserialize)]` structs. Not needed if you only
  use the SDK's built-in types.
