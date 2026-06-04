## MODIFIED Requirements

### Requirement: Daemon discovers and loads plugins at startup
The daemon process SHALL create a shared plugin host, discover plugins from `~/.clankers/agent/plugins/`, `.clankers/plugins/`, and project-root `plugins/`, then initialize every enabled plugin according to its kind before any sessions are created. Extism plugins SHALL load through the existing WASM path. `kind: "stdio"` plugins SHALL be launched as supervised child processes and SHALL not be marked active until they complete the ready handshake.

#### Scenario: Mixed plugin kinds discovered from standard directories
- **WHEN** the daemon starts with an Extism plugin in `~/.clankers/agent/plugins/` and a `kind: "stdio"` plugin in `./plugins/`
- **THEN** the daemon discovers both plugins during the same startup pass
- **THEN** the Extism plugin is loaded and the stdio plugin is launched under supervision

#### Scenario: No plugins present
- **WHEN** the daemon starts with no plugin directories or empty plugin directories
- **THEN** the daemon starts normally with an empty shared plugin host and no plugin tools

#### Scenario: Plugin launch or load failure does not block daemon startup
- **WHEN** a discovered plugin fails to load its WASM module or a stdio plugin fails command startup or ready handshake
- **THEN** that plugin is marked `error`
- **THEN** all other valid plugins continue initializing and the daemon still starts

---

### Requirement: PluginManager is shared across all daemon sessions
The daemon SHALL create one shared plugin host instance and pass it to `SessionFactory` so all daemon sessions observe the same active plugin runtime state, live tool registrations, and plugin UI/event subscriptions.

#### Scenario: Multiple sessions see same active stdio tools
- **WHEN** two daemon sessions are created after a stdio plugin registers `github_pr_list`
- **THEN** both sessions expose `github_pr_list` as the same plugin-provided tool

#### Scenario: Plugin restart updates all sessions
- **WHEN** a shared stdio plugin crashes, enters backoff, and later re-registers its tools after restart
- **THEN** all daemon sessions see the tool disappear during disconnect and reappear after successful re-registration

#### Scenario: Plugin disabled in one session does not affect others
- **WHEN** a plugin tool is disabled via `SetDisabledTools` in session A
- **THEN** the plugin remains loaded in the shared plugin host
- **THEN** session B still sees the plugin tool unless it is independently disabled there

---

### Requirement: SessionFactory stores optional PluginManager
`SessionFactory` SHALL keep optional access to the shared plugin host. When present, tool building SHALL include active Extism plugin tools and live registered stdio plugin tools. When absent, behavior SHALL match the current plugin-free path.

#### Scenario: Factory with plugins builds mixed plugin tools
- **WHEN** `build_tools_with_panel_tx` is called on a factory with shared plugin host access and an active stdio plugin has live-registered a tool
- **THEN** the returned tool list includes built-in tools, Extism plugin tools, and the stdio plugin tool

#### Scenario: Factory without plugins builds only built-in tools
- **WHEN** `build_tools_with_panel_tx` is called on a factory with no plugin host
- **THEN** the returned tool list contains only built-in tools

#### Scenario: Child factories for subagent recursion have no plugins
- **WHEN** a child `SessionFactory` is created inside `build_tools_with_panel_tx` for subagent actor context
- **THEN** the child factory's plugin host is `None`
- **THEN** recursive child agents do not launch or re-supervise plugins

---

### Requirement: Plugin state queryable via control protocol
The daemon SHALL respond to `ListPlugins` and per-session plugin-list queries with each plugin's name, version, kind, runtime state, current tool inventory, permissions, and last error when present.

#### Scenario: ListPlugins returns live stdio plugin status
- **WHEN** a client sends `ControlCommand::ListPlugins` after a stdio plugin has become active and registered tools
- **THEN** the daemon responds with a plugin summary containing `kind: "stdio"`, runtime state `active`, and the plugin's live tool list

#### Scenario: Backoff or error state is visible
- **WHEN** a stdio plugin is restarting or has exhausted its restart attempts
- **THEN** plugin-list queries show runtime state `backoff` or `error`
- **THEN** the summary includes the most recent launch or handshake error when one exists

#### Scenario: ListPlugins with no plugins loaded
- **WHEN** a client sends `ListPlugins` and no plugins are loaded
- **THEN** the daemon responds with an empty plugins list
