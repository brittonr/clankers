Artifact-Type: validation-log
Task-ID: I37,V36
Covers: r[remaining-coupling-drain.runtime-facade-contract-split.green-contracts], r[remaining-coupling-drain.runtime-facade-contract-split.docs], r[remaining-coupling-drain.runtime-facade-contract-split.validation], r[remaining-coupling-drain.runtime-facade-contract-split.closeout]
Status: pass

## Scope

Moved project process/job profile validation DTOs to the neutral tool-host process/job owner:

- Added `clankers_tool_host::process_jobs::ProjectProcessJobProfileValidationCode` and `ProjectProcessJobProfileValidationError` with stable display text and constructor.
- Re-exported those contracts through `clankers-runtime::process_jobs` so existing runtime public API paths remain available.
- Kept profile parsing/resolution orchestration, manifest source selection, validation logic, runtime error conversion, and all executable backend dispatch in `clankers-runtime`; only the backend-neutral validation code/error records moved to the existing process/job contract crate.
- Regenerated `docs/src/generated/runtime-facade-api.md` after the ownership change.

## Validation

Commands run from repository root:

```text
cargo check -p clankers-tool-host -p clankers-runtime -p clankers --tests
cargo test -p clankers-tool-host project_profile_validation_error_message_is_stable --lib
cargo test -p clankers-runtime profile_policy_rejects_paths_resources_and_unsupported_manifest_versions --lib
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
