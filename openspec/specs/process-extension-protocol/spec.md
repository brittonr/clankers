## ADDED Requirements

### Requirement: Framed stdio plugin protocol
The host and a stdio plugin SHALL communicate over frames encoded as `4-byte big-endian unsigned length` plus `UTF-8 JSON object`. Every JSON frame SHALL contain `type` and `plugin_protocol`. Protocol version `1` is required in the first change. Required frame shapes are:
- host -> plugin:
  - `hello { type: "hello", plugin_protocol: 1, plugin: <name>, cwd: <path>, mode: <"standalone"|"daemon"> }`
  - `event { type: "event", plugin_protocol: 1, event: { name: <event-name>, data: <json> } }`
  - `tool_invoke { type: "tool_invoke", plugin_protocol: 1, call_id: <id>, tool: <name>, args: <json> }`
  - `tool_cancel { type: "tool_cancel", plugin_protocol: 1, call_id: <id>, reason: <string> }`
  - `shutdown { type: "shutdown", plugin_protocol: 1, reason: <string> }`
- plugin -> host:
  - `hello { type: "hello", plugin_protocol: 1, plugin: <name>, version: <version> }`
  - `ready { type: "ready", plugin_protocol: 1 }`
  - `register_tools { type: "register_tools", plugin_protocol: 1, tools: [{ name, description, input_schema }] }`
  - `unregister_tools { type: "unregister_tools", plugin_protocol: 1, tools: [<name>, ...] }`
  - `subscribe_events { type: "subscribe_events", plugin_protocol: 1, events: [<event-name>, ...] }`
  - `tool_progress { type: "tool_progress", plugin_protocol: 1, call_id: <id>, message: <string> }`
  - `tool_result { type: "tool_result", plugin_protocol: 1, call_id: <id>, content: <json> }`
  - `tool_error { type: "tool_error", plugin_protocol: 1, call_id: <id>, message: <string> }`
  - `tool_cancelled { type: "tool_cancelled", plugin_protocol: 1, call_id: <id> }`
  - `ui { type: "ui", plugin_protocol: 1, actions: <json-array> }`
  - `display { type: "display", plugin_protocol: 1, message: <string> }`
The protocol SHALL support startup handshake, ready notification, live tool registration/unregistration, event subscription updates, tool invocation, tool progress, tool result, tool error, UI actions, display messages, cancellation, and orderly shutdown.

#### Scenario: Successful handshake
- **WHEN** the host launches a stdio plugin and the plugin sends `hello` followed by `ready` with `plugin_protocol: 1`
- **THEN** the host keeps the connection open
- **THEN** the plugin may register tools and subscriptions on that connection

#### Scenario: Invalid handshake rejected
- **WHEN** a stdio plugin sends malformed JSON, an invalid length frame, a mismatched `plugin_protocol`, or out-of-order startup frames
- **THEN** the host closes the connection
- **THEN** the plugin enters restart or error handling according to lifecycle policy

---

### Requirement: Connection-scoped tool registration
Tool registrations from a stdio plugin SHALL be scoped to the live process connection. `register_tools` is additive for the current connection. `unregister_tools` removes only the named tools from the current connection. A tool SHALL become callable only after successful registration, and SHALL be removed immediately on unregister, disconnect, or plugin restart.

#### Scenario: Tool appears after registration
- **WHEN** an active stdio plugin sends a valid tool registration for `github_pr_list`
- **THEN** `github_pr_list` is added to the active tool inventory
- **THEN** daemon and standalone tool lists show it as a plugin-provided tool

#### Scenario: Tool removed on disconnect
- **WHEN** a stdio plugin disconnects or is restarted
- **THEN** all tools previously registered on that connection are removed from the active tool inventory
- **THEN** later turns cannot invoke those tools until the plugin registers them again

#### Scenario: Multiple register_tools calls are additive
- **WHEN** a stdio plugin first registers `github_pr_list` and later sends another `register_tools` frame for `github_pr_comment`
- **THEN** both tools remain active on that connection until one is explicitly unregistered or the connection ends

#### Scenario: Conflicting tool name rejected
- **WHEN** a stdio plugin registers a tool whose name already belongs to a built-in tool or another active plugin tool
- **THEN** the host rejects that tool registration
- **THEN** non-conflicting tool registrations from the same plugin remain allowed

---

### Requirement: Correlated tool invocation and completion
When the agent invokes a stdio plugin tool, the host SHALL send the plugin a `tool_invoke` frame containing the tool name, call identifier, and JSON arguments. The plugin SHALL answer using the same call identifier in progress, result, error, or cancelled frames.

#### Scenario: Successful tool call
- **WHEN** the agent invokes a stdio plugin tool with JSON arguments
- **THEN** the host sends one `tool_invoke` frame containing the tool name, call ID, and arguments
- **THEN** the plugin may return `tool_progress` frames followed by one final `tool_result` frame for that call ID

#### Scenario: Tool call fails
- **WHEN** the plugin cannot complete an invoked tool call
- **THEN** it sends a `tool_error` frame referencing the original call ID
- **THEN** the host surfaces that failure as an error `ToolResult`

---

### Requirement: Tool invocation cancellation and timeout
The host SHALL make stdio plugin tool calls cancellable and time-bounded. On session cancel or interrupt, the host SHALL send `tool_cancel` for each in-flight stdio plugin tool call. If a call does not end in `tool_result`, `tool_error`, or `tool_cancelled` within 300 seconds of `tool_invoke`, the host SHALL fail the call with a timeout error. If a plugin does not acknowledge cancellation within 5 seconds, the host SHALL fail the call as cancelled and MAY drop the plugin connection.

#### Scenario: Cancelled tool call
- **WHEN** a user cancels or interrupts a turn while a stdio plugin tool call is in flight
- **THEN** the host sends `tool_cancel` with the original call ID
- **THEN** the host eventually surfaces the call as cancelled even if the plugin does not finish the work normally

#### Scenario: Hung tool call times out
- **WHEN** a stdio plugin receives `tool_invoke` but never returns a terminal frame for that call ID
- **THEN** the host fails the call after 300 seconds with a timeout error

---

### Requirement: Connection-scoped event subscriptions
A stdio plugin SHALL declare the event kinds it wants to receive over the live connection. The host SHALL deliver only subscribed event kinds and SHALL remove those subscriptions when the connection ends.

#### Scenario: Subscribed event delivered
- **WHEN** a stdio plugin subscribes to `tool_call`
- **THEN** the host forwards matching `tool_call` events to that plugin while the connection remains active

#### Scenario: Unsubscribed event omitted
- **WHEN** a stdio plugin has not subscribed to `message_update`
- **THEN** the host does not forward `message_update` events to that plugin

#### Scenario: Subscriptions removed on restart
- **WHEN** a stdio plugin restarts after disconnect
- **THEN** it receives no events until it sends a fresh subscription update on the new connection

---

### Requirement: Orderly shutdown
The host SHALL send `shutdown` before stopping an active stdio plugin for normal host shutdown or manual disable. A plugin SHALL be allowed to exit cleanly after receiving `shutdown`, but the host MAY terminate it after a bounded grace period if it does not exit.

#### Scenario: Host shutdown sends shutdown frame
- **WHEN** clankers shuts down normally while a stdio plugin is active
- **THEN** the host sends `shutdown` to that plugin before closing the process

#### Scenario: Manual disable sends shutdown frame
- **WHEN** a user disables an active stdio plugin
- **THEN** the host sends `shutdown`
- **THEN** the plugin's tools and subscriptions are removed from the live registry as the process exits
