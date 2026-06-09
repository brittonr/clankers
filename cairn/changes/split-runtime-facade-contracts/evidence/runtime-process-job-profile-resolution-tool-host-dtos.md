Artifact-Type: validation-log
Task-ID: I38,V37
Covers: r[remaining-coupling-drain.runtime-facade-contract-split.green-contracts], r[remaining-coupling-drain.runtime-facade-contract-split.docs], r[remaining-coupling-drain.runtime-facade-contract-split.validation], r[remaining-coupling-drain.runtime-facade-contract-split.closeout]
Status: pass

## Scope

Moved project process/job profile manifest/resolution DTOs and pure resolution helpers to the neutral tool-host process/job owner:

- Added `clankers_tool_host::process_jobs::{ProjectProcessJobProfiles, ProjectProcessJobProfile, ProjectProcessJobProfileResolution, ProjectProcessJobProfileResolutionEvidence, ProjectProcessJobProfileManifestSource, PROCESS_JOB_PROFILE_SCHEMA_VERSION}`.
- Moved pure profile JSON parsing, deterministic source precedence selection, backend-neutral start-spec projection, resource/path/env validation helpers, and sensitive-key checks into `clankers-tool-host`.
- Re-exported the moved contracts through `clankers-runtime::process_jobs` so existing runtime public API paths remain available.
- Kept runtime-owned service/backend/storage traits, profile start dispatch, runtime error projection at tool/service call sites, and all executable backend behavior in `clankers-runtime` or root adapters.
- Regenerated `docs/src/generated/runtime-facade-api.md` after the ownership change.

## Validation

Commands run from repository root:

```text
cargo check -p clankers-tool-host -p clankers-runtime -p clankers --tests
cargo test -p clankers-tool-host project_profile_resolution_produces_backend_neutral_start_spec --lib
cargo test -p clankers-runtime process_job_profile_kit_validates_manifest_policy_identity_and_redaction --lib
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
