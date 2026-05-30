Evidence-ID: focused-validation
Artifact-Type: test-report
Task-ID: V1
Covers: r[steel-executor-run-receipt.execution-receipt.default], r[steel-executor-run-receipt.execution-receipt.rust-native], r[steel-executor-run-receipt.redaction.no-secrets]
Created: 2026-05-30
Status: complete

# Focused Validation

## Commands

```text
TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= cargo test -p clankers-agent run_turn_loop_ --lib
TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= CC=gcc CXX=g++ CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_LINKER=gcc RUSTFLAGS='-C link-arg=-fuse-ld=bfd' cargo test -p clankers steel_runtime_smoke --test embedded_controller
./scripts/check-steel-agent-turn-wiring.rs
./scripts/check-steel-turn-planning-runtime-smoke.rs
```

## Results

```text
run_turn_loop_: 7 passed; 0 failed; STATUS 0
steel_runtime_smoke: 5 passed; 0 failed; STATUS 0
check-steel-agent-turn-wiring: steel agent turn wiring receipt written to target/steel-agent-turn-wiring/receipt.json; STATUS 0
check-steel-turn-planning-runtime-smoke: steel turn planning runtime smoke receipt written to target/steel-turn-planning-runtime-smoke/receipt.json; STATUS 0
```

The focused turn-loop test asserts a redacted `steel.host.execute_turn` receipt for default Steel execution. The embedded-controller smoke asserts default Steel execution emits the execution receipt, while comparison and explicit-disable paths do not.
