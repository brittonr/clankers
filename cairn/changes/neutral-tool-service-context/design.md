# Design: Move Agent Tool Execution Services Behind Neutral Context

## Summary

This change turns the neutral tool context from a partial compatibility layer into the owner of storage, hook, capability, cancellation, and progress services needed by tool execution. The agent should select tools and invoke a tool port; concrete desktop services should be hidden behind service traits.

## Decisions

### 1. Service bundle is the migration unit

Rather than pass individual concrete fields through `ControllerToolPort`, introduce a neutral tool service bundle with optional storage/search, hook, progress/event, capability, cancellation, and runtime policy services. Absent services fail closed or produce typed unsupported receipts.

### 2. Legacy tools remain behind an adapter

Existing tools implementing the agent `Tool` trait should continue to work through a legacy adapter. The adapter is the only place allowed to translate between concrete Clankers desktop services and neutral `ToolInvocationContext` services.

### 3. Migrate representative paths first

Pick one storage/search path and one hook/progress path to dogfood the neutral services. That proves the seam without forcing a risky all-tools rewrite.

### 4. Rails enforce reusable context purity

`clankers-tool-host` and runtime neutral context modules should not import `clankers_db`, `clankers_hooks`, `clanker-tui-types`, daemon protocol DTOs, or root tool state. Source rails should parse imports and paths and name the offending service owner.

## Validation plan

- Unit fixtures for neutral service success and missing-service failures.
- Legacy adapter parity fixtures covering hook continue/modify/deny, capability denial, progress emission, and cancellation.
- Focused tests for the representative migrated storage/search and hook/progress tool paths.
- Architecture rail and dependency budget update showing concrete services are edge-owned.
