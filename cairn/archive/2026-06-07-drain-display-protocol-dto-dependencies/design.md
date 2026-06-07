# Design: Drain Display/Protocol DTO Dependencies

## Context

Display and protocol DTO crates are intentionally shared at edges, but several orchestration or utility crates still depend on them. Existing rails protect some controller constructors, yet dependency-level coupling remains: a reusable crate can start depending on TUI state or protocol variants because the DTO crate is available.

## Decisions

### 1. Classify each display/protocol dependency by edge role

**Choice:** Every dependency on `clanker-tui-types` or `clankers-protocol` is classified as display edge, transport edge, shared neutral DTO, or drain target.

**Rationale:** Some types are genuinely shared contracts. Others are display-only or transport-only and should not shape reusable policy.

### 2. Replace display-only inputs with neutral DTOs

**Choice:** Config, model-selection, procmon, util, plugin, controller, and root policy modules should use neutral enums/DTOs for thinking level, loop state, status, progress, or summaries; TUI conversion happens at display adapters.

**Rationale:** Display state should not be canonical domain state. Neutral DTOs keep reusable logic testable without ratatui/TUI dependencies.

### 3. Protocol constructors stay at transport projection owners

**Choice:** Reusable logic emits neutral events/results; `convert.rs`, `transport_convert.rs`, and transport adapters own wire DTO construction.

**Rationale:** Protocol variants are wire contracts, not domain policy. Constructor-owner rails are the strongest guard against drift.

### 4. Dependency rails complement constructor rails

**Choice:** Add or extend rails to catch forbidden package-level display/protocol dependencies in lower or reusable crates, not only constructor sites.

**Rationale:** A crate-level dependency can become future constructor/policy leakage even if current code is harmless.

## Risks / Trade-offs

- Some crates may currently use `clanker-tui-types` as a convenient neutral type source; replacing those uses can cause broad type renames.
- Protocol DTOs may be stable wire contracts useful for tests; test helpers should import through projection fixtures rather than policy modules.
- Edge exceptions must be narrow or the rail becomes a rubber stamp.
