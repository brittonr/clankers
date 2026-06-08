Artifact-Type: validation-log
Task-ID: I34,V33
Covers: r[remaining-coupling-drain.runtime-facade-contract-split.green-contracts], r[remaining-coupling-drain.runtime-facade-contract-split.docs], r[remaining-coupling-drain.runtime-facade-contract-split.validation], r[remaining-coupling-drain.runtime-facade-contract-split.closeout]
Status: pass

## Scope

Moved the process/job startup reconciliation report DTO to the neutral tool-host process/job owner:

- Added `clankers_tool_host::process_jobs::ProcessJobReconciliationReport` plus the pure `record_observation` counter helper.
- Re-exported that report through `clankers-runtime::process_jobs` so existing runtime public API paths remain available.
- Kept startup reconciliation orchestration, store/backend traits, backend unavailable summary updates, wall-clock timestamps, and runtime error conversion in `clankers-runtime`; only the backend-neutral report record moved to the existing process/job contract crate.
- Regenerated `docs/src/generated/runtime-facade-api.md` after the ownership change.

## Validation

Commands run from repository root:

```text
cargo check -p clankers-tool-host -p clankers-runtime -p clankers --tests
cargo test -p clankers-tool-host reconciliation_report_counts_observations_and_backend_unavailability --lib
cargo test -p clankers-runtime startup_reconciliation_updates_nonterminal_jobs_and_skips_terminal_records --lib
scripts/check-runtime-facade-boundary.rs --write-inventory
scripts/check-runtime-facade-boundary.rs
cargo -q -Zscript scripts/check-runtime-facade-split.rs
scripts/check-lego-architecture-boundaries.rs
scripts/check-workspace-layering-rails.rs
git diff --check
cargo test -p clankers --no-run
nix run .#cairn -- gate tasks split-runtime-facade-contracts --root .
nix run .#cairn -- validate --root .
git diff --check
```

All listed commands exited 0.
