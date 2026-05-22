# Design: Embeddable Agent Engine First Slice

## Summary

The embeddable engine should be a reusable Rust brick that accepts neutral turn inputs, talks to host-provided ports, and emits neutral events/receipts. Product shells remain responsible for CLI parsing, TUI rendering, daemon framing, provider/auth construction, filesystem/session persistence wiring, and user-visible transport policy.

## Decisions

### Decision: engine facade is neutral

The first public engine facade should expose neutral request, event, outcome, and receipt DTOs. It must not require callers to construct TUI state, daemon protocol frames, CLI command structs, Matrix bridge types, or concrete provider/router/auth stores.

### Decision: hosts supply effects through ports

Provider completion, tool execution, session persistence, prompt/history loading, hooks, and cost accounting should be expressed as ports or adapter-owned DTO boundaries. The engine may orchestrate turn state, but concrete I/O and app-edge policy stay in shells or host adapters.

### Decision: Clankers shell dogfoods the same API

Standalone CLI/TUI/daemon paths should eventually call the same engine facade an external host would call. The first implementation slice may introduce an internal facade and fixture host before migrating every shell path, but it must include a parity plan for the existing controller/agent turn path.

### Decision: deterministic fixture host proves embeddability

A fixture host should run one minimal turn with fake model/tool ports and no live credentials, sockets, TUI, or daemon. The fixture should prove that host-visible inputs and outputs are stable enough to serve as the engine API contract.

### Decision: rails guard inward leaks

Architecture rails should reject engine modules that import root-shell, TUI, daemon protocol, Matrix, or concrete provider/auth/router construction types. Concrete adapters may import both sides, but reusable engine modules should only depend on neutral DTOs and port traits.

## Migration Strategy

1. Add this Cairn package and gates so the boundary is explicit before code moves.
2. Introduce the smallest engine facade or module around the already-extracted agent turn ports.
3. Add a deterministic fixture host with fake model/tool ports and one positive turn receipt.
4. Add a negative rail or fixture proving display/protocol/root-shell DTOs do not leak into the engine module.
5. Migrate one existing shell path to call the facade through an adapter, then expand parity coverage before broader migration.

## Non-goals

- No public plugin SDK, daemon protocol, or provider API compatibility break is required.
- No live provider, OAuth, Matrix, remote attach, or TUI rendering behavior is changed by this planning package.
- No claim is made that all Clankers modes are embeddable until shell paths are migrated and verified through the facade.
