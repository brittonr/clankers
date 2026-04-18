## Why

Clankers' plugin system is strong for small in-process Extism/WASM plugins, but it is a poor fit for richer integrations that need native dependencies, long-lived state, arbitrary language runtimes, or direct operating-system access. Tau's highest-value idea is the process-backed extension model: supervised tools that speak a simple protocol over stdio, register capabilities live, and can be sandboxed per extension.

Clankers already has most of the host-side pieces needed to adopt that idea cleanly: actor/process primitives, daemon and attach transports, shared plugin discovery, and existing plugin event/UI flows. This is a good time to add process-backed providers without replacing the current WASM path.

## What Changes

- Add a new supervised `stdio` plugin kind for process-backed extensions that live alongside existing Extism and Zellij plugins.
- Keep the current plugin directories, disable/enable flows, `/plugin` views, and manifest-based discovery model.
- Add a framed stdio protocol for process-backed plugins: handshake, ready, live tool registration/unregistration, tool invocation, tool progress/result/error, and event subscriptions.
- Change daemon session tool inventory from manifest-only plugin tools to a mixed model: static Extism tools plus live tools registered by connected process-backed plugins.
- Surface process-plugin runtime state to users and attached clients: starting, active, backoff, error, disabled.
- Add per-plugin launch policy and sandbox metadata so restricted process plugins can run with filtered environment, bounded filesystem access, and fail-closed startup when restrictions cannot be applied.
- Preserve the existing Extism plugin path for lightweight UI, hook, and host-function plugins.

## Capabilities

### New Capabilities
- `process-extension-runtime`: Supervised stdio plugin runtime that launches, monitors, restarts, and reports the lifecycle of process-backed plugins.
- `process-extension-protocol`: Connection-scoped stdio protocol for process-backed plugins to register tools live, receive tool invocations, emit results/progress/errors, and subscribe to events.
- `process-extension-sandboxing`: Launch profiles, filtered environment, and fail-closed sandbox application for process-backed plugins.

### Modified Capabilities
- `daemon-plugin-loading`: Daemon plugin discovery and status reporting expand from Extism-only startup to mixed Extism plus process-backed plugins with runtime state.
- `daemon-plugin-tools`: Daemon plugin tool inventory and rebuild logic change from static manifest-only tools to live registered plugin tools.
- `daemon-plugin-events`: Daemon event forwarding and plugin UI/message surfacing expand to process-backed plugins while keeping existing payload shapes.

## Impact

- `crates/clankers-plugin/` — manifest schema, plugin summary/state model, process runtime host, live tool registry
- `src/modes/common.rs` — plugin initialization, tool construction, mixed plugin-kind startup
- `src/modes/plugin_dispatch.rs` — event forwarding and UI/display responses for process-backed plugins
- `src/modes/daemon/{mod.rs,socket_bridge.rs,agent_process.rs}` — shared plugin host wiring, daemon session rebuilds, plugin status propagation
- `crates/clankers-protocol/` — plugin summary/status payloads exposed to clients
- Plugin documentation and example manifests for `kind: stdio`

## Verification

- Add at least one minimal reference stdio plugin fixture that exercises real handshake, registration, invocation, cancellation, and shutdown behavior.
- Keep Extism regression coverage explicit: mixed Extism + stdio discovery, tool inventory, event delivery, and conflict handling must keep current Extism behavior unchanged.
- Keep sandbox fail-closed behavior CI-testable through deterministic restricted-mode and missing-environment-variable tests rather than manual-only verification.

## Non-Goals

- Replacing Extism plugins or removing the current WASM host-function path.
- Switching daemon/client transports from length-prefixed JSON to CBOR in this change.
- Adding socket-attached or remote process plugins in the first iteration.
- Rewriting the session controller or agent loop around a new global event-bus architecture.
