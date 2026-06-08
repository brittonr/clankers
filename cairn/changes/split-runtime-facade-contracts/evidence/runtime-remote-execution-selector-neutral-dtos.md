Artifact-Type: validation-log
Task-ID: I20,V19
Covers: r[remaining-coupling-drain.runtime-facade-contract-split.green-contracts], r[remaining-coupling-drain.runtime-facade-contract-split.docs], r[remaining-coupling-drain.runtime-facade-contract-split.validation], r[remaining-coupling-drain.runtime-facade-contract-split.closeout]
Status: pass

## Scope

Moved remote/subagent execution selector DTOs to neutral message contracts:

- Added `clanker_message::RemoteExecutionArtifactKind` for safe artifact dependency kind labels.
- Added `clanker_message::RemoteExecutionTarget` for subagent vs remote-daemon execution target labels.
- Re-exported both DTOs through `clankers-runtime::effects` / crate root so existing runtime public API paths remain available.
- Kept `RemoteExecutionDependency`, `RemoteExecutionRequest`, artifact hashes, sync reports, envelope validation, UCAN metadata, and executable dependency-sync behavior in `clankers-runtime`; only reusable selector enum ownership moved.
- Regenerated `docs/src/generated/runtime-facade-api.md` after the ownership change.

## Validation

Commands run from repository root:

```text
cargo check -p clanker-message -p clankers-runtime -p clankers --tests
cargo test -p clanker-message remote_execution_selectors_roundtrip_preserve_kebab_case --lib
cargo test -p clankers-runtime remote_execution_request_declares_safe_dependencies_by_artifact_hash --lib
cargo test -p clankers-runtime remote_execution_required_hashes_project_for_effect_dependencies --lib
scripts/check-runtime-facade-boundary.rs --write-inventory
scripts/check-runtime-facade-boundary.rs
cargo -q -Zscript scripts/check-runtime-facade-split.rs
scripts/check-message-contract-boundary.rs
scripts/check-lego-architecture-boundaries.rs
scripts/check-workspace-layering-rails.rs
cargo test -p clankers --no-run
nix run .#cairn -- gate tasks split-runtime-facade-contracts --root .
nix run .#cairn -- validate --root .
git diff --check
```

All listed commands exited 0.
