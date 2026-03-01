# clankers-wordcount

An example clankers plugin that provides word count and text statistics tools.

## Tools

| Tool | Description |
|------|-------------|
| `wordcount` | Count words, lines, characters, and bytes in text |
| `textstats` | Detailed statistics: average word length, top frequent words, sentence count, reading time estimate |

## Building

Requires a Rust nightly toolchain with the `wasm32-unknown-unknown` target:

```sh
rustup target add wasm32-unknown-unknown
cd examples/plugins/clankers-wordcount
./build.sh
```

This produces `clankers_wordcount.wasm` in the plugin directory.

## Installing

```sh
clankers plugin install examples/plugins/clankers-wordcount
```

Or install to a specific project:

```sh
clankers plugin install examples/plugins/clankers-wordcount --project
```

## Verifying

```sh
clankers plugin list
# ✓ clankers-wordcount v0.1.0 — Word count and text statistics plugin
```

## How it works

clankers discovers plugins by scanning for directories containing a `plugin.json`
manifest. The manifest declares:

- **name** and **version** — plugin identity
- **wasm** — the WASM file to load (compiled from Rust via `extism-pdk`)
- **tool_definitions** — tools the LLM can call, with JSON Schema inputs
- **permissions** — sandbox capabilities (this plugin needs none)
- **events** — agent lifecycle events to subscribe to (this plugin uses none)

When the agent needs to use a tool, clankers calls the `handler` function specified
in `tool_definitions` (here, `handle_tool_call`), passing a JSON payload:

```json
{ "tool": "wordcount", "args": { "text": "hello world" } }
```

The plugin returns:

```json
{ "tool": "wordcount", "result": "...", "status": "ok" }
```

## Creating your own plugin

1. Copy this directory as a starting point
2. Edit `plugin.json` with your plugin's name, tools, and schemas
3. Implement `handle_tool_call` in `src/lib.rs`
4. Build with `./build.sh`
5. Install with `clankers plugin install <path>`
