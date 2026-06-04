# Ledger adapter and product dogfood evidence

Evidence-ID: promote-session-ledger-green-sdk.ledger-adapters
Artifact-Type: implementation-evidence
Task-ID: I3,I4,V1,V2
Covers: session-resume-brick.ledger-adapters, session-resume-brick.ledger-adapters.product-examples, session-resume-brick.ledger-adapters.desktop-edge, session-resume-brick.green-ledger-core.no-runtime-shell
Date: 2026-06-04
Status: PASS

## Adapter seams

- `clankers-runtime` consumes the green ledger through compatibility type aliases in `crates/clankers-runtime/src/ledger.rs` and keeps runtime/session errors at the app edge.
- `src/modes/session_ledger.rs` remains the desktop transcript adapter: persisted `AgentMessage` records are converted to neutral `SessionLedgerEntry` values before daemon seed projection.
- `examples/embedded-session-store` and `examples/embedded-product-workbench` now store model-visible history as `clankers-engine-host::SessionLedgerMessage` while keeping product-owned session stores and receipt DTOs local.

## Command evidence

```text
cargo test -p clankers-runtime --lib session_resume
running 2 tests
tests::session_resume_missing_or_unsupported_store_fails_before_model ... ok
tests::session_resume_two_backends_restore_ordered_ledger_context ... ok
exit=0

cargo run --locked --manifest-path examples/embedded-session-store/Cargo.toml
embedded-session-store passed
exit=0

cargo run --locked --manifest-path examples/embedded-product-workbench/Cargo.toml
embedded-product-workbench passed
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
```
