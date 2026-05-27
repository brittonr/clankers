# Steel default rails evidence

Evidence-ID: steel-default-rails
Artifact-Type: command-output-summary
Task-ID: V3, V4
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
clankers-agent turn::steel_planning: 19 passed
embedded_controller steel_runtime_smoke: 5 passed
steel turn planning runtime smoke receipt written to target/steel-turn-planning-runtime-smoke/receipt.json
steel turn planning config activation receipt written to target/steel-turn-planning-config-activation/receipt.json
steel default orchestration receipt written to target/steel-default-orchestration/profile-receipt.json
steel agent turn wiring receipt written to target/steel-agent-turn-wiring/receipt.json
steel turn planning UCAN authority receipt written to target/steel-turn-planning-ucan-authority/receipt.json
```

## Coverage notes

The post-change smoke includes positive default settings coverage, explicit-disable opt-out coverage, explicit profile smoke coverage, bundled-script over-budget rejection, malformed profile rejection in the artifact core, malformed script rejection in the artifact core before Steel execution, hash-mismatch failure before provider calls, and missing-authority failure before provider calls.

## V3 negative-case matrix

- Missing bundled default profile: covered as a compile-time failure by `include_bytes!("../../../../policy/steel-default-orchestration/orchestration-profile.json")` in `crates/clankers-agent/src/turn/steel_planning.rs`; the focused `clankers-agent` compile/test rail cannot build if the checked-in profile path is missing.
- Missing bundled default script: covered as a compile-time failure by `include_str!("../../../../policy/steel-default-orchestration/scripts/default-plan-turn.scm")` in `crates/clankers-agent/src/turn/steel_planning.rs`; the focused `clankers-agent` compile/test rail cannot build if the checked-in script path is missing.
- Malformed bundled default profile: covered by `artifact_core_rejects_malformed_profile_json`.
- Malformed bundled default script: covered by `artifact_core_rejects_malformed_script_before_steel_execution`, which rejects a non-`steel.host.plan_turn` source while constructing `AgentTurnSteelPlanningConfig`, before any Steel runtime evaluation can start.
- Hash-mismatched bundled default script: covered by `settings_activation_rejects_bundled_hash_mismatch`; runtime smoke also covers hash-mismatched settings fail closed before receipt/provider work.
- Over-budget bundled default script: covered by `settings_activation_rejects_bundled_script_over_budget`.
- Actual checked-in bundled script parser health: covered by `settings_activation_uses_bundled_default_without_paths` and `steel_runtime_smoke_default_settings_emit_redacted_receipt`, both exercising the compiled bundled script.
