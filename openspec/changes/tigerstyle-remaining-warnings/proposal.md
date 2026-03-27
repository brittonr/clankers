## Why

The tigerstyle dylint lints flag ~272 warnings across the clankers workspace. Two rounds of fixes reduced the count from ~900, handling the mechanical cases (ignored results, platform casts, narrowing casts, infallible writes). The remaining 272 are structural — they require refactoring patterns, not search-and-replace. Leaving them degrades the signal-to-noise ratio of future lint runs and allows new regressions to hide among existing noise.

## What Changes

- Eliminate all remaining tigerstyle lint warnings through targeted refactoring
- Establish patterns and utility code (e.g., a `write_str!` macro, panel accessor helpers) that prevent recurrence
- Add crate-level `#[cfg_attr(dylint_lib = "tigerstyle", allow(...))]` with documented reasons only where the lint is a genuine false positive

Breakdown by category:
- **Bool naming** (77): Rename struct fields and their callers where `is_`/`has_` prefix fits; add documented allows where it doesn't (`bold`, `italic`, `compact`)
- **`.unwrap()`/`.expect()`** (72): Replace with `?` propagation, `.ok()`, or extract to helper functions that return `Result`; document justified cases (mutex locks, static regexes, startup invariants)
- **Unbounded loops** (39): Add explicit iteration caps with `take()` or counter-based bounds; document event loops with `#[allow]` + reason
- **Unchecked division** (18): Add zero-guards or `checked_div` for TUI layout divisions; document guarded-by-`is_empty()` cases
- **Catch-all enum** (17): Replace `_ =>` with exhaustive variant listing so the compiler catches new variants
- **`panic!()` / `unreachable!()`** (16): Replace with error returns or `debug_assert!`; keep `unreachable!()` only in provably unreachable arms
- **Deep nesting** (11): Extract helper functions to flatten conditional depth below 5 levels
- **Acronym style** (5): Rename types to follow `RpcHandler` convention (not `RPCHandler`)
- **Function length** (8): Decompose functions over 100 lines into focused helpers

## Capabilities

### New Capabilities
- `tigerstyle-bool-naming`: Rename boolean struct fields and local variables to use predicate prefixes across the workspace
- `tigerstyle-error-propagation`: Replace `.unwrap()`/`.expect()` with proper error handling (Result propagation, helper extractors, documented allows)
- `tigerstyle-loop-bounds`: Add iteration bounds to loops or document them as bounded event loops
- `tigerstyle-enum-exhaustive`: Replace catch-all `_ =>` patterns on enums with exhaustive matches
- `tigerstyle-structural-cleanup`: Fix deep nesting, function length, acronym style, unchecked division, and panic removal

### Modified Capabilities

## Impact

- ~120 files across the workspace will be touched (primarily `src/`, `crates/clankers-tui/`, `crates/clankers-agent/`)
- No API changes — all fixes are internal
- New helper functions/macros may be added to `crates/clankers-util/` for common patterns (panel downcast, bounded iteration)
- Bool renames will cascade through struct construction sites and pattern matches
- Enum exhaustive matches will cause future compile errors when new variants are added (desired behavior)
