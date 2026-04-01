## Context

The standalone interactive mode creates a `PluginManager` at startup, discovers plugins from global (`~/.clankers/plugins/`) and project (`./plugins/`) dirs, loads their WASM, and wires them in three ways:

1. **Tools** — `build_all_tiered_tools` wraps each plugin's `tool_definitions` in `PluginTool`/`ValidatorTool` and adds them at Specialty tier.
2. **Events** — `EventLoopRunner::dispatch_to_plugins` forwards `AgentEvent`s to subscribed plugins after each event, collecting UI actions and messages.
3. **Hooks** — `PluginHookHandler` integrates into the `HookPipeline` so plugins can deny pre-hook operations.

The daemon skips all three. `start_foreground` builds tools via `build_tiered_tools` (no plugin arg). `SessionFactory::build_tools_with_panel_tx` does the same. `build_session_hook_pipeline` explicitly comments "without plugin hooks (plugins are client-side in daemon mode)."

The attach client receives `DaemonEvent` variants and renders them in a TUI, but has no plugin awareness — no `PluginUiState`, no widget rendering, no status segments.

## Goals / Non-Goals

**Goals:**

- Daemon sessions have the same plugin tools available as standalone mode.
- Plugins receive agent events and can return UI actions and deny verdicts, same as standalone.
- Attached TUI clients render plugin widgets, status segments, and notifications.
- `/plugin` and `/tools` slash commands work from attached clients and show plugin state.
- A single `PluginManager` instance is shared across all daemon sessions (plugins are loaded once at daemon startup, not per-session).

**Non-Goals:**

- Per-session plugin configuration (different plugins per session). All sessions share the daemon's plugin set.
- Hot-loading new plugins without daemon restart. `reload_all` works for reloading existing plugins, but discovering new plugin directories requires a restart.
- Client-side plugin loading. Plugins run in the daemon process where they have access to host functions (fs, env). The client is a thin renderer.
- Plugin state isolation between sessions. Extism plugins are stateless between calls (WASM linear memory resets). The `PluginManager` mutex already serializes calls.

## Decisions

### 1. Single shared PluginManager owned by the daemon process

The `PluginManager` is created once in `start_foreground`/`ensure_daemon_running` and threaded into `SessionFactory`. Every session shares it.

**Why not per-session:** Plugins are discovered from fixed directories and their WASM is loaded once. Creating per-session managers would reload WASM N times for N sessions and complicate lifecycle. The existing `Mutex<PluginManager>` design already handles concurrent access.

**Why not client-side:** Plugins need host functions (`read_file`, `write_file`, `get_env`, `list_dir`). These operate on the daemon's filesystem, not the client's. Running plugins client-side would break the permission model and require reimplementing host functions over the wire.

### 2. SessionFactory gains an `Option<Arc<Mutex<PluginManager>>>`

`SessionFactory` gets an optional `plugin_manager` field. `build_tools_with_panel_tx` calls `build_all_tiered_tools` when plugins are present, falling back to `build_tiered_tools` when `None`. This keeps the factory usable in contexts where plugins aren't loaded (tests, child agent factories where `registry: None`).

**Alternative considered:** Making it non-optional. Rejected because child `SessionFactory` instances (created inside `build_tools_with_panel_tx` for subagent actor context) intentionally strip features to avoid recursion. Keeping `Option` matches the existing `registry: Option<ProcessRegistry>` pattern.

### 3. Plugin events dispatched inside the daemon's event broadcast loop

`agent_process.rs` already subscribes to `AgentEvent` via broadcast channel and converts events to `DaemonEvent` for clients. Plugin dispatch hooks into this same path — after converting an event for clients, also dispatch to plugins. UI actions from plugins become new `DaemonEvent::PluginUiAction` variants sent to clients.

**Why not in the controller:** `SessionController` is in `clankers-controller`, which has no dependency on `clankers-plugin`. Adding it would pull Extism into the library crate. Keeping dispatch in the binary crate matches the existing architecture.

### 4. Three new DaemonEvent variants for plugin UI

```rust
DaemonEvent::PluginWidget { plugin: String, widget: Option<serde_json::Value> }
DaemonEvent::PluginStatus { plugin: String, text: Option<String>, color: Option<String> }
DaemonEvent::PluginNotify { plugin: String, message: String, level: String }
```

Serialized as JSON values rather than the `Widget` enum directly, because `clankers-protocol` shouldn't depend on `clankers-tui-types`. The attach client deserializes them back into `Widget`/`StatusSegment`/`PluginNotification`.

**Alternative considered:** A single `PluginUiAction` variant carrying the full `PluginUiAction` enum as `serde_json::Value`. Rejected because three specific variants are clearer in the protocol and let clients handle each case independently.

### 5. PluginHookHandler added to daemon hook pipeline

`build_session_hook_pipeline` registers a `PluginHookHandler` wrapping the shared `PluginManager`. This gives plugins the same deny capability in daemon mode as standalone.

The function signature changes to accept `Option<&Arc<Mutex<PluginManager>>>`.

### 6. Plugin info exposed via ControlCommand/ControlResponse

A new `ControlCommand::ListPlugins` and `ControlResponse::Plugins(Vec<PluginSummary>)` pair. This lets `/plugin` work from attached clients by querying the daemon's control socket rather than needing a local `PluginManager`.

A `SessionCommand::GetPlugins` / `DaemonEvent::PluginList` pair handles per-session queries from the attach client.

## Risks / Trade-offs

- **Mutex contention** — All sessions share one `Mutex<PluginManager>`. Plugin calls are synchronous (Extism is not async). A slow plugin blocks other sessions' plugin calls. → Mitigation: Plugin calls are already wrapped in `spawn_blocking` in the hook handler. Tool calls go through `PluginTool::execute` which also holds the lock briefly. The existing standalone mode has the same contention profile. If this becomes a bottleneck, per-plugin locks could replace the global mutex later.

- **Memory footprint** — Each loaded WASM module consumes memory in the daemon process. With 7 plugins this is negligible (~10-20MB total), but a plugin directory with many large modules could matter for long-running daemons. → Mitigation: `disable`/`enable` already exist. A future `max_plugins` setting could cap loaded modules.

- **Protocol growth** — Three new `DaemonEvent` variants increase the protocol surface. Older clients that don't understand them will hit unknown variant deserialization. → Mitigation: `serde(deny_unknown_fields)` is not used on `DaemonEvent`; unknown variants are skipped by default with externally-tagged enums when using `#[serde(other)]` or lenient deserialization. Alternatively, older clients simply ignore unrecognized events.

- **No plugin UI in non-TUI clients** — Matrix and JSON-mode clients won't render plugin widgets. → Acceptable. These clients already skip TUI-specific features. Plugin tools still work — only the UI rendering is absent.
