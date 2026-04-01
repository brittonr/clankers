## Why

WASM plugins work in standalone interactive mode but are completely absent from the daemon. The daemon builds its tool set via `build_tiered_tools` without any `PluginManager`, so plugin-provided tools (calendar, email, GitHub, hash, text-stats, self-validate) are unavailable to daemon sessions. Clients attaching via `clankers attach` or remote iroh QUIC get a degraded experience compared to standalone mode.

## What Changes

- Create and initialize a `PluginManager` during daemon startup, discovering and loading plugins the same way standalone mode does.
- Feed plugin tools into `SessionFactory` so every new daemon session gets them alongside built-in tools.
- Wire `PluginHookHandler` into the daemon's hook pipeline so plugins can participate in pre/post hooks and deny operations.
- Forward plugin UI actions (widgets, status segments, notifications) through the daemon→client event stream so attached TUI clients can render them.
- Dispatch agent events to subscribed plugins inside daemon sessions (agent_start, tool_call, turn_start, etc.).
- Expose plugin state through the daemon control protocol so `/plugin` and `/tools` commands work from attached clients.

## Capabilities

### New Capabilities
- `daemon-plugin-loading`: Plugin discovery, WASM loading, and lifecycle management within the daemon process.
- `daemon-plugin-tools`: Registration of plugin-provided tools into daemon sessions via SessionFactory.
- `daemon-plugin-events`: Dispatching agent events to subscribed plugins and forwarding their responses (UI actions, messages, deny verdicts) through the daemon protocol.

### Modified Capabilities

## Impact

- `src/commands/daemon.rs` — add `PluginManager` creation in `start_foreground` and `ensure_daemon_running`
- `src/modes/daemon/mod.rs` — thread `PluginManager` into `run_daemon` and `SessionFactory`
- `src/modes/daemon/socket_bridge.rs` — `SessionFactory` gains a `plugin_manager` field; `build_tools_with_panel_tx` calls `build_all_tiered_tools` instead of `build_tiered_tools`
- `src/modes/daemon/agent_process.rs` — `build_session_hook_pipeline` registers `PluginHookHandler`; event dispatch calls `dispatch_event_to_plugins`; `DaemonToolRebuilder` includes plugin tools
- `crates/clankers-protocol/` — extend `DaemonEvent` to carry plugin UI actions so clients can render widgets/status/notifications
- `src/modes/attach.rs` — handle incoming plugin UI events and apply them to the TUI's `PluginUiState`
- No new crate dependencies — `clankers-plugin` and `extism` are already in the workspace
