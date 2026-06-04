## Why

The current deterministic replay rail proves the pure `clankers-engine` reducer, but it intentionally does not cross the shell boundary where Clankers has regressed before: `Agent`/`SessionController` provider request shaping, session metadata propagation, tool dispatch correlation, emitted events, and session transcript mutation.

## What Changes

- Add a credential-free deterministic replay test at the controller/agent boundary.
- Reuse scripted provider/tool fixture ideas while exercising the shell path that builds provider-native requests and dispatches tool calls.
- Extend the deterministic harness profile so the cheap local rail covers both pure engine replay and controller replay.

## Capabilities

### Modified Capabilities
- `deterministic-agent-testing`: Adds a controller/agent seam replay requirement beyond pure engine replay.
- `test-harness`: Extends the `deterministic` profile to include controller replay once implemented.

## Impact

- **Files**: controller/agent tests, deterministic fixtures/helpers, `scripts/test-harness.sh`, harness contract tests, OpenSpec specs.
- **APIs**: Test-only seams only unless existing public test helpers can be reused.
- **Dependencies**: Prefer existing workspace dependencies; no live credentials, network, daemon sockets, or ambient user config.
- **Testing**: Focused controller replay test, harness contract test, deterministic harness profile, OpenSpec validation, formatting, and `git diff --check`.
