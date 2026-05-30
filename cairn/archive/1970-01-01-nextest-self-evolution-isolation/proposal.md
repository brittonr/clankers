# Proposal: Nextest Self-Evolution Isolation

## Why

A broad `cargo nextest run` should never mutate Clankers source files. During UCAN auth closeout, a full nextest pass was followed by unrelated source-tree diffs that looked like self-evolution/cleanup edits. Even when the exact culprit is environmental or concurrent, the test suite still has risky seams: some subprocess integration tests inherit the repository root or user home while exercising tools that can write, edit, apply, or rollback artifacts.

Those seams make a green broad suite harder to trust because a test or fake-provider prompt can accidentally receive authority over the real checkout instead of a disposable fixture.

## What Changes

- Run self-evolution CLI subprocess tests from a per-test temporary working directory with an isolated HOME/XDG environment.
- Keep readiness fake-provider tool tests inside the `ReadinessSandbox` workdir instead of overriding them back to the repository root.
- Record focused evidence that the isolated subprocess/tool rails pass without relying on the real checkout as their working directory.

## Impact

- **Files**: `tests/self_evolution_cli.rs`, `tests/readiness_e2e.rs`, Cairn change artifacts.
- **Testing**: focused nextest runs for `self_evolution_cli` and `readiness_e2e`, `git diff --check`, Cairn gates/validation.
