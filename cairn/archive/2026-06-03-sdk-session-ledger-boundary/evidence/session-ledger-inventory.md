Task-ID: I1
Covers: sdk-session-ledger-boundary.inventory
Artifact-Type: inventory

# Session Ledger Boundary Inventory

## Summary

Added `scripts/check-session-ledger-boundary.rs` as the Phase 1 owner receipt for desktop and SDK session persistence paths.

## Owner map

- Neutral SDK ledger/runtime: `crates/clankers-runtime/src/ledger.rs`, `crates/clankers-runtime/src/session.rs`.
- Host-owned SDK examples: `examples/embedded-session-store/src/main.rs`, `examples/embedded-product-workbench/src/main.rs`.
- Selected daemon resume adapter path: `src/modes/session_ledger.rs`, `src/modes/daemon/session_builder.rs`.
- Desktop compatibility setup/storage/merge/restore: `crates/clankers-session/src/lib.rs`, `crates/clankers-session/src/merge.rs`, `src/modes/session_setup.rs`, `src/modes/interactive.rs`.
- Controller persistence/search adapter: `crates/clankers-controller/src/persistence.rs`.
- Display replay projection/app edge: `src/modes/session_restore.rs`, `crates/clankers-controller/src/convert.rs`, `src/modes/attach/events.rs`.
- Existing resume fixture rail: `scripts/check-session-resume-brick.rs`.

## Validation

`nix develop -c cargo -q -Zscript scripts/check-session-ledger-boundary.rs`

Result: `ok: session ledger boundary inventory covers 15 paths`.
