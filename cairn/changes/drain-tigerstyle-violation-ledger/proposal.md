# Change: Drain Tigerstyle Violation Ledger

## Why

Tigerstyle progress currently depends on an informal memory of crate-level and local `tigerstyle::...` allowances. That makes it easy to lose track of remaining violations, validate slices inconsistently, or declare a crate clean before the full workspace audit confirms it.

A native Cairn ledger will turn every current Tigerstyle allowance site into traceable work with explicit validation evidence. The ledger should preserve the current narrow-slice workflow while making the finish line auditable.

## What Changes

- Add a Tigerstyle compliance change package that inventories every current Tigerstyle allow site in source code.
- Track each allow site as typed implementation and verification work, with exact lint names recorded for that site.
- Require each drain slice to remove or justify the corresponding allowance, run focused package validation, run root compile validation when public APIs move, and run the full Tigerstyle audit.
- Record evidence for each completed slice before checking off the matching verification task.

## Impact

- **Files**: `src/{lib.rs,main.rs}`, crate root `lib.rs` files with Tigerstyle allow lists, and narrow local allow sites under `crates/**`.
- **Testing**: affected package tests, `cargo test -p clankers --no-run` for cross-crate API movement, and full `./xtask/tigerstyle.sh -- --keep-going`.
- **Lifecycle**: this change package becomes the canonical task ledger for Tigerstyle violation burn-down.
