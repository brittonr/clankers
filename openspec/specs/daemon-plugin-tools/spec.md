## MODIFIED Requirements

### Requirement: Plugin tools registered in daemon sessions
Daemon sessions SHALL include both static Extism plugin tools and live tools registered by active stdio plugins. Static Extism plugin tools SHALL continue to be wrapped in `PluginTool` (or `ValidatorTool` for exec-permission plugins), stdio tools SHALL be exposed through the process-backed plugin tool adapter, and plugin-provided tools SHALL appear at Specialty tier. A stdio plugin tool SHALL become callable only after registration, SHALL appear in `ToolList` output while the plugin connection is active, and SHALL be removed automatically on unregister, disconnect, or restart.

#### Scenario: Stdio plugin tool callable by LLM in daemon session
- **WHEN** a daemon session is created and an active stdio plugin has registered `github_pr_list`
- **THEN** the LLM can call `github_pr_list`
- **THEN** the plugin receives the invocation over the stdio protocol and the session receives its response

#### Scenario: Stdio plugin tool appears in ToolList event
- **WHEN** a client attaches to a daemon session after a stdio plugin registers a tool
- **THEN** the `DaemonEvent::ToolList` includes that tool with its registered name, description, and plugin source

#### Scenario: Extism plugin tool execution wraps params in envelope
- **WHEN** the LLM calls an Extism plugin tool with parameters
- **THEN** the parameters are wrapped in `{"tool": "<name>", "args": {...}}` and passed to the plugin's WASM handler function

#### Scenario: Plugin tool error returns ToolResult error
- **WHEN** an Extism or stdio plugin tool call fails because of a WASM error, a tool error frame, disconnect, unknown tool, or plugin panic
- **THEN** the tool returns a `ToolResult` with `is_error: true` and a descriptive message

#### Scenario: Stdio plugin tool removed on disconnect
- **WHEN** the stdio plugin disconnects or restarts
- **THEN** the registered tool is removed from the session tool inventory
- **THEN** later turns cannot call it until the plugin registers it again

#### Scenario: Conflicting tool name rejected
- **WHEN** a stdio plugin registers a tool name already used by a built-in or active plugin tool
- **THEN** the host rejects that tool registration
- **THEN** the existing tool remains unchanged in the session tool inventory

#### Scenario: Extism tool keeps ownership against later stdio collision
- **WHEN** an Extism plugin already provides `hash_text` and a stdio plugin later registers `hash_text`
- **THEN** the stdio registration is rejected for that tool name
- **THEN** the Extism plugin tool remains the active `hash_text` implementation

---

### Requirement: DaemonToolRebuilder includes plugin tools
The `DaemonToolRebuilder` used for `SetDisabledTools` SHALL rebuild the tool list from built-in tools, Extism plugin tools, and the current live stdio tool registry so disabled/enabled state changes apply correctly to all tool sources.

#### Scenario: Disabling a stdio plugin tool via SetDisabledTools
- **WHEN** a client sends `SetDisabledTools` with a live stdio plugin tool name such as `github_pr_list`
- **THEN** the rebuilt tool list excludes `github_pr_list`
- **THEN** all other built-in and non-disabled plugin tools remain available

#### Scenario: Re-enabling a stdio plugin tool
- **WHEN** a client clears `SetDisabledTools` after previously disabling a live stdio plugin tool while that plugin is still connected
- **THEN** the rebuilt tool list includes the plugin tool again

#### Scenario: Disabled stdio tool stays hidden across plugin restart
- **WHEN** a stdio plugin restarts and re-registers a tool that is still in the session's disabled tool set
- **THEN** the rebuilt tool list keeps that tool hidden until the disabled tool set is changed

---

### Requirement: Plugin tools respect capability gates
Plugin tools from both Extism and stdio plugins SHALL be subject to the same UCAN capability gate as built-in tools. A session with restricted capabilities cannot call a plugin tool unless the capability set permits it.

#### Scenario: Capability gate blocks stdio plugin tool
- **WHEN** a session has capabilities restricting tools to `['bash', 'read']` and the LLM attempts to call a stdio plugin tool such as `github_pr_list`
- **THEN** the call is blocked with a `ToolBlocked` event

#### Scenario: Unrestricted session allows plugin tools
- **WHEN** a session has no capability restrictions
- **THEN** all active Extism and stdio plugin tools are callable
