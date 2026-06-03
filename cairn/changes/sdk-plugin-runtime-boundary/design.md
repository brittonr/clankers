# Design: Separate Plugin Runtime, Manifest, Tool, and UI Boundaries

## Summary

Plugin support has several independent concerns: manifest schema, runtime dispatch, sandbox/launch policy, tool registration, lifecycle supervision, and UI/status projection. This change separates those concerns so product-owned tools can use plugin-like manifests without dragging desktop plugin manager or TUI types.

## Current coupling points

- `PluginManager` owns discovered manifests, Extism instances, stdio supervisors, live state, host events, reserved tool names, and plugin directories.
- `clankers-plugin::ui` re-exports `clanker-tui-types` plugin UI/status types.
- Manifest validation, dispatch ownership, launch policy, and UI projection live in the same crate namespace.
- Root daemon/TUI drains async plugin output into display/protocol events.

## Decisions

### 1. Manifest/tool runtime contracts are neutral

Manifest parsing and runtime dispatch policy should use neutral DTOs. TUI widgets/status projection belongs at display edges.

### 2. Runtime owners remain separate

Extism, stdio, built-in, and product-owned runtimes each have one loader/dispatcher. Non-Extism entries must not flow through WASM loading.

### 3. Desktop plugin manager remains app-edge

Supervisor loops, directories, child processes, and UI queues stay yellow/red desktop composition unless a future kit explicitly narrows them.

## Validation plan

- Manifest/runtime responsibility inventory.
- Source rails rejecting TUI/protocol DTOs in neutral manifest/runtime modules.
- Runtime dispatch matrix fixtures for Extism, stdio, built-in, and product-owned entries.
- Existing stdio/Extism tests plus dependency checks for SDK examples.
