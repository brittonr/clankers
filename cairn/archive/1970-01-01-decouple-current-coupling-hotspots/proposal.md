# Proposal: Decouple Current Coupling Hotspots

## Problem

Recent architecture scouting found nine active coupling hotspots that keep Clankers hard to evolve in narrow slices:

1. `clankers-config` depends upward on TUI/rendering types.
2. `clankers-agent` still knows too many concrete systems directly.
3. root `src/modes/common.rs` centralizes every built-in/plugin/tool factory.
4. `clankers-controller` mixes session domain policy with daemon wire DTOs.
5. `src/modes/daemon/socket_bridge.rs` combines socket IO, session construction, actor spawn, resume, and registry mutation.
6. slash command handlers mutate `App` and send agent/session commands directly.
7. `clankers-provider` duplicates parts of `clanker-router`'s provider/request/routing contract.
8. `clankers-runtime` and the `process` tool expose an oversized process-job/runtime seam.
9. root compatibility re-export modules preserve old import paths and blur extracted-crate ownership.

These are architectural blockers rather than one bug. Fixing them piecemeal without accepted boundaries risks trading one coupling shape for another.

## Proposed Change

Add a Cairn package that captures concrete decoupling requirements, sequencing, and validation rails for all nine hotspots. This change is planning/specification only: it defines the target behavior-preserving boundaries future implementation changes must satisfy.

The implementation direction is:

- Make config data-only and project config into TUI/runtime types at edges.
- Shrink agent dependencies behind explicit turn ports/service adapters.
- Replace the monolithic tool factory with capability-specific registries/factories.
- Move controller command/event handling onto domain DTOs before protocol projection.
- Split daemon control socket handling from session building and actor launch.
- Make slash commands return declarative effects instead of mutating every subsystem directly.
- Choose one provider/router owner per request-shaping/routing concern.
- Split runtime process-job contracts from backend/storage/tool adapters.
- Remove root compatibility re-exports after call sites import owning crates directly.

## Impact

- No public CLI, daemon protocol, or user-facing behavior changes are required by this planning package.
- Future implementation work gets traceable requirement IDs and validation expectations.
- Each hotspot can be implemented as a smaller Cairn change while preserving current behavior with focused tests and architecture rails.
