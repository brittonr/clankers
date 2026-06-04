Task-ID: R1
Covers: embeddable-agent-engine.composable-host-contract, embeddable-agent-engine.host-extraction-rails, embeddable-agent-engine.core-engine-boundary-rails
Artifact-Type: validation-evidence

# R1 prerequisite cleanup evidence

Prerequisite changes referenced by design:

- `decouple-llm-contract-surface` archived before this change.
- `separate-engine-core-composition` archived before this change.

## `./scripts/check-llm-contract-boundary.sh`

```text
ok: clankers-engine normal-edge tree excludes forbidden crates
ok: clanker-message normal-edge tree excludes forbidden crates
ok: crates/clankers-engine/src excludes forbidden source tokens
```

## `cargo test -p clankers-controller --test fcis_shell_boundaries`

```text
running 28 tests
...
test result: ok. 28 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.35s
```

## `openspec validate extract-composable-engine-host --strict`

```text
Change 'extract-composable-engine-host' is valid
```
