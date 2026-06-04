# Validation closeout evidence

Evidence-ID: promote-session-ledger-green-sdk.validation-closeout
Artifact-Type: command-output-summary
Task-ID: V2,V3
Covers: session-resume-brick.green-ledger-core, session-resume-brick.green-ledger-core.no-runtime-shell, session-resume-brick.green-ledger-core.deterministic-replay, session-resume-brick.ledger-adapters, session-resume-brick.ledger-adapters.product-examples, session-resume-brick.ledger-adapters.desktop-edge
Date: 2026-06-04
Status: PASS

## Commands completed

```text
cargo test -p clankers-engine-host --lib session_ledger
cargo test -p clankers-runtime --lib session_resume
cargo test -p clankers-runtime --test api_compat
cargo run --locked --manifest-path examples/embedded-session-store/Cargo.toml
cargo run --locked --manifest-path examples/embedded-product-workbench/Cargo.toml
scripts/check-embedded-sdk-api.rs
scripts/check-brick-inventory-stability.rs
scripts/check-experimental-sdk-port-budget.rs
scripts/check-session-resume-brick.rs
scripts/check-session-ledger-boundary.rs
scripts/check-embedded-sdk-deps.rs
scripts/check-runtime-facade-boundary.rs
scripts/check-behavioral-lego-rails.rs
scripts/emit-embedded-sdk-release-receipt.rs
env TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= scripts/check-embedded-agent-sdk.rs
git diff --check
nix run .#cairn -- validate --root .
nix run .#cairn -- gate proposal promote-session-ledger-green-sdk --root .
nix run .#cairn -- gate design promote-session-ledger-green-sdk --root .
nix run .#cairn -- gate tasks promote-session-ledger-green-sdk --root .
```

## Relevant output

```text
cargo test -p clankers-engine-host --lib session_ledger
running 2 tests
session_ledger::tests::session_ledger_replay_is_deterministic_and_counts_non_message_entries ... ok
session_ledger::tests::session_ledger_unsupported_entries_fail_closed_with_neutral_error ... ok
exit=0

cargo test -p clankers-runtime --lib session_resume
running 2 tests
tests::session_resume_missing_or_unsupported_store_fails_before_model ... ok
tests::session_resume_two_backends_restore_ordered_ledger_context ... ok
exit=0

cargo test -p clankers-runtime --test api_compat
running 8 tests
ledger_module_and_root_reexports_are_source_compatible ... ok
exit=0

cargo run --locked --manifest-path examples/embedded-session-store/Cargo.toml
embedded-session-store passed
exit=0

cargo run --locked --manifest-path examples/embedded-product-workbench/Cargo.toml
embedded-product-workbench passed
exit=0

scripts/check-embedded-sdk-api.rs
ok: embedded SDK API inventory covers 659 public items (664 rows)
exit=0

scripts/check-brick-inventory-stability.rs
brick-inventory-stability receipt written to target/embedded-sdk-release/brick-inventory-stability-receipt.json
exit=0

scripts/check-experimental-sdk-port-budget.rs
ok: experimental SDK port budget covers 23 experimental rows; 137 promoted rows
exit=0

scripts/check-session-resume-brick.rs
session-resume-brick receipt written to target/embedded-sdk-release/session-resume-brick-receipt.json
exit=0

scripts/check-session-ledger-boundary.rs
ok: session ledger boundary inventory covers 16 paths
exit=0

scripts/check-embedded-sdk-deps.rs
ok: embedded SDK example dependency graph has 56 packages and excludes forbidden runtime crates
exit=0

scripts/check-runtime-facade-boundary.rs
ok: runtime facade boundary inventories clankers-runtime public API and dependency classifications
exit=0

scripts/check-behavioral-lego-rails.rs
behavioral lego rail inventory receipt written to target/embedded-sdk-release/behavioral-rail-inventory-receipt.json
exit=0

scripts/emit-embedded-sdk-release-receipt.rs
embedded SDK release receipt written to target/embedded-sdk-release/receipt.json
exit=0

pueue task 15: promote-session-ledger-green-sdk-acceptance-final
command: env TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= scripts/check-embedded-agent-sdk.rs
start: Thu, 4 Jun 2026 13:57:58 -0400
end: Thu, 4 Jun 2026 14:05:32 -0400
embedded-agent-sdk acceptance passed
exit=0

git diff --check
exit=0

nix run .#cairn -- validate --root .
valid=true
changes=1
specs_validated=124
exit=0

nix run .#cairn -- gate proposal promote-session-ledger-green-sdk --root .
verdict=PASS
exit=0

nix run .#cairn -- gate design promote-session-ledger-green-sdk --root .
verdict=PASS
exit=0

nix run .#cairn -- gate tasks promote-session-ledger-green-sdk --root .
verdict=PASS
exit=0
```

## Closeout note

After recording this evidence and checking V2/V3 complete, final `git diff --check`, Cairn validation, and the proposal/design/tasks gates were rerun so the evidence packet is proven after the last evidence/task edit.
