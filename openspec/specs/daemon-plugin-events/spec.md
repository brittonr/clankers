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
When an Extism plugin returns JSON UI actions or a stdio plugin emits a `ui { type: "ui", plugin_protocol, actions: [...] }` frame, the daemon SHALL convert those actions to `DaemonEvent` variants and broadcast them to attached clients.

#### Scenario: Stdio plugin sets a widget
- **WHEN** a stdio plugin emits `ui { type: "ui", plugin_protocol: 1, actions: [{"action": "set_widget", "widget": {...}}] }`
- **THEN** the daemon broadcasts a `DaemonEvent::PluginWidget` with the plugin name and serialized widget

#### Scenario: Stdio plugin sets status segment
- **WHEN** a stdio plugin emits `ui { type: "ui", plugin_protocol: 1, actions: [{"action": "set_status", "text": "running", "color": "green"}] }`
- **THEN** the daemon broadcasts `DaemonEvent::PluginStatus` with the text and color

#### Scenario: Stdio plugin sends notification
- **WHEN** a stdio plugin emits `ui { type: "ui", plugin_protocol: 1, actions: [{"action": "notify", "message": "Done!", "level": "info"}] }`
- **THEN** the daemon broadcasts `DaemonEvent::PluginNotify` with the message and level

#### Scenario: Plugin without ui permission has actions stripped
- **WHEN** an Extism or stdio plugin without `ui` permission returns UI actions
- **THEN** the actions are stripped and no plugin UI `DaemonEvent` is sent

---

### Requirement: Attached TUI clients render plugin UI from daemon events
The attach client SHALL maintain a `PluginUiState` and apply incoming plugin UI events, rendering widgets, status segments, and notifications the same way standalone mode does.

#### Scenario: Attach client renders plugin widget panel
- **WHEN** an attached TUI client receives `DaemonEvent::PluginWidget` with a widget tree
- **THEN** the widget is stored in `PluginUiState` and rendered as a plugin panel in the TUI

#### Scenario: Attach client renders plugin status segment
- **WHEN** an attached TUI client receives `DaemonEvent::PluginStatus`
- **THEN** the status bar shows the plugin's segment with the given text and color

#### Scenario: Attach client renders plugin notification
- **WHEN** an attached TUI client receives `DaemonEvent::PluginNotify`
- **THEN** a toast notification appears and auto-expires after 5 seconds

#### Scenario: Attach client clears widget on None
- **WHEN** `DaemonEvent::PluginWidget` arrives with `widget: None`
- **THEN** the plugin's widget panel is removed from the TUI

---

### Requirement: Plugin hook handler active in daemon sessions
The daemon's hook pipeline SHALL include a `PluginHookHandler` so plugins can participate in pre/post hooks and deny operations.

#### Scenario: Plugin denies a tool call via hook
- **WHEN** a plugin's `on_event` response for a pre-tool hook includes `{"deny": true, "reason": "blocked"}`
- **THEN** the tool call is denied with the plugin's reason

#### Scenario: Plugin hook handler absent when no plugins loaded
- **WHEN** the daemon starts with no plugins
- **THEN** no `PluginHookHandler` is registered and the hook pipeline contains only script/git hooks

---

### Requirement: Plugin messages surfaced in daemon sessions
When an Extism plugin returns `{"display": true, "message": "..."}` or a stdio plugin emits `display { type: "display", plugin_protocol: 1, message: "..." }`, the message SHALL be forwarded to attached clients as a `DaemonEvent::SystemMessage`.

#### Scenario: Stdio plugin display message sent to client
- **WHEN** a stdio plugin emits `display { type: "display", plugin_protocol: 1, message: "..." }` during event handling
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
