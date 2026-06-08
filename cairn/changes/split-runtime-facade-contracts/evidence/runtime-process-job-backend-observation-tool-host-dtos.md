Artifact-Type: validation-log
Task-ID: I35,V34
Covers: r[remaining-coupling-drain.runtime-facade-contract-split.green-contracts], r[remaining-coupling-drain.runtime-facade-contract-split.docs], r[remaining-coupling-drain.runtime-facade-contract-split.validation], r[remaining-coupling-drain.runtime-facade-contract-split.closeout]
Status: pass

## Scope

Moved process/job backend observation result DTOs to the neutral tool-host process/job owner:

- Added `clankers_tool_host::process_jobs::ProcessJobBackendStart` and `ProcessJobBackendStatus`.
- Re-exported both contracts through `clankers-runtime::process_jobs` so existing runtime public API paths remain available.
- Kept the runtime-owned async `ProcessJobBackend` trait, process/job service orchestration, storage boundaries, runtime errors, and backend mutation behavior in `clankers-runtime`; only the backend-neutral accepted-start and observed-status records moved to the existing process/job contract crate.
- Added the tool-host `chrono` workspace dependency because the existing public `ProcessJobBackendStatus` contract carries `DateTime<Utc>`.
- Regenerated `docs/src/generated/runtime-facade-api.md` after the ownership change.

## Validation

Commands run from repository root:

```text
cargo check -p clankers-tool-host -p clankers-runtime -p clankers --tests
cargo test -p clankers-tool-host backend_status_contract_preserves_backend_ref_status_and_logs --lib
cargo test -p clankers-runtime fake_backend_contract_covers_projection_and_mutations --lib
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
