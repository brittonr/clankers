# Plugin Migration — Extism to Hyperlight

## Summary

Migrate clankers' WASM plugin system from Extism to aspen's Hyperlight-wasm
host.  This unifies the plugin runtime, adds capability-based permissions,
KV namespace isolation, and hot reload support.

## Current Plugin System (Extism)

```
plugins/
  clankers-hash/
    plugin.json         ← manifest
    clankers_hash.wasm  ← WASM binary
  clankers-text-stats/
    plugin.json
    clankers_text_stats.wasm
```

**plugin.json format:**
```json
{
  "name": "clankers-hash",
  "version": "0.1.0",
  "wasm": "clankers_hash.wasm",
  "kind": "extism",
  "tools": ["hash"],
  "tool_definitions": [{
    "name": "hash",
    "description": "Hash text with SHA-256, BLAKE3, etc.",
    "handler": "handle_tool_call",
    "input_schema": { ... }
  }]
}
```

**Guest code (Extism PDK):**
```rust
use extism_pdk::*;

#[plugin_fn]
pub fn handle_tool_call(input: String) -> FnResult<String> {
    let call: ToolCallInput = serde_json::from_str(&input)?;
    // execute tool logic
    Ok(serde_json::to_string(&result)?)
}
```

## Target Plugin System (Hyperlight)

### Manifest (in aspen KV)

```
clankers:plugins:clankers-hash:manifest
  → {
      "name": "clankers-hash",
      "version": "0.1.0",
      "wasm_hash": "<blake3-hash>",     ← WASM bytes in iroh-blobs
      "tools": ["hash"],
      "tool_definitions": [{ ... }],
      "permissions": {
        "kv_read": false,
        "kv_write": false,
        "blob_read": false,
        "blob_write": false,
        "randomness": true,
        "timers": false,
        "hooks": false
      },
      "kv_prefixes": []                 ← auto-scoped to __plugin:clankers-hash:
    }
```

### Guest code (adapted SDK)

The guest API stays almost identical.  The main change is the import
mechanism — Hyperlight uses a different host function calling convention
than Extism, but the `clankers-plugin-sdk` crate abstracts this:

```rust
use clankers_plugin_sdk::*;

#[plugin_export]
pub fn handle_tool_call(input: &str) -> Result<String> {
    let call: ToolCallInput = serde_json::from_str(input)?;
    // execute tool logic — same as before
    Ok(serde_json::to_string(&result)?)
}

#[plugin_export]
pub fn plugin_info() -> Result<String> {
    Ok(serde_json::to_string(&PluginInfo {
        name: "clankers-hash",
        version: "0.1.0",
        tools: vec!["hash"],
    })?)
}
```

### Host functions available to plugins

```rust
/// Host functions exposed to WASM plugins via Hyperlight.
/// Only available if the plugin's permissions allow it.
trait PluginHostFunctions {
    /// Read from KV (scoped to plugin's allowed prefixes)
    fn kv_get(key: &str) -> Option<String>;
    /// Write to KV (scoped to plugin's allowed prefixes)
    fn kv_set(key: &str, value: &str) -> Result<()>;
    /// Delete from KV (scoped to plugin's allowed prefixes)
    fn kv_delete(key: &str) -> Result<()>;
    /// Read a blob by hash
    fn blob_read(hash: &str) -> Option<Vec<u8>>;
    /// Store a blob, returns hash
    fn blob_write(data: &[u8]) -> Result<String>;
    /// Get current timestamp (monotonic, not wall clock)
    fn now_millis() -> u64;
    /// Generate random bytes
    fn random_bytes(len: usize) -> Vec<u8>;
}
```

Current Extism plugins don't use any host functions (they're pure
input → output).  The new host functions are opt-in — existing plugins
work without them.

## Migration Path

### Phase 1: Dual runtime (compatibility)

Both Extism and Hyperlight run side by side.  Existing `plugin.json` with
`"kind": "extism"` continues to work.  New plugins use `"kind": "hyperlight"`.

```rust
enum PluginRuntime {
    Extism(ExtismPlugin),       // legacy
    Hyperlight(HyperlightPlugin), // new
}
```

### Phase 2: SDK migration tool

```bash
clankers plugin migrate ./plugins/clankers-hash/
  → Updates Cargo.toml: extism-pdk → clankers-plugin-sdk
  → Updates guest code: #[plugin_fn] → #[plugin_export]
  → Rebuilds with --target wasm32-wasi
  → Generates Hyperlight manifest
  → Uploads WASM to blob store (cluster) or plugins/ dir (standalone)
```

### Phase 3: Extism removal

After all shipped plugins are migrated, remove the `extism` dependency.

## Hot Reload

Aspen's `HandlerRegistry` uses `ArcSwap` for lock-free handler updates.
When a plugin is updated in the KV store:

1. File watcher (standalone) or KV watch (cluster) detects change
2. New WASM binary loaded from blob store
3. Hyperlight sandbox created with host functions
4. `plugin_info()` called to validate
5. `ArcSwap::store()` atomically swaps the old handler
6. Old sandbox dropped (no in-flight calls affected)

## Permissions Enforcement

In standalone mode, all plugins get full permissions (backwards compatible).
In cluster mode, permissions are enforced per the manifest:

```rust
fn validate_host_call(plugin: &PluginManifest, call: &HostCall) -> Result<()> {
    match call {
        HostCall::KvGet(key) | HostCall::KvSet(key, _) | HostCall::KvDelete(key) => {
            ensure!(plugin.permissions.kv_read || plugin.permissions.kv_write);
            ensure!(plugin.kv_prefixes.iter().any(|p| key.starts_with(p)),
                "key {} outside allowed prefixes", key);
        }
        HostCall::BlobRead(_) => ensure!(plugin.permissions.blob_read),
        HostCall::BlobWrite(_) => ensure!(plugin.permissions.blob_write),
        HostCall::Random(_) => ensure!(plugin.permissions.randomness),
        HostCall::Now => ensure!(plugin.permissions.timers),
    }
    Ok(())
}
```
