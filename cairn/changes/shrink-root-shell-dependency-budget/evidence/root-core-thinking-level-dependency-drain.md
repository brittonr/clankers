Artifact-Type: validation-log
Task-ID: I1,I3,V3
Covers: r[remaining-coupling-drain.root-shell-dependency-budget.slice-drain], r[remaining-coupling-drain.root-shell-dependency-budget.budget-evidence], r[remaining-coupling-drain.root-shell-dependency-budget.behavior-validation], r[remaining-coupling-drain.root-shell-dependency-budget.closeout]
Status: pass

## Scope

Removed the root `clankers` crate's direct production dependency on `clankers-core` for thinking-level setup.

- Added `clankers_controller::config::thinking_level_from_message(...)` as the controller-owned reducer-level conversion seam.
- Re-exported the reducer thinking-level type through `clankers_controller::config::CoreThinkingLevel` so root mode code does not name `clankers_core` directly.
- Updated `src/modes/common.rs` to delegate thinking-level conversion to the controller config seam.
- Removed `clankers-core` from the root package dependencies and refreshed the lego dependency ownership baseline.

## Dependency result

The root crate internal dependency count decreased from 29 to 28. The root crate no longer lists `clankers-core` in `target/lego-architecture/dependency-ownership-inventory.json`; reducer policy remains owned by `clankers-controller` and `clankers-core`.

## Validation

Commands run from repository root:

```text
cargo check -p clankers-controller -p clankers --tests
cargo test -p clankers-controller thinking_level_from_message_matches_core_reducer_levels --lib
scripts/check-lego-architecture-boundaries.rs
scripts/check-workspace-layering-rails.rs
```

All commands exited 0.
