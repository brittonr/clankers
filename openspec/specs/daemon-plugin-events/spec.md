

### Requirement: Agent events dispatched to plugins in daemon sessions
The daemon SHALL forward agent events to subscribed plugins during session execution, matching the standalone mode's `dispatch_event_to_plugins` behavior.

#### Scenario: Plugin receives agent_start event
- **WHEN** a daemon session starts processing a prompt and a plugin subscribes to `agent_start`
- **THEN** the plugin's `on_event` function is called with `{"event": "agent_start", "data": {}}`

#### Scenario: Plugin receives tool_call event
- **WHEN** the LLM calls a tool in a daemon session and a plugin subscribes to `tool_call`
- **THEN** the plugin's `on_event` is called with the tool name and call ID

#### Scenario: Unsubscribed plugin does not receive events
- **WHEN** an agent event fires and a plugin's manifest does not list that event type
- **THEN** the plugin's `on_event` is not called

### Requirement: Plugin UI actions forwarded to attached clients
When a plugin's event handler returns UI actions (`set_widget`, `set_status`, `notify`), the daemon SHALL convert them to `DaemonEvent` variants and broadcast them to attached clients.

#### Scenario: Plugin sets a widget
- **WHEN** a plugin's `on_event` response includes `{"ui": [{"action": "set_widget", "widget": {...}}]}`
- **THEN** the daemon broadcasts a `DaemonEvent::PluginWidget` with the plugin name and serialized widget

#### Scenario: Plugin sets status segment
- **WHEN** a plugin returns `{"ui": [{"action": "set_status", "text": "running", "color": "green"}]}`
- **THEN** the daemon broadcasts `DaemonEvent::PluginStatus` with the text and color

#### Scenario: Plugin sends notification
- **WHEN** a plugin returns `{"ui": [{"action": "notify", "message": "Done!", "level": "info"}]}`
- **THEN** the daemon broadcasts `DaemonEvent::PluginNotify` with the message and level

#### Scenario: Plugin without ui permission has actions stripped
- **WHEN** a plugin without `ui` in its permissions returns UI actions
- **THEN** the actions are stripped by `filter_ui_actions` and no `DaemonEvent` is sent

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

### Requirement: Plugin hook handler active in daemon sessions
The daemon's hook pipeline SHALL include a `PluginHookHandler` so plugins can participate in pre/post hooks and deny operations.

#### Scenario: Plugin denies a tool call via hook
- **WHEN** a plugin's `on_event` response for a pre-tool hook includes `{"deny": true, "reason": "blocked"}`
- **THEN** the tool call is denied with the plugin's reason

#### Scenario: Plugin hook handler absent when no plugins loaded
- **WHEN** the daemon starts with no plugins
- **THEN** no `PluginHookHandler` is registered and the hook pipeline contains only script/git hooks

### Requirement: Plugin messages surfaced in daemon sessions
When a plugin's event handler returns `{"display": true, "message": "..."}`, the message SHALL be forwarded to attached clients as a `DaemonEvent::SystemMessage`.

#### Scenario: Plugin display message sent to client
- **WHEN** a plugin returns a display message during event handling
- **THEN** attached clients receive `DaemonEvent::SystemMessage` with the plugin-prefixed text

### Requirement: Plugin list queryable per-session
Attached clients SHALL be able to query the plugin list via `SessionCommand::GetPlugins`, receiving a `DaemonEvent::PluginList` with plugin summaries.

#### Scenario: GetPlugins returns plugin info
- **WHEN** an attached client sends `SessionCommand::GetPlugins`
- **THEN** the session responds with `DaemonEvent::PluginList` containing name, version, state, tools, and permissions for each plugin

#### Scenario: /plugin slash command works from attached client
- **WHEN** a user types `/plugin` in an attached TUI session
- **THEN** the command queries the daemon and displays the plugin list with states and tools
