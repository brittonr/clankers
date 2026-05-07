## Why

The embedded SDK acceptance rail is green only when the caller clears `CDPATH`; otherwise Bash command substitution captures `cd` output and computes an invalid repository root. The same status review found low-risk hygiene gaps: dead-code warnings in the agent turn adapter module and stale OpenSpec drain-state text that still says the last commit is pending after the queue is drained.

## What Changes

- Make embedded SDK verification scripts robust against caller shell environment such as `CDPATH`.
- Remove or narrow dead-code warnings in `clankers-agent` turn helpers without weakening engine-adapter rails.
- Reset drain-state bookkeeping when there are no active changes so future drain reviews start from accurate state.

## Capabilities

### Modified Capabilities
- `embeddable-agent-engine`: embedded SDK acceptance is reproducible from ordinary shells and keeps warning/drain-state hygiene visible.

## Impact

- **Files**: `scripts/check-embedded-agent-sdk.sh`, `crates/clankers-agent/src/turn/*`, `openspec/changes/.drain-state.md`, and focused tests if needed.
- **APIs**: no public SDK API changes intended.
- **Testing**: run the script from a polluted `CDPATH` shell, the normal embedded SDK acceptance command, and focused agent turn tests.
