## Context

Clankers currently treats plugins as in-process artifacts. `PluginManager` discovers plugin manifests from the standard plugin directories, loads Extism WASM, and `build_plugin_tools()` wraps manifest `tool_definitions` in `PluginTool` adapters. Event delivery is also in-process: `dispatch_event_to_plugins()` and daemon-side equivalents serialize an event payload, call a WASM function synchronously, then translate returned UI actions or display messages into TUI/daemon events.

That model is good for lightweight UI panels, hook filters, and pure-WASM logic. It is weak for richer integrations:
- native libraries and arbitrary language runtimes do not fit cleanly inside Extism
- long-lived background state is awkward
- every new capability needs a new host function or a larger trusted surface
- a plugin panic or poisoned state still lives inside the clankers process

Clankers already has strong host-side seams that can support process-backed plugins without a rewrite:
- actor/process primitives through `clanker-actor`
- daemon session sharing through `SessionFactory`
- structured attach/control protocol over length-prefixed JSON
- existing plugin discovery, disable/enable flows, and `/plugin` visibility

This change adopts the highest-ROI Tau idea in a narrow first slice: supervised stdio plugins that can register tools and event subscriptions live, while Extism plugins remain supported.

## Goals / Non-Goals

**Goals:**
- Add a `stdio` plugin kind that launches an external process from the plugin manifest.
- Keep one plugin discovery surface and one user-facing plugin model across Extism and stdio plugins.
- Make live runtime registration the source of truth for stdio plugin tools and event subscriptions.
- Supervise stdio plugin processes with visible lifecycle state and automatic cleanup on disconnect.
- Preserve existing plugin UI/message payloads so attach mode and standalone mode keep the same rendering model.
- Add restricted launch policy for stdio plugins: filtered environment, declared working directory, bounded filesystem/network access, fail-closed startup when restrictions cannot be applied.

**Non-Goals:**
- Removing Extism or Zellij plugin support.
- Switching daemon/client transports to CBOR in this change.
- Adding socket-attached or remote process plugins in the first iteration.
- Designing a general middleware/interceptor stack for tool wrapping.
- Reworking the session controller or daemon session protocol beyond plugin-related additions.

## Verification Plan

- Unit tests for `PluginKind::Stdio` manifest parsing, plugin summary serialization, tool-name collision handling, and environment filtering.
- Runtime seam tests that exercise the real stdio framing path: successful handshake, invalid handshake, live tool registration, unregister-on-disconnect, and tool invoke/progress/result/error correlation.
- Mixed-runtime integration tests that prove one host can run Extism and stdio plugins together, and that daemon sessions see the same active stdio registrations through attach/rebuild flows.
- Daemon attach tests that verify plugin list/status output shows kind + runtime state and that stdio plugin UI/display messages reach attached clients.
- Sandbox tests that prove restricted profiles filter environment, bound filesystem access, and fail closed when restrictions cannot be applied.
- Finish-line checks: `cargo nextest run`, `cargo clippy -- -D warnings`, and `nix build .#clankers`.

## Decisions

### 1. Add stdio as the first process-backed plugin kind

`PluginKind` gains `Stdio`. Existing `Extism` and `Zellij` behavior stays intact.

Why:
- stdio is the simplest transport to supervise inside the existing host process
- it aligns with Tau's process-extension idea without requiring a daemon-wide transport rewrite
- the same message protocol can be reused later for local socket or remote attachment if needed

Alternative considered: add both stdio and socket kinds now. Rejected for the first change because it would spread the work across daemon transport, attach, auth/policy, and plugin runtime at once.

### 2. Keep one plugin discovery and status surface

Plugin manifests continue to live in the current global/project plugin directories and continue to participate in the same disable/enable flows. `/plugin`, `ListPlugins`, and `SessionCommand::GetPlugins` stay the canonical visibility path for every plugin kind.

Why:
- users already understand the plugin model
- migration cost stays low for existing Extism plugins
- a mixed deployment (Extism UI plugin + stdio tool plugin) should look like one coherent system

Design consequence:
- plugin summaries need additive fields for runtime kind and runtime state
- plugin state expands from coarse `Loaded/Active/Error/Disabled` to user-visible lifecycle states that cover stdio startup and restart

### 3. Introduce a unified host facade over Extism and stdio backends

The plugin subsystem will expose a single host facade that answers four questions regardless of plugin kind:
- what plugins are discovered and what state are they in?
- what tools are currently available?
- which plugins subscribe to which events?
- how do I invoke a tool or deliver an event to its backing runtime?

Extism remains manifest-driven and in-process. Stdio becomes connection-driven and asynchronous. The unified facade keeps `build_all_tiered_tools()`, plugin list queries, event dispatch, and attach-mode rendering from splitting into separate code paths per plugin kind.

Why:
- the rest of clankers should not care whether a plugin tool came from WASM or a child process
- mixed deployments are expected
- this reduces future churn if another runtime kind is added later

Alternative considered: bolt stdio process management directly into `PluginManager` and keep separate call sites per kind. Rejected because the current `PluginManager` is largely synchronous and metadata-oriented, while stdio supervision needs background tasks, channels, and disconnect cleanup.

### 4. Reuse the existing framed JSON style and existing plugin payload shapes

The stdio plugin protocol uses the same framing style as clankers' current daemon/session transports: a 4-byte big-endian unsigned length prefix followed by one UTF-8 JSON object. Every object carries `type` and `plugin_protocol` fields. Version `1` is the initial protocol in this change. Plugin `stderr` is captured by the host, attached to plugin status/error reporting, and written to tracing/log output for handshake and launch debugging; it is never treated as framed protocol data.

Why:
- faster implementation and easier debugging than introducing CBOR plus a new schema in the same change
- lets process plugins share event payload examples with Extism plugins
- makes it easy to write minimal reference plugins in shell, Python, Rust, or Node

Frame contract in this change:
- host -> plugin:
  - `hello { type, plugin_protocol, plugin, cwd, mode }`
  - `event { type, plugin_protocol, event: { name, data } }`
  - `tool_invoke { type, plugin_protocol, call_id, tool, args }`
  - `tool_cancel { type, plugin_protocol, call_id, reason }`
  - `shutdown { type, plugin_protocol, reason }`
- plugin -> host:
  - `hello { type, plugin_protocol, plugin, version }`
  - `ready { type, plugin_protocol }`
  - `register_tools { type, plugin_protocol, tools: [{ name, description, input_schema }] }`
  - `unregister_tools { type, plugin_protocol, tools: [name, ...] }`
  - `subscribe_events { type, plugin_protocol, events: [name, ...] }`
  - `tool_progress { type, plugin_protocol, call_id, message }`
  - `tool_result { type, plugin_protocol, call_id, content }`
  - `tool_error { type, plugin_protocol, call_id, message }`
  - `tool_cancelled { type, plugin_protocol, call_id }`
  - `ui { type, plugin_protocol, actions: [...] }`
  - `display { type, plugin_protocol, message }`

The JSON field names above are part of the contract, not an implementation detail.

### 5. Live registration is the source of truth for stdio tools and subscriptions

For stdio plugins, the manifest declares how to launch the plugin and what permissions/sandbox policy it wants. It does not own the active tool inventory. The plugin process must register tools and event subscriptions after startup.

Why:
- tool inventory can depend on runtime checks, account state, or configuration
- disconnect cleanup becomes deterministic: if the connection is gone, registrations are gone
- this matches Tau's strongest extensibility idea and avoids stale manifest metadata becoming authoritative

Rules:
- a stdio tool becomes callable only after registration
- disconnect or explicit unregister removes the tool immediately
- tool name collisions are rejected per conflicting tool; the first active tool keeps ownership and the later conflicting registration is rejected
- `SetDisabledTools` and capability gating apply to registered stdio tools the same way they apply to built-in and Extism tools

### 6. Supervise stdio plugins with bounded restart and visible state

Enabled stdio plugins launch eagerly during plugin initialization in standalone and daemon modes, just like Extism plugins load eagerly today. Unexpected exit enters `backoff` and triggers restart with exponential delays. Manual disable and normal host shutdown do not restart the plugin.

Chosen policy:
- backoff sequence: 1s, 2s, 4s, 8s, 16s
- after 5 consecutive failed launches or crash loops without a successful `ready`, mark the plugin `error`
- a successful `ready` resets the failure counter
- standalone mode and daemon mode use the same lifecycle policy and the same live registry semantics

Why:
- deterministic enough for tests and user-visible status
- robust enough for transient failures
- avoids an infinite hot crash loop flooding logs and UI

### 7. Tool calls are cancellable and time-bounded

Stdio plugin tool calls use the existing turn cancellation model plus one plugin-specific addition: the host sends `tool_cancel` when the turn is cancelled or interrupted. If the plugin does not complete cancellation within 5 seconds, the host fails the call and may tear down the plugin connection. Independent of cancellation, any stdio tool call that does not produce a terminal `tool_result`, `tool_error`, or `tool_cancelled` within 300 seconds fails with a host-generated timeout error.

Why:
- process-backed tools have no natural execution bound like Extism fuel limits
- agent cancellation should not leave orphaned work running forever
- deterministic timeout values make seam tests possible

### 8. Restricted stdio plugins run under explicit launch policy

Stdio plugin manifests gain launch policy metadata:
- command + args
- optional working-directory mode
- optional environment allowlist
- sandbox mode (`inherit` or `restricted`)
- declared writable roots and network allowance for restricted mode

Rules:
- `inherit` is explicit and means "run like current clankers child processes"
- `restricted` means filtered environment plus host-enforced filesystem/network limits
- allowlisted environment variables are required launch inputs in this first change; if any allowlisted variable is absent, the plugin does not start and enters `error`
- optional environment forwarding is intentionally out of scope for v1; plugin authors that want optional variables must not declare them in the allowlist yet
- if a plugin requests `restricted` and the host cannot apply those restrictions, the plugin must not start
- restricted plugins get a dedicated writable state directory under the clankers config tree in addition to any explicitly declared project root access
- logical plugin permissions (`ui`, `exec`, `net`, etc.) remain separate from OS sandboxing; both must allow the action

Why:
- process plugins only help security if the host can bound them
- fail-open sandbox behavior would silently negate the whole value proposition

### 9. Keep current client UX, but enrich plugin status

Attach mode and standalone mode keep the existing plugin widget/status/notify message flow. The user-visible changes are:
- plugin list/status shows runtime kind and lifecycle state
- tool inventory for stdio plugins is live, not static
- restart/backoff/error transitions are visible instead of silent
- standalone interactive mode and daemon attach mode show the same plugin state model

Why:
- preserves existing TUI mental model
- gives enough operational feedback to trust supervised plugins
- keeps process-backed plugins from feeling like hidden subprocesses

## Risks / Trade-offs

- **More moving parts in plugin runtime** -> Contain async process supervision behind one host facade instead of leaking channels and child-process state through the rest of the codebase.
- **Live registration can make tool inventory unstable** -> Treat connection state as authoritative, update tool lists atomically, and reject collisions deterministically.
- **Restricted sandboxing is platform-sensitive** -> Keep the first restricted backend narrow and fail closed when unavailable instead of pretending a plugin is sandboxed.
- **Extism and stdio parity may drift** -> Reuse one plugin summary surface, one event payload shape, and one plugin list UX; add mixed-runtime integration tests.
- **Eager startup may cost more at launch** -> Limit first iteration to stdio plugins declared by the user and keep remote/socket transport out of scope.

## Migration Plan

1. Extend the manifest schema and plugin summary types with additive fields for stdio launch metadata, runtime kind, and runtime state.
2. Introduce the stdio runtime host and live registry behind the unified plugin host facade while keeping existing Extism call sites working.
3. Switch daemon and standalone plugin initialization to start/load mixed plugin kinds from the same discovery pass.
4. Change tool building and daemon rebuild flows to consume live stdio registrations in addition to Extism manifest tools.
5. Add `/plugin` and attached-client status updates for runtime kind/state and verify mixed-runtime behavior.
6. Land docs and example manifests for `kind: stdio` plus sandbox guidance.

Rollback:
- disable or remove stdio plugin manifests; Extism plugins continue to work unchanged
- no persisted session or auth migration is required

## Open Questions

None blocking this first change. Socket-attached plugins and CBOR framing remain intentional follow-up work, not unresolved scope inside this change.
