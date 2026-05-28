# Proposal: Neutral Tool Context

## Problem

The real agent `Tool` API is still tied to Clankers shell concerns. `ToolContext` carries `AgentEvent`, `clankers_db::Db`, search index, hook pipeline, TUI progress DTOs, and session metadata. `clankers-tool-host` is clean, but built-in and plugin tools cannot be reused as SDK bricks without importing the product shell.

## Proposed Change

Introduce a neutral tool invocation context and result/progress/event contracts in reusable tool-host/runtime crates. Real built-in/plugin tools should be adapted at the app edge, while tool implementations depend on host-provided capabilities, event sinks, storage handles, and cancellation traits.

## Impact

- **Files**: `crates/clankers-tool-host`, `crates/clankers-agent/src/tool.rs`, `src/tools/**`, `crates/clankers-runtime/src/tools.rs`, plugin adapters.
- **Testing**: built-in tool adapter fixtures, source-boundary rail for forbidden shell imports, tool-host compatibility tests.
