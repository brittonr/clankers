Artifact-Type: validation-log
Task-ID: I29,V28
Covers: r[remaining-coupling-drain.runtime-facade-contract-split.green-contracts], r[remaining-coupling-drain.runtime-facade-contract-split.docs], r[remaining-coupling-drain.runtime-facade-contract-split.validation], r[remaining-coupling-drain.runtime-facade-contract-split.closeout]
Status: pass

## Scope

Moved backend-neutral process/job reconciliation identity/status contracts to the neutral tool-host process/job owner:

- Added `clankers_tool_host::process_jobs::ProcessJobReconciliationState`, `ProcessJobLogReconciliationState`, `NativeProcessJobIdentity`, and `NativeProcessJobObservation`.
- Re-exported those contracts through `clankers-runtime::process_jobs` so existing runtime public API paths remain available.
- Kept runtime reconciliation outcomes, external backend reconciliation, summary timestamp updates, storage/service traits, and backend orchestration in `clankers-runtime`; only backend-neutral identity/status DTOs and their conservative identity check moved to the existing process/job contract crate.
- Regenerated `docs/src/generated/runtime-facade-api.md` after the ownership change.

## Validation

Commands run from repository root:

```text
cargo check -p clankers-tool-host -p clankers-runtime -p clankers --tests
cargo test -p clankers-tool-host process_job_reconciliation_state_classifies_adopted_and_fail_closed --lib
cargo test -p clankers-tool-host native_process_identity_conservatively_verifies_observations --lib
cargo test -p clankers-runtime reconciliation_state_vocabulary_serializes_and_classifies_fail_closed_states --lib
cargo test -p clankers-runtime native_identity_reconciliation_fails_closed_on_pid_reuse_or_ambiguous_identity --lib
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
