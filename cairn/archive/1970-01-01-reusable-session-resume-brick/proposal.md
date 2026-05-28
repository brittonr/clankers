# Proposal: Reusable Session Resume Brick

## Problem

Clankers has product-shaped session resume examples, but reusable SDK code still leaves session persistence/resume to examples or desktop storage. Real resume semantics live across `clankers-session`, `clankers-db`, controller history replay, and agent message conversion. External hosts lack a reusable brick that preserves Clankers-compatible transcript semantics without importing desktop storage shells.

## Proposed Change

Define a reusable session ledger/resume contract that stores neutral transcript entries, tool results, summaries, prompt ids, session ids, and safe receipts. Runtime and desktop adapters should use that contract, while products can provide their own storage backends.

## Impact

- **Files**: `crates/clankers-runtime/src/services.rs`, possible new session-ledger module/crate, `crates/clankers-session`, controller replay adapters, embedded session examples.
- **Testing**: two-backend resume fixtures, missing-session fail-closed, desktop replay parity, transcript conversion rail.
