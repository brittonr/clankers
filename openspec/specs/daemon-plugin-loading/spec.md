

### Requirement: Daemon discovers and loads plugins at startup
The daemon process SHALL create a `PluginManager`, discover plugins from global and project directories, and load their WASM modules during daemon initialization, before any sessions are created.

#### Scenario: Plugins discovered from standard directories
- **WHEN** the daemon starts with plugins present in `~/.clankers/plugins/` and `./plugins/`
- **THEN** the daemon's `PluginManager` discovers and loads all valid plugins from both directories

#### Scenario: No plugins present
- **WHEN** the daemon starts with no plugin directories or empty plugin directories
- **THEN** the daemon starts normally with an empty `PluginManager` and no plugin tools

#### Scenario: Plugin with invalid WASM
- **WHEN** a plugin directory contains a valid `plugin.json` but the WASM file fails to load
- **THEN** that plugin's state is set to `Error` and all other plugins load normally

### Requirement: PluginManager is shared across all daemon sessions
The daemon SHALL create a single `Arc<Mutex<PluginManager>>` instance and pass it to `SessionFactory` so all sessions share the same loaded plugins.

#### Scenario: Multiple sessions see same plugins
- **WHEN** two sessions are created after daemon startup
- **THEN** both sessions have identical plugin tools available

#### Scenario: Plugin disabled in one session does not affect others
- **WHEN** a plugin tool is disabled via `SetDisabledTools` in session A
- **THEN** the plugin tool remains available in session B

### Requirement: SessionFactory stores optional PluginManager
`SessionFactory` SHALL have an `Option<Arc<Mutex<PluginManager>>>` field. When `Some`, plugin tools are included in tool builds. When `None`, behavior matches the current plugin-free path.

#### Scenario: Factory with plugins builds plugin tools
- **WHEN** `build_tools_with_panel_tx` is called on a factory with `plugin_manager: Some(...)`
- **THEN** the returned tool list includes plugin-provided tools at Specialty tier alongside built-in tools

#### Scenario: Factory without plugins builds only built-in tools
- **WHEN** `build_tools_with_panel_tx` is called on a factory with `plugin_manager: None`
- **THEN** the returned tool list contains only built-in tools (same as current behavior)

#### Scenario: Child factories for subagent recursion have no plugins
- **WHEN** a child `SessionFactory` is created inside `build_tools_with_panel_tx` for subagent actor context
- **THEN** the child factory's `plugin_manager` is `None` to avoid recursive plugin loading

### Requirement: Plugin state queryable via control protocol
The daemon SHALL respond to a `ListPlugins` control command with the name, version, state, tools, and permissions of each loaded plugin.

#### Scenario: ListPlugins returns loaded plugins
- **WHEN** a client sends `ControlCommand::ListPlugins` to the daemon control socket
- **THEN** the daemon responds with `ControlResponse::Plugins` containing a summary for each discovered plugin

#### Scenario: ListPlugins with no plugins loaded
- **WHEN** a client sends `ListPlugins` and no plugins are loaded
- **THEN** the daemon responds with an empty plugins list
