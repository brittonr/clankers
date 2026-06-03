# Design: Neutral Display and Protocol Effects

## Context

Display/protocol leakage was partially reduced by neutral progress DTOs. The remaining opportunity is slash and attach policy, where `SessionCommand` and TUI state still appear in reusable decisions.

## Decisions

### 1. Neutral effect first

Define policy output in terms of domain/action intent, not daemon protocol or TUI display structs.

### 2. Edge adapters project explicitly

Standalone, attach, remote attach, and daemon paths should translate neutral effects at their edges.

### 3. Parity tests are mandatory

Every moved slash/effect family must have standalone and attach parity assertions.

## Risks / Trade-offs

- Slash behavior has many small parity rules; move one command family at a time.
- Projection adapters can duplicate suppression logic; keep shared ack policy neutral.
- TUI snapshots may change if display semantics move; prefer behavioral tests first.
