# Change: Move Agent Tool Execution Services Behind Neutral Context

## Problem

`clankers-agent::turn::ports::ControllerToolPort` still carries concrete `clankers_db::Db`, `clankers_hooks::HookPipeline`, capability-gate, progress/event sender, and legacy tool maps into tool execution. The existing neutral tool context work introduced the target direction, but storage, hook, progress, and capability services still leak through the agent adapter.

## Goals

- Define neutral tool host services for storage/search, hook decisions, progress/events, capability checks, and cancellation.
- Move `ControllerToolPort` to pass a neutral `ToolInvocationContext`/service bundle instead of concrete DB/hook/event fields.
- Migrate at least one storage/search tool path and one hook/progress path to the neutral service context.
- Keep legacy `Tool` implementations supported through an explicit adapter while shrinking concrete agent dependencies.

## Non-goals

- Do not migrate every built-in tool in one slice.
- Do not remove plugin or stdio tool support; adapters must preserve existing behavior.
- Do not change user-facing tool result JSON except for safe metadata additions required by neutral receipts.

## Proposed scope

Add service traits or DTOs to `clankers-tool-host`/`clankers-runtime` for the concrete services currently threaded through `ControllerToolPort`. Update the legacy adapter to resolve concrete DB/hook/progress behavior at the product edge and present neutral services to tool execution.

The first slice is intentionally an inventory/source-rail pass: name every concrete service currently crossing `ControllerToolPort`, then fail future changes that add new DB/hook/TUI/protocol/root fields to reusable tool-host context modules without an owner receipt.

## Verification

Validation should include neutral tool fixtures for success, missing service, hook denial/modify/continue, capability denial, cancellation, progress emission, and legacy adapter parity; source rails should reject DB/hook/TUI/protocol imports in reusable tool-host context modules.
