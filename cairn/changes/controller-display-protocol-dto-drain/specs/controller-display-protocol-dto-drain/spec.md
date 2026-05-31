## ADDED Requirements

### Requirement: Controller policy uses neutral display inputs [r[controller-display-protocol-dto-drain.neutral-inputs]]

Reusable controller command, auto-test, runtime, and persistence policy MUST use core or controller-neutral DTOs instead of display-only TUI state for decisions.

#### Scenario: Thinking control is core-owned [r[controller-display-protocol-dto-drain.neutral-inputs.thinking]]
- GIVEN a thinking level is parsed or applied by controller command policy
- WHEN source-boundary rails inspect the command path
- THEN the controller MUST use `CoreThinkingLevel` or a controller-local neutral DTO
- AND `clanker_tui_types::ThinkingLevel` MUST appear only in TUI/attach projection edges or tests

#### Scenario: Loop display state is edge-projected [r[controller-display-protocol-dto-drain.neutral-inputs.loop-state]]
- GIVEN auto-test or loop synchronization needs current loop status
- WHEN reusable controller code receives that status
- THEN it MUST receive a neutral loop status DTO
- AND `clanker_tui_types::LoopDisplayState` MUST be converted at the TUI/attach edge before entering controller policy

### Requirement: Protocol DTOs stay at projection adapters [r[controller-display-protocol-dto-drain.protocol-edge]]

Controller behavior that emits user-visible or transport-visible output MUST construct protocol DTOs through explicit projection adapters rather than treating protocol events as canonical domain state.

#### Scenario: Command output goes through projection owner [r[controller-display-protocol-dto-drain.protocol-edge.command-output]]
- GIVEN a controller command branch emits user-visible output
- WHEN that output can be represented as a semantic/domain event or receipt
- THEN the branch MUST route through the named projection owner before producing `DaemonEvent`
- AND any direct protocol constructor that remains MUST have a documented edge-owner exception

#### Scenario: Transport conversion remains centralized [r[controller-display-protocol-dto-drain.protocol-edge.transport]]
- GIVEN attach, daemon, Matrix, or remote transports need wire frames
- WHEN frames are produced
- THEN `transport_convert.rs` or another explicit transport adapter MUST own wire DTO construction
- AND reusable command/effect/runtime modules MUST NOT construct transport DTOs for decisions

### Requirement: DTO ownership rails are typed [r[controller-display-protocol-dto-drain.boundary-rails]]

Architecture rails MUST use typed Rust/module inventories to distinguish allowed projection adapters from forbidden inward display/protocol state.

#### Scenario: Rails diagnose expected owner [r[controller-display-protocol-dto-drain.boundary-rails.owner-diagnostics]]
- GIVEN a forbidden TUI/protocol DTO constructor appears in controller policy
- WHEN validation runs
- THEN the diagnostic MUST name the offending module, DTO, allowed projection owner, and requirement id
- AND refactors that preserve ownership SHOULD NOT fail only because source text moved

### Requirement: DTO drain preserves parity [r[controller-display-protocol-dto-drain.verification]]

The drain MUST preserve daemon/attach/TUI behavior and be covered by focused conversion fixtures and architecture rails.

#### Scenario: Closeout proves unchanged projection behavior [r[controller-display-protocol-dto-drain.verification.closeout]]
- GIVEN neutral DTO replacements and projection moves are complete
- WHEN closeout validation runs
- THEN controller fixtures, conversion tests, attach parity tests, FCIS/lego rails, Cairn gates/validate, and diff checks MUST pass or include explicit checked evidence for environmental limitations
