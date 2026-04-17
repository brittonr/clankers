## MODIFIED Requirements

### Requirement: Agent events dispatched to plugins in daemon sessions
The daemon SHALL forward plugin events to both active Extism plugins and active stdio plugins during session execution. Extism plugin subscriptions continue to come from the manifest. Stdio plugin subscriptions SHALL come from the live stdio connection. The forwarded payload shape SHALL remain `{"event": <name>, "data": {...}}`.

#### Scenario: Stdio plugin receives subscribed agent_start event
- **WHEN** a daemon session starts processing a prompt and an active stdio plugin subscribes to `agent_start`
- **THEN** the plugin receives `{"event": "agent_start", "data": {}}` over the stdio protocol

#### Scenario: Stdio plugin receives tool_call event
- **WHEN** the LLM calls a tool in a daemon session and an active stdio plugin subscribes to `tool_call`
- **THEN** the plugin receives the tool name and call ID in the forwarded event payload

#### Scenario: Unsubscribed or disconnected stdio plugin does not receive events
- **WHEN** an agent event fires after a stdio plugin has not subscribed to that event or its connection has ended
- **THEN** the daemon does not forward that event to that plugin

---

### Requirement: Plugin UI actions forwarded to attached clients
When an Extism or stdio plugin returns UI actions (`set_widget`, `set_status`, `notify`), the daemon SHALL convert them to `DaemonEvent` variants and broadcast them to attached clients.

#### Scenario: Stdio plugin sets a widget
- **WHEN** a stdio plugin response includes `{"ui": [{"action": "set_widget", "widget": {...}}]}`
- **THEN** the daemon broadcasts a `DaemonEvent::PluginWidget` with the plugin name and serialized widget

#### Scenario: Stdio plugin sets status segment
- **WHEN** a stdio plugin response includes `{"ui": [{"action": "set_status", "text": "running", "color": "green"}]}`
- **THEN** the daemon broadcasts `DaemonEvent::PluginStatus` with the text and color

#### Scenario: Stdio plugin sends notification
- **WHEN** a stdio plugin response includes `{"ui": [{"action": "notify", "message": "Done!", "level": "info"}]}`
- **THEN** the daemon broadcasts `DaemonEvent::PluginNotify` with the message and level

#### Scenario: Plugin without ui permission has actions stripped
- **WHEN** an Extism or stdio plugin without `ui` permission returns UI actions
- **THEN** the actions are stripped and no plugin UI `DaemonEvent` is sent

---

### Requirement: Plugin messages surfaced in daemon sessions
When an Extism or stdio plugin returns `{"display": true, "message": "..."}`, the message SHALL be forwarded to attached clients as a `DaemonEvent::SystemMessage`.

#### Scenario: Stdio plugin display message sent to client
- **WHEN** a stdio plugin returns a display message during event handling
- **THEN** attached clients receive `DaemonEvent::SystemMessage` with the plugin-prefixed text

---

### Requirement: Plugin list queryable per-session
Attached clients SHALL be able to query the plugin list via `SessionCommand::GetPlugins`, receiving a `DaemonEvent::PluginList` with each plugin's name, version, kind, runtime state, current tools, permissions, and last error when present.

#### Scenario: GetPlugins returns stdio plugin runtime info
- **WHEN** an attached client sends `SessionCommand::GetPlugins` after a stdio plugin has connected
- **THEN** the session responds with `DaemonEvent::PluginList` showing that plugin's kind, runtime state, and current tools

#### Scenario: /plugin slash command works from attached client
- **WHEN** a user types `/plugin` in an attached TUI session
- **THEN** the command queries the daemon and displays plugin kinds, runtime states, tools, and any current error text
