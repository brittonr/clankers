## Why

The engine/host split has focused unit and adapter tests, but the acceptance rail does not yet prove feature interactions across model streaming, retries, tool calls, cancellation, usage observation, and budget/stop policies in one explicit matrix. Decoupling is only durable if combined feature paths cannot drift back into shell-local orchestration.

## What Changes

- Add a matrix-driven verification suite for `clankers-engine` and `clankers-engine-host` feature combinations.
- Cover positive and negative combinations rather than isolated single-feature cases only.
- Require machine-readable matrix cases so gaps are visible when new engine features are added.

## Capabilities

### Modified Capabilities
- `embeddable-agent-engine`: engine/host acceptance includes combined feature-matrix coverage.

## Impact

- **Files**: engine/host tests, matrix fixtures, and acceptance scripts.
- **APIs**: no public API changes unless fixture metadata requires a test-only helper.
- **Testing**: `cargo test -p clankers-engine --lib`, `cargo test -p clankers-engine-host --lib`, and embedded SDK acceptance.
