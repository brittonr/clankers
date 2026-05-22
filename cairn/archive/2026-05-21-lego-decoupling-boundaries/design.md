# Design: Lego Decoupling Boundaries

## Summary

This change defines architecture contracts, not implementation code. The intended implementation sequence is behavior-preserving extraction: add a narrow port/DTO/core seam, prove existing behavior with focused fixtures, then move concrete shell code behind the seam.

## Decisions

### Decision: one shell, many bricks

The root `clankers` crate should remain the product shell and command dispatcher. Reusable behavior belongs in workspace bricks with small public APIs. Root may wire bricks together, but it must not own domain policy, request shaping, storage schemas, or rendering semantics inline.

### Decision: domain events precede display/protocol events

Agent/controller/runtime code should emit neutral domain events and receipts. TUI, daemon protocol, Matrix, and attach transports should project those neutral events through explicit adapters. `clanker-tui-types` is allowed as a display DTO crate, not as the canonical domain model.

### Decision: behavior parity must be shared, not reimplemented

Local, daemon, local attach, and remote attach paths should use the same command/effect/ack policy core. Transport-specific code may frame and deliver events, but it must not reimplement thinking-level, disabled-tool, compaction, queued-prompt, or ack-suppression policy.

### Decision: rails should parse architecture, not grep folklore

String-presence rails are acceptable as temporary drift checks, but final lego rails should parse Cargo metadata, Rust ASTs, fixtures, or typed manifests so failures identify the exact boundary violation and owner.

## Migration Strategy

1. Start with `process` tool adapter thinness because it has the largest duplicated concrete surface and a clear request/service/receipt shape.
2. Move agent turn dependencies behind ports after process-job seams prove the pattern.
3. Split controller responsibilities into input translation, core effect interpretation, domain event translation, and transport projection.
4. Consolidate provider/router ownership so there is one request-shaping owner and one routing owner.
5. Convert attach parity and event projection to shared cores.
6. Replace brittle string rails with typed boundary manifests and AST/Cargo checks as each seam stabilizes.

## Non-goals

- No public CLI or protocol behavior change is required by this planning package.
- No provider contract, OAuth flow, plugin runtime, or process backend is removed.
- No broad historical cleanup is required before a narrow implementation slice can begin.
