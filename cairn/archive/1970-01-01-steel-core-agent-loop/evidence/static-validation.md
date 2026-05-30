Evidence-ID: static-validation
Artifact-Type: test-report
Task-ID: V2
Covers: r[steel-core-agent-loop.no-ambient-authority.host-effects], r[steel-core-agent-loop.fail-closed.before-provider]
Created: 2026-05-30
Status: complete

# Static Validation

## Commands

```text
TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= cargo test -p clankers-controller --test fcis_shell_boundaries
rustfmt --check crates/clankers-agent/src/turn/mod.rs crates/clankers-agent/src/turn/steel_execution.rs crates/clankers-agent/src/turn/steel_planning.rs scripts/check-steel-agent-turn-wiring.rs
git diff --check
./scripts/check-steel-agent-turn-wiring.rs
```

## Results

```text
running 35 tests
...
test agent_turn_delegates_runner_policy_to_host_runner ... ok
...
test result: ok. 35 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
STATUS 0

rustfmt --check ...
STATUS 0

git diff --check
STATUS 0

./scripts/check-steel-agent-turn-wiring.rs
steel agent turn wiring receipt written to target/steel-agent-turn-wiring/receipt.json
STATUS 0
```
