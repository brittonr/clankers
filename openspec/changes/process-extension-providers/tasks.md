## 1. Manifest and shared plugin host model

- [x] 1.1 Extend plugin manifest parsing and validation with `kind: stdio`, launch command/args, working-directory mode, environment allowlist, sandbox mode, declared writable roots, and any new plugin summary fields needed for runtime status
- [x] 1.2 Introduce the unified plugin host facade interface and route existing Extism behavior through it without changing current Extism discovery, tool inventory, or event/UI behavior
- [x] 1.3 Add an explicit Extism regression checkpoint for the new facade: existing Extism plugin tests and mixed built-in/Extism tool flows still pass before stdio backend work lands
- [x] 1.4 Extend plugin summary/control-protocol payloads so `/plugin`, `ListPlugins`, and `SessionCommand::GetPlugins` can show plugin kind, runtime state, live tool inventory, and last error text

## 2. Stdio runtime and supervision

- [x] 2.1 Implement the exact framed stdio protocol contract: `u32` big-endian length prefix plus JSON envelopes with `type` and `plugin_protocol`, including `hello`, `ready`, `register_tools`, `unregister_tools`, `subscribe_events`, `tool_invoke`, `tool_cancel`, `shutdown`, `tool_progress`, `tool_result`, `tool_error`, `tool_cancelled`, `ui`, and `display`
- [x] 2.2 Implement stdio plugin launcher tasks for standalone and daemon startup paths so enabled stdio plugins start during plugin initialization in both modes
- [x] 2.3 Implement runtime state tracking (`starting`, `active`, `backoff`, `error`, `disabled`) and the fixed restart policy (`1s`, `2s`, `4s`, `8s`, `16s`, then `error` after 5 failed starts without `ready`)
- [x] 2.4 Ensure disconnect, crash, manual disable, and host shutdown clean up child processes, send `shutdown` on normal teardown, capture plugin stderr for diagnostics, and remove connection-scoped registrations deterministically
- [x] 2.5 Add real runtime seam tests that exercise the actual stdio framing path for successful handshake, invalid handshake, standalone startup, crash/restart, orderly shutdown, unregister-on-disconnect behavior, and stderr-backed launch diagnostics

## 3. Live tool registration and invocation

- [ ] 3.1 Implement connection-scoped live tool registration/unregistration for stdio plugins, including deterministic rejection of colliding tool names
- [ ] 3.2 Add a process-backed tool adapter that sends tool invocations over stdio, maps progress/result/error/cancelled frames back into `ToolResult` behavior, and enforces the 300-second timeout plus cancel/interrupt-driven `tool_cancel` flow
- [ ] 3.3 Update plugin tool discovery and `ToolList` generation so active stdio tools appear alongside built-in and Extism plugin tools with correct source labels
- [ ] 3.4 Update `DaemonToolRebuilder` / disabled-tools handling so live stdio tools participate in disable, re-enable, and post-restart rebuild flows
- [ ] 3.5 Keep capability-gate enforcement identical for Extism and stdio plugin tools, with regression coverage for blocked and allowed stdio tool calls

## 4. Event delivery, UI actions, and attach visibility

- [ ] 4.1 Implement live stdio event subscription updates and deliver existing plugin event payloads (`{"event": ..., "data": ...}`) only to subscribed stdio plugins
- [ ] 4.2 Map stdio plugin UI actions and display messages onto the existing `DaemonEvent::PluginWidget`, `PluginStatus`, `PluginNotify`, and `SystemMessage` flows
- [ ] 4.3 Update standalone and attached-client plugin list/status views so runtime kind, lifecycle state, live tool inventory, and current error text are visible
- [ ] 4.4 Add daemon attach tests proving subscribed/unsubscribed stdio events, plugin UI actions, display messages, and `/plugin` query output all work through the real daemon session path

## 5. Sandbox and launch policy

- [ ] 5.1 Implement manifest-driven launch policy for stdio plugins: command, args, working-directory mode, required environment allowlist, sandbox mode, and writable roots
- [ ] 5.2 Implement explicit `inherit` and `restricted` sandbox modes, including filtered environment, bounded writable roots, network allowance checks, and dedicated plugin state directory handling
- [ ] 5.3 Fail closed when a plugin requests `restricted` mode and the host cannot apply it, and surface that failure through plugin runtime state and error reporting
- [ ] 5.4 Add sandbox tests for environment filtering, missing required environment variables, bounded writes, denied network access when policy disallows it, and restricted-mode startup refusal on unsupported hosts

## 6. Documentation and finish-line validation

- [ ] 6.1 Add developer documentation and example manifests for `kind: stdio`, including protocol expectations, launch policy fields, sandbox modes, and migration guidance from manifest-only Extism tools
- [ ] 6.2 Add or update integration fixtures/examples so plugin authors have at least one minimal reference stdio plugin exercised by tests
- [ ] 6.3 Add a mixed-runtime integration pass that specifically proves Extism + stdio plugins can coexist in one host, preserve Extism behavior, reject cross-kind tool-name collisions deterministically, and surface correct live tool and event visibility
- [ ] 6.4 Run `cargo nextest run`
- [ ] 6.5 Run `cargo clippy -- -D warnings`
- [ ] 6.6 Run `nix build .#clankers`
