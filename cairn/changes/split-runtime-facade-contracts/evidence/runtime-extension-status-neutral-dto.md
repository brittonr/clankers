Artifact-Type: validation-log
Task-ID: I15,V14
Covers: r[remaining-coupling-drain.runtime-facade-contract-split.green-contracts], r[remaining-coupling-drain.runtime-facade-contract-split.docs], r[remaining-coupling-drain.runtime-facade-contract-split.validation], r[remaining-coupling-drain.runtime-facade-contract-split.closeout]
Status: pass

## Scope

Moved runtime extension status DTO ownership to neutral message contracts:

- Added `clanker_message::ExtensionStatus` as the neutral serializable extension execution status enum.
- Re-exported `ExtensionStatus` through `clankers-runtime::services` / crate root so existing runtime public API paths remain available.
- Kept `ExtensionReceipt`, extension service traits, extension runtime requests, event metadata, and side-effect policy in `clankers-runtime`; only the reusable status enum moved.
- Regenerated `docs/src/generated/runtime-facade-api.md` after the ownership change.

## Validation

Commands run from repository root:

```text
cargo check -p clanker-message -p clankers-runtime -p clankers --tests
cargo test -p clanker-message extension_status_roundtrip_preserves_snake_case --lib
cargo test -p clankers-runtime disabled_extension_services_fail_closed_without_startup_side_effects --lib
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
