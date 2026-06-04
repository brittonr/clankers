# Design: Promote Session Ledger Green SDK

## Context

`crates/clankers-runtime/src/ledger.rs` contains neutral DTOs and replay projection, but it also depends on runtime identifiers, `RuntimeError`, and `Utc::now()` for record construction. Policy still classifies `clankers-runtime` as outside the generic green SDK crate set.

## Decisions

### 1. Pure ledger core is green

The reusable ledger core should contain only serializable entries, roles, summaries, usage, safe receipt metadata, and deterministic replay into engine-native messages. It should not own storage, clocks, desktop session IDs, daemon seeds, or runtime errors.

### 2. Runtime and desktop code become adapters

`clankers-runtime`, `src/modes/session_ledger.rs`, daemon resume seed handling, and `clankers-session` compatibility paths should convert into or out of the green ledger core at named adapter seams.

### 3. Examples dogfood the promoted API

`examples/embedded-session-store` and `examples/embedded-product-workbench` should use the green ledger API for persisted model-visible history while retaining product-owned storage wrappers and receipts.

## Risks / Trade-offs

- Extracting a new crate may require lockfile, flake, build-plan, and docs updates.
- Moving ledger types too broadly could accidentally promote desktop session storage; keep storage adapters outside green crates.
- Existing runtime facade API may need compatibility reexports while consumers migrate.
