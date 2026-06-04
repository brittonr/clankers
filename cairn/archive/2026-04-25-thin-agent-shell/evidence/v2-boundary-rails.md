# V2: Boundary Rail Evidence

## Test

`agent_turn_adapters_reject_shared_mutable_turn_state` in `crates/clankers-controller/tests/fcis_shell_boundaries.rs`

## What it checks

1. No non-test paths in `crates/clankers-agent/src/turn/mod.rs` reference `SharedTurnHostState` or `TurnHostState`.
2. No source text in the same file contains `Arc<Mutex<TurnHostState>>` or `Arc<parking_lot::Mutex<TurnHostState>>`.

## Result

```
PASS [0.050s] clankers-controller::fcis_shell_boundaries agent_turn_adapters_reject_shared_mutable_turn_state
Summary: 1 test run: 1 passed
```

## Verification

```bash
CC=gcc CXX=g++ CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_LINKER=gcc RUSTFLAGS='-C link-arg=-fuse-ld=bfd' \
  cargo nextest run -p clankers-controller --test fcis_shell_boundaries agent_turn_adapters
```
