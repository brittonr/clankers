# V3: Embedded SDK Parity Evidence

## Checks run

1. `cargo test -p clankers-engine-host --lib` — 26 passed
2. `cargo test -p clankers-agent --lib "turn::tests::"` — 46 passed
3. `cargo test -p clankers-controller --test fcis_shell_boundaries` — 34 passed (includes new `agent_turn_adapters_reject_shared_mutable_turn_state`)
4. `cargo run --manifest-path examples/embedded-agent-sdk/Cargo.toml` — "embedded-agent-sdk example passed"

## Result

All embedded SDK acceptance checks pass after the agent shell restructuring. The engine-host layer, turn tests, FCIS boundary rails, and the standalone embedded example all function correctly with transcript-based adapters and the `TurnLoopContext` parameter shape.
