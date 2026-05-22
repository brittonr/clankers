# Proposal: Embeddable Agent Engine First Slice

## Problem

Clankers now has several lego seams, but the agent is still primarily experienced as the product shell: CLI, TUI, daemon, session persistence, provider wiring, and turn execution remain difficult for an external Rust host to compose as a small reusable brick. The next coupling risk is that new agent behavior will keep landing in shell paths instead of proving an embeddable engine boundary.

## Proposed Change

Define the first native Cairn package for an embeddable agent engine. The package specifies a narrow reusable engine API, host-supplied ports, shell-owned wiring, deterministic host fixtures, and architecture rails that prevent TUI/daemon/root-shell types from leaking into the reusable engine surface.

This is a planning/specification package. Implementation should drain in narrow follow-up slices: first define the engine facade and fixture host, then migrate one stable turn path behind it without changing standalone, daemon, or attach behavior.

## Impact

- External Rust hosts can target a small engine API instead of constructing the full Clankers product shell.
- Root/TUI/daemon modes stay as imperative shells around reusable engine bricks.
- Existing behavior remains compatible while the engine boundary is introduced behind adapters and parity fixtures.
- Future changes get durable requirement IDs for engine API, port ownership, shell thinness, deterministic fixtures, and architecture rails.
