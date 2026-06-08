Artifact-Type: validation-log
Task-ID: I21,V20
Covers: r[remaining-coupling-drain.runtime-facade-contract-split.green-contracts], r[remaining-coupling-drain.runtime-facade-contract-split.docs], r[remaining-coupling-drain.runtime-facade-contract-split.validation], r[remaining-coupling-drain.runtime-facade-contract-split.closeout]
Status: pass

## Scope

Moved remaining simple effect/tool policy status DTOs to neutral message contracts:

- Added `clanker_message::EffectResultStatus` for effect handler outcome labels.
- Added `clanker_message::RemoteDependencyFailureKind` for fail-closed remote dependency sync failure labels.
- Added `clanker_message::ToolCollisionPolicy` for tool catalog collision policy labels and default behavior.
- Re-exported those DTOs through `clankers-runtime::effects` / `clankers-runtime::tools` / crate root so existing runtime public API paths remain available.
- Kept `EffectResult`, `RemoteDependencyFailure`, sync reports, tool catalog builders, effect handlers, artifact hashing, and executable runtime behavior in `clankers-runtime`; only reusable enum DTO ownership moved.
- Regenerated `docs/src/generated/runtime-facade-api.md` after the ownership change.

## Validation

Commands run from repository root:

```text
cargo check -p clanker-message -p clankers-runtime -p clankers --tests
cargo test -p clanker-message effect_result_status_roundtrip_preserves_kebab_case --lib
cargo test -p clanker-message remote_dependency_failure_kind_roundtrip_preserves_kebab_case --lib
cargo test -p clanker-message tool_collision_policy_default_and_roundtrip_preserve_snake_case --lib
cargo test -p clankers-runtime remote_dependency_sync_fails_on_hash_mismatch_unsupported_version_and_secret_dependencies --lib
cargo test -p clankers-runtime effect_result_redacts_secret_markers_and_preserves_request_ref --lib
cargo test -p clankers-runtime tool_catalog_custom_tools_apply_collision_policy_matrix --lib
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
