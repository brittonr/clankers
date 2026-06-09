Artifact-Type: validation-log
Task-ID: I40,V39
Covers: r[remaining-coupling-drain.runtime-facade-contract-split.green-contracts], r[remaining-coupling-drain.runtime-facade-contract-split.docs], r[remaining-coupling-drain.runtime-facade-contract-split.validation], r[remaining-coupling-drain.runtime-facade-contract-split.closeout]
Status: pass

## Scope

Moved native process/job log-retention DTO and pure log-reference projection to the neutral tool-host process/job owner:

- Added `clankers_tool_host::process_jobs::ProcessJobLogRetentionPolicy` with its `reference_for(...)` helper.
- Re-exported the policy through `clankers-runtime::process_jobs` so existing runtime public API paths remain available.
- Kept runtime-owned log store/service traits, append/read operations, garbage collection orchestration, wall-clock call sites, and runtime errors in `clankers-runtime`; only the backend-neutral retention record and log-ref projection moved to the existing process/job contract crate.
- Regenerated `docs/src/generated/runtime-facade-api.md` after the ownership change.

## Validation

Commands run from repository root:

```text
cargo check -p clankers-tool-host -p clankers-runtime -p clankers --tests
cargo test -p clankers-tool-host log_retention_policy_projects_safe_log_reference_without_host_io --lib
cargo test -p clankers-runtime retention_policy_classifies_metadata_lifetimes_and_active_protection --lib
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
