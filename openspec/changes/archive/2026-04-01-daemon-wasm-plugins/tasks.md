## 1. SessionFactory plugin support

- [x] 1.1 Add `plugin_manager: Option<Arc<Mutex<PluginManager>>>` field to `SessionFactory` in `src/modes/daemon/socket_bridge.rs`
- [x] 1.2 Update `build_tools_with_panel_tx` to call `build_all_tiered_tools` when `plugin_manager` is `Some`, passing the manager into the `ToolEnv`
- [x] 1.3 Set child `SessionFactory` instances (subagent actor context) to `plugin_manager: None` to avoid recursive loading

## 2. Daemon startup plugin loading

- [x] 2.1 In `src/commands/daemon.rs::start_foreground`, create a `PluginManager`, call `discover()` and `load_wasm` for each plugin, wrap in `Arc<Mutex<_>>`
- [x] 2.2 Pass the `PluginManager` into the `SessionFactory` constructor in `src/modes/daemon/mod.rs::run_daemon`
- [x] 2.3 In `ensure_daemon_running` (auto-daemon path), verify the re-exec'd daemon process picks up plugins from standard directories (no code change needed — it calls `start_foreground`)

## 3. DaemonToolRebuilder plugin support

- [x] 3.1 Update `DaemonToolRebuilder` in `agent_process.rs` to store the `plugin_manager` and use `build_all_tiered_tools` in `rebuild_filtered`
- [x] 3.2 Verify that `SetDisabledTools` correctly filters plugin tools from the rebuilt list

## 4. Plugin hook pipeline

- [x] 4.1 Change `build_session_hook_pipeline` signature to accept `Option<&Arc<Mutex<PluginManager>>>`
- [x] 4.2 When plugin manager is `Some`, create and register a `PluginHookHandler` in the pipeline
- [x] 4.3 Update all call sites of `build_session_hook_pipeline` to pass the plugin manager

## 5. Plugin event dispatch in daemon sessions

- [x] 5.1 Add plugin event dispatch to the agent process event loop in `agent_process.rs` — after broadcasting each `AgentEvent` as a `DaemonEvent`, call `dispatch_event_to_plugins` with the shared `PluginManager`
- [x] 5.2 Convert plugin display messages to `DaemonEvent::SystemMessage` with a `🔌 plugin_name:` prefix and broadcast to clients

## 6. Protocol extensions for plugin UI

- [x] 6.1 Add `PluginWidget { plugin: String, widget: Option<serde_json::Value> }` variant to `DaemonEvent` in `crates/clankers-protocol/src/event.rs`
- [x] 6.2 Add `PluginStatus { plugin: String, text: Option<String>, color: Option<String> }` variant to `DaemonEvent`
- [x] 6.3 Add `PluginNotify { plugin: String, message: String, level: String }` variant to `DaemonEvent`
- [x] 6.4 Add `PluginList { plugins: Vec<PluginSummary> }` variant to `DaemonEvent` with a `PluginSummary` struct (name, version, state, tools, permissions)
- [x] 6.5 Add `GetPlugins` variant to `SessionCommand`
- [x] 6.6 Convert `PluginUiAction` results from dispatch into the new `DaemonEvent` variants and broadcast them in `agent_process.rs`

## 7. Control protocol plugin query

- [x] 7.1 Add `ListPlugins` variant to `ControlCommand` in `crates/clankers-protocol/`
- [x] 7.2 Add `Plugins(Vec<PluginSummary>)` variant to `ControlResponse`
- [x] 7.3 Handle `ListPlugins` in the daemon's control socket handler, reading from the shared `PluginManager`

## 8. Attach client plugin rendering

- [x] 8.1 Add `PluginUiState` to the attach client's `App` state in `src/modes/attach.rs`
- [x] 8.2 Handle `DaemonEvent::PluginWidget` — deserialize widget JSON into `Widget` enum, insert/remove from `PluginUiState`
- [x] 8.3 Handle `DaemonEvent::PluginStatus` — insert/remove `StatusSegment` in `PluginUiState`
- [x] 8.4 Handle `DaemonEvent::PluginNotify` — push `PluginNotification` into `PluginUiState`
- [x] 8.5 Wire `render_plugin_panels`, `plugin_status_spans`, and `render_plugin_notifications` from `widget_host.rs` into the attach client's render path
- [x] 8.6 Handle `DaemonEvent::PluginList` to support `/plugin` and `/tools` slash commands from attached clients
- [x] 8.7 Send `SessionCommand::GetPlugins` when the user runs `/plugin` in an attached session

## 9. Tests

- [x] 9.1 Unit test: `SessionFactory` with `plugin_manager: Some(...)` returns plugin tools from `build_tools_with_panel_tx`
- [x] 9.2 Unit test: `SessionFactory` with `plugin_manager: None` returns only built-in tools
- [x] 9.3 Unit test: `DaemonToolRebuilder` with plugins filters plugin tools when disabled
- [x] 9.4 Protocol round-trip test: new `DaemonEvent` plugin variants serialize/deserialize correctly
- [x] 9.5 Protocol round-trip test: `ControlCommand::ListPlugins` / `ControlResponse::Plugins` serialize/deserialize correctly
