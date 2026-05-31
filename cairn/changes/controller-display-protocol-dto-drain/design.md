# Design: Drain Controller Display and Protocol DTO Coupling

## Summary

The controller is a session/runtime shell, not a display or wire-protocol owner. This change moves remaining display/protocol DTOs to adapter edges while keeping command lifecycle behavior unchanged.

## Decisions

### 1. Thinking and loop controls use neutral/core types

Controller command policy should parse thinking levels into `CoreThinkingLevel` or a controller-local DTO, not `clanker_tui_types::ThinkingLevel`. Auto-test loop sync should use a neutral loop status DTO instead of `LoopDisplayState`.

### 2. Projection modules remain the allowed DTO edge

`convert.rs`, `transport_convert.rs`, and explicitly named attach/TUI adapter modules may construct TUI/protocol DTOs. Reusable command, effect interpretation, persistence, and runtime adapter modules should emit semantic/domain state instead.

### 3. Drain one protocol constructor path at a time

Direct `DaemonEvent` construction is still common in command code. This change should move at least one user-visible output branch to semantic/domain projection and update rails to guide future branch migrations without overfitting formatting.

### 4. Rails use AST ownership checks

The FCIS and lego rails should inspect Rust item/module ownership for TUI/protocol constructors. Diagnostics should name the expected projection owner rather than reporting only a missing string anchor.

## Validation plan

- Unit fixtures for thinking-level parsing and loop status sync using neutral/core DTOs.
- Conversion fixtures proving TUI/protocol output remains byte/shape compatible after projection.
- Attach parity tests for `/think`, loop status, disabled tools if touched, and replay behavior if event projection changes.
- FCIS and lego source-boundary rails updated with owner diagnostics.
