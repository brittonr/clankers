# Green ledger core evidence

Evidence-ID: promote-session-ledger-green-sdk.green-ledger-core
Artifact-Type: implementation-evidence
Task-ID: I1,I2,I5
Covers: session-resume-brick.green-ledger-core, session-resume-brick.green-ledger-core.no-runtime-shell, session-resume-brick.green-ledger-core.deterministic-replay
Date: 2026-06-04
Status: PASS

## Core promotion

The reusable session ledger core now lives in `crates/clankers-engine-host/src/session_ledger.rs`, a green SDK owner already used by minimal embedded products. The module exports `SessionLedgerEntry`, `SessionLedgerMessage`, `SessionLedgerRecord`, replay metadata, `SessionLedgerError`, and deterministic conversion helpers between ledger history and `EngineMessage` without importing `clankers-runtime`, `clankers-session`, daemon protocol, database, TUI, wall-clock construction, or runtime-specific errors.

`crates/clankers-runtime/src/ledger.rs` is now a compatibility adapter: it aliases the green ledger types with runtime `PromptId`/`EventMetadata` parameters and maps neutral `SessionLedgerError` into `RuntimeError::SessionUnsupported` at the yellow runtime edge.

## API/policy artifacts

- `docs/src/generated/embedded-sdk-api.md`: 659 scanned public items / 664 inventory rows; session ledger rows in `clankers-engine-host` are `supported`.
- `policy/embedded-lego/brick-inventory-stability.json`: total `664`, supported `513`, optional-support `67`, experimental `23`, unsupported-internal `61`, stable-contract `580`, stable hash `7133165a686fa2ff5e4b6cc70616797e6d04eae0b083aa253fc996f600b115cb`.
- `policy/embedded-lego/runtime-facade-boundary.json`: runtime ledger group reclassified as `yellow-compat-reexports-for-green-ledger`.

## Command evidence

```text
cargo test -p clankers-engine-host --lib session_ledger
running 2 tests
session_ledger::tests::session_ledger_replay_is_deterministic_and_counts_non_message_entries ... ok
session_ledger::tests::session_ledger_unsupported_entries_fail_closed_with_neutral_error ... ok
exit=0

cargo check -p clankers-engine-host -p clankers-runtime --tests
Finished `dev` profile
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
```
