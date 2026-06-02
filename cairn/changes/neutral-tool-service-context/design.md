# Design: Move Agent Tool Execution Services Behind Neutral Context

## Summary

This change turns the neutral tool context from a partial compatibility layer into the owner of storage, hook, capability, cancellation, and progress services needed by tool execution. The agent should select tools and invoke a tool port; concrete desktop services should be hidden behind service traits.

This is the next decoupling layer after `controller-runtime-adapter-production`: controller command policy now reaches the agent through an adapter, but agent turn execution still threads product-shell services through `ControllerToolPort`. This change drains those services into neutral contracts so tool execution can move independently of daemon/TUI/root assembly.

## Initial inventory targets

I1 should inventory and rail the current concrete service edges before moving code:

- `crates/clankers-agent/src/turn/ports.rs::ControllerToolPort` currently carries the legacy tool map, `broadcast::Sender<AgentEvent>`, cancellation token, `clankers_hooks::HookPipeline`, session id, `clankers_db::Db`, capability gate, user tool filter, and Steel substrate policy.
- `crates/clankers-agent/src/turn/execution.rs` constructs `ControllerToolPort` from `Agent` shell fields.
- `crates/clankers-agent/src/turn/adapters.rs`, `transcript.rs`, `usage.rs`, and `policy.rs` still expose `AgentEvent` as the progress/event surface that the neutral progress service must eventually wrap.
- Legacy `ToolContext` construction remains the compatibility edge for built-in tools until representative storage/search and hook/progress paths migrate.

The I1 source inventory is checked into `crates/clankers-agent/src/turn/ports.rs` as `CONTROLLER_TOOL_PORT_SERVICE_INVENTORY` and `LEGACY_TOOL_CONTEXT_SERVICE_INVENTORY`. The lego architecture rail also inspects the `ControllerToolPort` and legacy `ToolContext` fields so future service drift must update the inventory intentionally.

The I2 neutral service contracts live in `crates/clankers-tool-host/src/lib.rs` and intentionally use semantic DTOs only: `ToolStorageService`, `ToolSearchService`, `ToolHookService`, `ToolProgressSink`, `ToolCapabilityService`, `ToolCancellationService`, and `ToolRuntimePolicyService`. `ToolInvocationContext` can carry these services by `Arc<dyn ...>` while the old handle inventory remains available during migration.

The I3 controller port migration keeps concrete desktop handles at adapter construction: `ControllerToolServices::from_concrete(...)` builds neutral progress, cancellation, hook, and capability services plus a legacy runner. Reusable tool execution now consumes `ControllerToolServices` through neutral service traits and invokes legacy tools through `LegacyToolRunner`; the runner remains the compatibility edge that constructs old `ToolContext` until concrete production tools migrate.

The I4 representative migration adds the `Tool::uses_neutral_tool_context()` / `execute_with_neutral_context(...)` seam and a deterministic neutral-native controller tool path that requires storage and search services, emits neutral progress, and panics if the legacy runner is used. This proves the controller executor can run storage/search and hook/progress paths through `ToolInvocationContext` before moving a production tool off `ToolContext`.

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
