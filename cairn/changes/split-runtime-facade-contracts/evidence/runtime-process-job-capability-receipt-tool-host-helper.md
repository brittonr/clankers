Artifact-Type: validation-log
Task-ID: I41,V40
Covers: r[remaining-coupling-drain.runtime-facade-contract-split.green-contracts], r[remaining-coupling-drain.runtime-facade-contract-split.docs], r[remaining-coupling-drain.runtime-facade-contract-split.validation], r[remaining-coupling-drain.runtime-facade-contract-split.closeout]
Status: pass

## Scope

Moved the process/job backend capability unsupported-receipt projection helper to the neutral tool-host process/job owner:

- Added `clankers_tool_host::process_jobs::ProcessJobBackendCapabilitiesReceiptExt`.
- Re-exported the extension trait through `clankers-runtime::process_jobs` so existing runtime public API paths remain available.
- Kept runtime-owned service/backend/store traits, backend validation call sites, and runtime orchestration in `clankers-runtime`; only the backend-neutral capability-to-receipt helper moved to the existing process/job contract crate.
- Regenerated `docs/src/generated/runtime-facade-api.md` after the ownership change.

## Validation

Commands run from repository root:

```text
cargo check -p clankers-tool-host -p clankers-runtime -p clankers --tests
cargo test -p clankers-tool-host backend_capability_extension_projects_unsupported_receipts --lib
cargo test -p clankers-runtime service_validation_can_fail_closed_before_backend_mutation --lib
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
