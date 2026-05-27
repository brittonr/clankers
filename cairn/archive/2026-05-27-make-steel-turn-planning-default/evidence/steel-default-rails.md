# Steel default rails evidence

Evidence-ID: steel-default-rails
Artifact-Type: command-output-summary
Task-ID: V4
Covers: steel-default-orchestration.rollout-evidence.default-current-seam
Date: 2026-05-27
Status: PASS

## Baseline before implementation

Command group:

```text
TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= cargo test -p clankers-config steel_turn_planning --lib
TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= cargo test -p clankers-agent steel_turn_planning --lib
./scripts/check-steel-turn-planning-config-activation.rs
./scripts/check-steel-agent-turn-wiring.rs
```

Result: pueue task 24 PASS. Note: `cargo test -p clankers-agent steel_turn_planning --lib` matched zero tests in the pre-change baseline; post-change verification used `turn::steel_planning` to run the full module.

## Post-change focused rails

Commands:

```text
TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= cargo test -p clankers-config steel_turn_planning --lib
TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= cargo test -p clankers-agent turn::steel_planning --lib
CC=gcc CXX=g++ CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_LINKER=gcc RUSTFLAGS='-C link-arg=-fuse-ld=bfd' RUSTC_WRAPPER= CARGO_TARGET_DIR=target/steel-runtime-smoke-test cargo test -p clankers steel_runtime_smoke --test embedded_controller
./scripts/check-steel-turn-planning-runtime-smoke.rs
./scripts/check-steel-turn-planning-config-activation.rs
./scripts/check-steel-default-orchestration.rs
./scripts/check-steel-agent-turn-wiring.rs
./scripts/check-steel-turn-planning-ucan-authority.rs
```

Relevant output:

```text
clankers-config steel_turn_planning: 5 passed
clankers-agent turn::steel_planning: 17 passed
embedded_controller steel_runtime_smoke: 5 passed
steel turn planning runtime smoke receipt written to target/steel-turn-planning-runtime-smoke/receipt.json
steel turn planning config activation receipt written to target/steel-turn-planning-config-activation/receipt.json
steel default orchestration receipt written to target/steel-default-orchestration/profile-receipt.json
steel agent turn wiring receipt written to target/steel-agent-turn-wiring/receipt.json
steel turn planning UCAN authority receipt written to target/steel-turn-planning-ucan-authority/receipt.json
```

## Coverage notes

The post-change smoke includes positive default settings coverage, explicit-disable opt-out coverage, explicit profile smoke coverage, bundled-script over-budget rejection, malformed profile rejection in the artifact core, hash-mismatch failure before provider calls, and missing-authority failure before provider calls.
