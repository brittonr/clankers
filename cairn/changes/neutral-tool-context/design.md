# Design: Neutral Tool Context

## Summary

Tools should receive a host-neutral invocation context: call id, cancellation, safe event/progress sink, capability decision, and optional typed host services. Shell-only data such as `AgentEvent`, redb handles, TUI progress widgets, and hook pipeline internals should live in adapters.

## Decisions

### Decision: reusable context lives below agent

A `ToolInvocationContext` or equivalent should be owned by `clankers-tool-host` or `clankers-runtime`, with conversion from/to the existing agent `ToolContext` during migration.

### Decision: storage and hooks are services, not fields

Tools that need persistence, search, hooks, or process management should request them through typed host service traits or fail closed when unavailable. The neutral context must not expose `clankers_db::Db` directly.

### Decision: progress is semantic

Progress/result streaming should use provider-neutral DTOs that can be projected into TUI, daemon, runtime, or SDK events without importing display crates.

## Verification Plan

- Add compatibility adapters from old `Tool` to new `ToolExecutor` and back while migrating built-ins incrementally.
- Add source-boundary checks forbidding `AgentEvent`, `clankers_db`, `clanker-tui-types`, and hook internals in reusable tool-host contexts.
- Add positive and fail-closed tool fixtures for storage absent, capability denied, cancellation, progress, and truncation.
