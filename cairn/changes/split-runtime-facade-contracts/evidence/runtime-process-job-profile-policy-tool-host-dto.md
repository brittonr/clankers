Artifact-Type: validation-log
Task-ID: I32,V31
Covers: r[remaining-coupling-drain.runtime-facade-contract-split.green-contracts], r[remaining-coupling-drain.runtime-facade-contract-split.docs], r[remaining-coupling-drain.runtime-facade-contract-split.validation], r[remaining-coupling-drain.runtime-facade-contract-split.closeout]
Status: pass

## Scope

Moved project process/job profile policy DTOs to the neutral tool-host process/job owner:

- Added `clankers_tool_host::process_jobs::ProjectProcessJobProfilePolicy` with its native-only default policy.
- Re-exported that contract through `clankers-runtime::process_jobs` so existing runtime public API paths remain available.
- Kept profile manifest parsing, profile resolution, validation error projection, profile-start orchestration, storage/service traits, and runtime error conversion in `clankers-runtime`; only the backend-neutral profile policy bounds record moved to the existing process/job contract crate.
- Regenerated `docs/src/generated/runtime-facade-api.md` after the ownership change.

## Validation

Commands run from repository root:

```text
cargo check -p clankers-tool-host -p clankers-runtime -p clankers --tests
cargo test -p clankers-tool-host project_process_job_profile_policy_defaults_to_native_only --lib
cargo test -p clankers-runtime process_job_profile_kit_validates_manifest_policy_identity_and_redaction --lib
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
