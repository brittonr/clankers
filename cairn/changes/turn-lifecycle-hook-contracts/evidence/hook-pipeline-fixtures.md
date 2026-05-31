Evidence-ID: hook-pipeline-fixtures
Task-ID: V1
Artifact-Type: command-log
Covers: turn-lifecycle-hooks.prompt-hooks, turn-lifecycle-hooks.agent-turn-hooks
Status: passed

# Hook Pipeline Fixtures

Implemented the first contract slice:

- Added `HookPoint::PreTurn` with script filename `pre-turn`.
- Added `HookPoint::PostTurn` with script filename `post-turn`.
- Kept `HookPoint::TurnStart` / `TurnEnd` as separate non-blocking lifecycle notification names.
- Made `PreTurn` a blocking deny-capable pre hook but not a mutating pre hook.
- Kept `PrePrompt` / `PreTool` mutation behavior through `allows_modify()`.
- Tightened `HookPipeline::fire(...)` so non-pre hook verdicts remain observational and return `Continue`.
- Added script-hook tests for `PreTurn` denial, `PostTurn` continue, `PrePrompt` modification, and ignored `PreTurn` modification output.
- Added plugin event parser coverage for `pre_turn` and `post_turn` mapping names.

## Commands

```text
rustfmt crates/clankers-hooks/src/point.rs \
  crates/clankers-hooks/src/dispatcher.rs \
  crates/clankers-hooks/src/script.rs \
  crates/clankers-plugin/src/hooks.rs \
  crates/clankers-plugin/src/bridge.rs \
  crates/clankers-plugin/src/bridge_tests.rs
```

Result: exit 0.

```text
env TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= \
  cargo nextest run -p clankers-hooks
```

Result: 43 tests run, 43 passed, 0 skipped.

```text
env TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= \
  cargo nextest run -p clankers-plugin bridge
```

Result: 4 tests run, 4 passed, 36 skipped.
