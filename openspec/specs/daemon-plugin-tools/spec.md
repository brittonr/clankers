

### Requirement: Plugin tools registered in daemon sessions
Daemon sessions SHALL include plugin-provided tools in their tool set. Each plugin's `tool_definitions` are wrapped in `PluginTool` (or `ValidatorTool` for exec-permission plugins) and added at Specialty tier.

#### Scenario: Plugin tool callable by LLM in daemon session
- **WHEN** a daemon session is created and a plugin provides a tool (e.g. `hash_text`)
- **THEN** the LLM can call `hash_text` and receives the plugin's response

#### Scenario: Plugin tool appears in ToolList event
- **WHEN** a client attaches to a daemon session with plugins loaded
- **THEN** the `DaemonEvent::ToolList` includes plugin tools with their names and descriptions

#### Scenario: Plugin tool execution wraps params in envelope
- **WHEN** the LLM calls a plugin tool with parameters
- **THEN** the parameters are wrapped in `{"tool": "<name>", "args": {...}}` and passed to the plugin's WASM handler function

#### Scenario: Plugin tool error returns ToolResult error
- **WHEN** a plugin tool call fails (WASM error, unknown tool, plugin panic)
- **THEN** the tool returns a `ToolResult` with `is_error: true` and a descriptive message

### Requirement: DaemonToolRebuilder includes plugin tools
The `DaemonToolRebuilder` used for `SetDisabledTools` SHALL rebuild the tool list including plugin tools, so disabled/enabled state changes apply correctly to both built-in and plugin tools.

#### Scenario: Disabling a plugin tool via SetDisabledTools
- **WHEN** a client sends `SetDisabledTools` with a plugin tool name (e.g. `github_pr_list`)
- **THEN** the rebuilt tool list excludes `github_pr_list` but retains all other plugin and built-in tools

#### Scenario: Re-enabling a plugin tool
- **WHEN** a client sends `SetDisabledTools` with an empty list after previously disabling a plugin tool
- **THEN** the rebuilt tool list includes all plugin tools again

### Requirement: Plugin tools respect capability gates
Plugin tools SHALL be subject to the same UCAN capability gate as built-in tools. A session with restricted capabilities cannot call plugin tools unless the capability set permits them.

#### Scenario: Capability gate blocks plugin tool
- **WHEN** a session has capabilities restricting tools to `["bash", "read"]` and the LLM attempts to call `hash_text`
- **THEN** the call is blocked with a `ToolBlocked` event

#### Scenario: Unrestricted session allows plugin tools
- **WHEN** a session has no capability restrictions (local session, no UCAN)
- **THEN** all plugin tools are callable
