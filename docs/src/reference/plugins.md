# Plugins

Plugins are WebAssembly modules loaded via [Extism](https://extism.org). They add tools the agent can call at runtime.

## Installing a plugin

Drop a `plugin.json` + `.wasm` file into `plugins/`, or:

```bash
clankers plugin install <path>
```

## Plugin manifest

Each plugin needs a `plugin.json`:

```json
{
  "name": "clankers-wordcount",
  "version": "0.1.0",
  "wasm": "clankers_wordcount.wasm",
  "kind": "extism",
  "tools": ["wordcount"],
  "tool_definitions": [
    {
      "name": "wordcount",
      "description": "Count words, lines, and characters in text",
      "handler": "handle_tool_call",
      "input_schema": {
        "type": "object",
        "properties": {
          "text": { "type": "string" }
        },
        "required": ["text"]
      }
    }
  ]
}
```

## Writing a plugin in Rust

Use the `clanker-plugin-sdk` crate. The Rust side is a single Extism guest function:

```rust
use extism_pdk::*;

#[plugin_fn]
pub fn handle_tool_call(input: String) -> FnResult<String> {
    let call: ToolCallInput = serde_json::from_str(&input)?;
    // do work, return JSON result
}
```

Build for WASM:

```bash
cargo build --target wasm32-unknown-unknown --release
# or use the xtask helper:
cargo xtask build-plugins
```

## Shipped plugins

calendar, email, github, hash, self-validate, text-stats.

## Listing plugins

```
/plugin             # list all plugins in the TUI
/plugin wordcount   # show details for a specific plugin
/tools              # list all available tools (built-in + plugin)
```
