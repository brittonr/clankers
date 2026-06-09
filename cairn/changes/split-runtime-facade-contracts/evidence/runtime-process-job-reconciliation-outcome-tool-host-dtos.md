Artifact-Type: validation-log
Task-ID: I36,V35
Covers: r[remaining-coupling-drain.runtime-facade-contract-split.green-contracts], r[remaining-coupling-drain.runtime-facade-contract-split.docs], r[remaining-coupling-drain.runtime-facade-contract-split.validation], r[remaining-coupling-drain.runtime-facade-contract-split.closeout]
Status: pass

## Scope

Moved process/job reconciliation outcome projection contracts to the neutral tool-host process/job owner:

- Added `clankers_tool_host::process_jobs::ProcessJobReconciliationOutcome`, `process_job_timestamp`, and the pure `reconcile_external_backend_reference(...)` helper.
- Re-exported those contracts/helpers through `clankers-runtime::process_jobs` so existing runtime public API paths remain available.
- Kept runtime-owned async backend/store/service traits, startup reconciliation orchestration, native log-retention policy, wall-clock call sites, and runtime errors in `clankers-runtime`; only the backend-neutral outcome/timestamp/external-facts projection moved to the existing process/job contract crate.
- Regenerated `docs/src/generated/runtime-facade-api.md` after the ownership change.

## Validation

Commands run from repository root:

```text
cargo check -p clankers-tool-host -p clankers-runtime -p clankers --tests
cargo test -p clankers-tool-host external_backend_reconciliation_maps_matching_backend_facts_to_outcome --lib
cargo test -p clankers-tool-host process_job_timestamp_projects_chrono_seconds --lib
cargo test -p clankers-runtime external_backend_reconciliation_maps_refs_into_common_outcomes --lib
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
