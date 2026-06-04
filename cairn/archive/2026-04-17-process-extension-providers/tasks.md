## 1. Manifest and shared plugin host model

- [x] 1.1 Extend plugin manifest parsing and validation with `kind: stdio`, launch command/args, working-directory mode, environment allowlist, sandbox mode, declared writable roots, and any new plugin summary fields needed for runtime status, with unit coverage for manifest parsing/validation
- [x] 1.2 Introduce the unified plugin host facade interface and route existing Extism behavior through it without changing current Extism discovery, tool inventory, or event/UI behavior
- [x] 1.3 Add an explicit Extism regression checkpoint for the new facade: existing Extism plugin tests and mixed built-in/Extism tool flows still pass before stdio backend work lands
- [x] 1.4 Extend plugin summary/control-protocol payloads so `/plugin`, `ControlCommand::ListPlugins`, and `SessionCommand::GetPlugins` can show plugin kind, runtime state, live tool inventory, permissions, and last error text, with plugin-summary serialization coverage and runtime verification for `starting`, `active`, `backoff`, `error`, `disabled`, and empty `ListPlugins` responses
- [x] 1.5 Add an explicit Zellij regression checkpoint so the unified host/runtime changes do not break existing Zellij discovery or UX behavior
- [x] 1.6 Add verification that child `SessionFactory` instances created for subagent/delegate recursion keep `plugin_manager = None` so recursive child agents stay plugin-free
- [x] 1.7 Add multi-session daemon verification for the shared plugin host: multiple sessions see the same live stdio tools, restart/disconnect updates propagate to every session, and per-session disabled tools stay session-local

## 2. Stdio runtime and supervision

- [x] 2.1 Implement the exact framed stdio protocol contract: `u32` big-endian length prefix plus JSON envelopes with `type` and `plugin_protocol`, including `hello`, `ready`, `register_tools`, `unregister_tools`, `subscribe_events`, `tool_invoke`, `tool_cancel`, `shutdown`, `tool_progress`, `tool_result`, `tool_error`, `tool_cancelled`, `ui`, and `display`
- [x] 2.2 Implement stdio plugin launcher tasks for standalone and daemon startup paths so enabled stdio plugins start during plugin initialization in both modes
- [x] 2.3 Implement runtime state tracking (`starting`, `active`, `backoff`, `error`, `disabled`) and the fixed restart policy (`1s`, `2s`, `4s`, `8s`, `16s`, then `error` after 5 failed starts without `ready`)
- [x] 2.4 Ensure disconnect, crash, manual disable, and host shutdown clean up child processes, send `shutdown` on normal teardown, capture plugin stderr for diagnostics, and remove connection-scoped registrations deterministically
- [x] 2.5 Add real runtime seam tests that exercise the actual stdio framing path for successful handshake, invalid handshake (malformed JSON, invalid length prefix, mismatched protocol version, and out-of-order startup frames), standalone startup, crash/restart, orderly shutdown, unregister-on-disconnect behavior, and stderr-backed launch diagnostics
- [x] 2.6 Add daemon startup resilience verification for empty plugin directories and mixed Extism/stdio startup failures so plugin problems do not block daemon startup
- [x] 2.7 Add reload verification for stdio plugins that reached `error`, proving a user-triggered reload retries startup and can restore the plugin to `active`
- [x] 2.8 Add bounded shutdown grace-period enforcement and forced-termination coverage for unresponsive stdio plugins
- [x] 2.9 Add explicit verification that manual disable and normal host shutdown send `shutdown` and do not schedule stdio plugin restart
- [x] 2.10 Add explicit verification that a successful `ready` resets the consecutive-failure counter before later restart failures are counted toward `error`
- [x] 2.11 Add explicit verification that re-enabling a disabled stdio plugin relaunches it, restores `active`, and re-registers its live tools

## 3. Live tool registration and invocation

- [x] 3.1 Implement connection-scoped live tool registration/unregistration for stdio plugins, including deterministic rejection of colliding tool names and verification that multiple `register_tools` frames are additive
- [x] 3.2 Add a process-backed tool adapter that sends tool invocations over stdio, maps progress/result/error/cancelled frames back into `ToolResult` behavior, enforces the 300-second timeout, and applies the 5-second cancel-ack bound with a host-generated cancelled result (and any connection-drop handling chosen for that case)
- [x] 3.3 Update plugin tool discovery and `ToolList` generation so active stdio tools appear alongside built-in and Extism plugin tools with correct source labels
- [x] 3.4 Update `DaemonToolRebuilder` / disabled-tools handling so live stdio tools participate in disable, re-enable, and post-restart rebuild flows
- [x] 3.5 Keep capability-gate enforcement identical for Extism and stdio plugin tools, with regression coverage for blocked and allowed stdio tool calls

## 4. Event delivery, UI actions, and attach visibility

- [x] 4.1 Implement live stdio event subscription updates and deliver existing plugin event payloads (`{"event": ..., "data": ...}`) only to subscribed stdio plugins, with verification that restart clears subscriptions until the new connection resubscribes
- [x] 4.2 Map stdio plugin UI actions and display messages onto the existing `DaemonEvent::PluginWidget`, `PluginStatus`, `PluginNotify`, and `SystemMessage` flows, including stripping UI actions when the plugin lacks `ui` permission
- [x] 4.3 Map stdio plugin UI actions and display messages through the standalone interactive-mode plugin/TUI flows so non-daemon sessions preserve the same rendering model, with standalone-path regression coverage
- [x] 4.4 Update standalone and attached-client plugin list/status views so runtime kind, lifecycle state (`starting`, `active`, `backoff`, `error`, `disabled`), live tool inventory, and current error text are visible, with explicit standalone `/plugin` list/detail regression coverage
- [x] 4.5 Add daemon attach tests proving subscribed/unsubscribed stdio events, plugin UI actions, permission-based UI stripping, display messages, and `/plugin` query output all work through the real daemon session path

## 5. Sandbox and launch policy

- [x] 5.1 Apply the non-sandbox launch-policy pieces at runtime for stdio plugins: command, args, working-directory mode, and required environment allowlist, with restricted-mode-specific writable-root/state-dir semantics left to 5.2/5.3
- [x] 5.2 Implement explicit `inherit` and `restricted` sandbox modes, including filtered environment, the exact host-required runtime-variable exception set (empty in v1), bounded writable roots, network allowance checks that require both logical `net` permission and sandbox `allow_network`, and dedicated plugin state directory handling
- [x] 5.3 Fail closed when a plugin requests `restricted` mode and the host cannot apply it, and surface that failure through plugin runtime state and error reporting
- [x] 5.4 Add sandbox tests for explicit `inherit`-mode launch behavior, environment filtering, the exact host-required runtime-variable exception set (empty in v1), missing required environment variables, bounded writes, denied network access unless both logical `net` permission and sandbox `allow_network` permit it, and restricted-mode startup refusal on unsupported hosts

## 6. Documentation and finish-line validation

- [x] 6.1 Add developer documentation and example manifests for `kind: stdio`, including protocol expectations, launch policy fields, sandbox modes, and migration guidance from manifest-only Extism tools
- [x] 6.2 Add or update integration fixtures/examples so plugin authors have at least one minimal reference stdio plugin exercised by tests, and ensure one fixture covers handshake, registration, invocation, cancellation, and shutdown end-to-end
- [x] 6.3 Add a mixed-runtime integration pass that specifically proves Extism + stdio plugins can coexist in one host, preserve Extism behavior, reject cross-kind tool-name collisions deterministically, and surface correct live tool and event visibility
- [x] 6.4 Run `cargo nextest run`
- [x] 6.5 Run `cargo clippy -- -D warnings`
- [x] 6.6 Run `nix build .#clankers`

Verification evidence (2026-04-18 final rerun):
- `cargo nextest run` — passed (`1068 tests run: 1068 passed, 0 skipped`, 308.821s)
- `cargo clippy -- -D warnings` — passed
- `nix build .#clankers` — passed (`result -> /nix/store/wws940qn7ww587ddhx5np5ns37nvylp0-rust_clankers-0.1.0`)
