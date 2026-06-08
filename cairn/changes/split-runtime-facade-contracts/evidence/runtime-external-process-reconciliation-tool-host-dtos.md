Artifact-Type: validation-log
Task-ID: I30,V29
Covers: r[remaining-coupling-drain.runtime-facade-contract-split.green-contracts], r[remaining-coupling-drain.runtime-facade-contract-split.docs], r[remaining-coupling-drain.runtime-facade-contract-split.validation], r[remaining-coupling-drain.runtime-facade-contract-split.closeout]
Status: pass

## Scope

Moved external backend process/job reconciliation input DTOs to the neutral tool-host process/job owner:

- Added `clankers_tool_host::process_jobs::ExternalProcessJobBackendState` and `ExternalProcessJobReconciliationFacts`.
- Re-exported those contracts through `clankers-runtime::process_jobs` so existing runtime public API paths remain available.
- Kept `reconcile_external_backend_reference`, reconciliation outcomes, summary timestamp updates, storage/service traits, and backend orchestration in `clankers-runtime`; only backend-neutral external reconciliation input records moved to the existing process/job contract crate.
- Regenerated `docs/src/generated/runtime-facade-api.md` after the ownership change.

## Validation

Commands run from repository root:

```text
cargo check -p clankers-tool-host -p clankers-runtime -p clankers --tests
cargo test -p clankers-tool-host external_reconciliation_facts_roundtrip_preserves_backend_state --lib
cargo test -p clankers-runtime external_backend_reconciliation_maps_refs_into_common_outcomes --lib
cargo test -p clankers-runtime external_backend_reconciliation_fails_closed_for_unavailable_missing_or_mismatched_refs --lib
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
