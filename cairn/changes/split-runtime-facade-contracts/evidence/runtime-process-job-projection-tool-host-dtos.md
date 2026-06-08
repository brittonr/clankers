Artifact-Type: validation-log
Task-ID: I28,V27
Covers: r[remaining-coupling-drain.runtime-facade-contract-split.green-contracts], r[remaining-coupling-drain.runtime-facade-contract-split.docs], r[remaining-coupling-drain.runtime-facade-contract-split.validation], r[remaining-coupling-drain.runtime-facade-contract-split.closeout]
Status: pass

## Scope

Moved runtime process/job list projection contracts to the neutral tool-host process/job owner:

- Added `clankers_tool_host::process_jobs::ProcessJobLifecycleBucket`, `ProcessJobProjectionBounds`, `ProcessJobProjectionItem`, `ProcessJobListProjection`, and `project_process_job_list`.
- Re-exported those contracts through `clankers-runtime::process_jobs` so existing runtime public API paths remain available.
- Kept runtime process/job reconciliation, backend orchestration, storage/service traits, profile validation, notification policy execution, and runtime error handling in `clankers-runtime`; only backend-neutral list projection DTOs and their pure projection helper moved to the already-neutral process/job contract crate.
- Regenerated `docs/src/generated/runtime-facade-api.md` after the ownership change.

## Validation

Commands run from repository root:

```text
cargo check -p clankers-tool-host -p clankers-runtime -p clankers --tests
cargo test -p clankers-tool-host process_job_list_projection_splits_sorts_and_truncates_lifecycles --lib
cargo test -p clankers-runtime list_projection_includes_safe_capability_hints_only --lib
cargo test -p clankers-runtime process_job_projection_unifies_backends_and_bounds_active_completed_views --lib
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
