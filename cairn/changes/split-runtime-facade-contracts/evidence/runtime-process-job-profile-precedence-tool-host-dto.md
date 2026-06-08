Artifact-Type: validation-log
Task-ID: I33,V32
Covers: r[remaining-coupling-drain.runtime-facade-contract-split.green-contracts], r[remaining-coupling-drain.runtime-facade-contract-split.docs], r[remaining-coupling-drain.runtime-facade-contract-split.validation], r[remaining-coupling-drain.runtime-facade-contract-split.closeout]
Status: pass

## Scope

Moved project process/job profile source-precedence selector DTOs to the neutral tool-host process/job owner:

- Added `clankers_tool_host::process_jobs::ProjectProcessJobProfileSourcePrecedence` with its stable labels and ordering.
- Re-exported that selector through `clankers-runtime::process_jobs` so existing runtime public API paths remain available.
- Kept manifest source selection, profile resolution, fail-closed duplicate detection, validation error projection, and runtime error conversion in `clankers-runtime`; only the backend-neutral precedence selector moved to the existing process/job contract crate.
- Regenerated `docs/src/generated/runtime-facade-api.md` after the ownership change.

## Validation

Commands run from repository root:

```text
cargo check -p clankers-tool-host -p clankers-runtime -p clankers --tests
cargo test -p clankers-tool-host project_process_job_profile_source_precedence_orders_by_specificity --lib
cargo test -p clankers-runtime profile_manifest_sources_resolve_by_deterministic_precedence --lib
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
