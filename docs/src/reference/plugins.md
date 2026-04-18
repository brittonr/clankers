# Plugins

clankers supports multiple plugin runtimes:

- `kind: "extism"` — WebAssembly plugins loaded via [Extism](https://extism.org)
- `kind: "stdio"` — supervised process plugins that speak the clankers stdio protocol
- `kind: "zellij"` — Zellij-backed integrations discovered through the same plugin surface

## Installing a plugin

Drop a plugin directory into a scanned plugin root (`plugins/`, `.clankers/plugins/`, or `~/.clankers/agent/plugins/`), or install it with:

```bash
clankers plugin install <path>
```

## Manifest basics

Every plugin needs a `plugin.json` with at least `name`, `version`, and `kind`.

### Extism manifest

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

### Stdio manifest

```json
{
  "name": "clankers-stdio-echo",
  "version": "0.1.0",
  "kind": "stdio",
  "stdio": {
    "command": "./plugin.py",
    "args": [],
    "working_dir": "plugin-dir",
    "env_allowlist": ["GITHUB_TOKEN"],
    "sandbox": "inherit",
    "writable_roots": ["build/output"],
    "allow_network": false
  }
}
```

Reference stdio example lives at `examples/plugins/clankers-stdio-echo/`.

## Stdio launch policy fields

`stdio` manifests use these fields:

- `command` — executable to launch. Required.
- `args` — command-line arguments. Optional.
- `working_dir` — `"plugin-dir"` or `"project-root"`.
- `env_allowlist` — environment variables forwarded to the child. In v1, every listed variable is required.
- `sandbox` — `"inherit"` or `"restricted"`.
- `writable_roots` — extra project-root-relative writable paths requested for restricted mode.
- `allow_network` — restricted-mode network allowance. Effective access still also requires logical `"net"` permission.

Important v1 rules:

- clankers resolves `stdio.command` before spawn, then clears the child environment
- no implicit child `PATH` inheritance is needed or forwarded
- only `env_allowlist` variables are passed through
- missing allowlisted variables fail plugin startup
- stdio tool inventory is live: the process must register tools after startup

## Stdio protocol expectations

A stdio plugin must speak the framed clankers process-extension protocol:

- each frame starts with a 4-byte big-endian length prefix
- payload is JSON
- every frame carries `plugin_protocol: 1`
- host sends `hello`, `event`, `tool_invoke`, `tool_cancel`, `shutdown`
- plugin sends `hello`, `ready`, `register_tools`, `unregister_tools`, `subscribe_events`, `tool_progress`, `tool_result`, `tool_error`, `tool_cancelled`, `ui`, `display`

Startup sequence:

1. host launches plugin process
2. host sends `hello`
3. plugin sends `hello`
4. plugin sends `ready`
5. plugin registers tools and optional event subscriptions

Tools are not live until the plugin sends `register_tools`.

## Sandbox modes

### `inherit`

`inherit` runs like a normal clankers child process, but still applies manifest-driven command resolution, working-directory selection, and environment filtering.

### `restricted`

`restricted` applies a host-enforced sandbox for bounded filesystem and network execution. The manifest may declare `writable_roots` and `allow_network`, and clankers derives a dedicated plugin state directory for that runtime profile.

Current status:

- Linux: clankers applies a restricted backend that bounds writes with Landlock and denies socket creation unless both logical `net` permission and sandbox `allow_network` permit network access
- unsupported hosts or unavailable restricted backends: clankers fails closed and refuses to start the plugin

## Writing an Extism plugin in Rust

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

## Migrating from manifest-only Extism tools to stdio

Use stdio when plugin needs:

- native libraries or external binaries
- long-lived state outside one WASM call
- another language runtime
- supervised restart and live tool registration

Migration checklist:

1. change `kind` from `extism` to `stdio`
2. replace `wasm` with a `stdio` launch policy
3. move static manifest-only tool ownership into runtime `register_tools`
4. move event subscriptions into runtime `subscribe_events`
5. keep permissions accurate: logical permissions still gate host-visible actions even for stdio plugins
6. start with `sandbox: "inherit"`; use `restricted` only when clankers can enforce it

## Shipped plugins

calendar, email, github, hash, self-validate, text-stats.

## Listing plugins

```
/plugin                    # list all plugins in the TUI
/plugin clankers-wordcount # show details for a specific plugin
/tools                     # list all available tools (built-in + plugin)
```
