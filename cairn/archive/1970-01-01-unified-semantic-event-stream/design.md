# Design: Unified Semantic Event Stream

## Summary

The semantic event stream should be the stable middle layer between reusable runtime/engine behavior and presentation/transport adapters. It should be serializable, safe, and rich enough to cover existing user-visible behavior without carrying UI widgets or protocol frames.

## Decisions

### Decision: semantic event contract is reusable

The contract should live in a reusable crate/module below controller/TUI/protocol. It may use shared message/content DTOs, but not daemon frames, TUI blocks, Matrix types, provider-native payloads, or root shell state.

### Decision: projection is one-way at edges

Agent/runtime/controller should emit or convert to semantic events. Daemon, TUI, Matrix, remote attach, JSON, and batch outputs project from semantic events at their edges.

### Decision: event ordering is fixture-backed

Migration must include fixtures that pin causal order across prompt accepted, content deltas, tool start/result, usage, error, and completion.

## Verification Plan

- Add a canonical event fixture and run it through runtime, agent/controller, daemon projection, and TUI/JSON projection where practical.
- Add a source-boundary rail forbidding transport/display DTO constructors in semantic event modules.
- Add negative redaction tests for metadata and hidden context.
