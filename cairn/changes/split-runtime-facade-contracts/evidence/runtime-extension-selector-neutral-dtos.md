Artifact-Type: validation-log
Task-ID: I16,V15
Covers: r[remaining-coupling-drain.runtime-facade-contract-split.green-contracts], r[remaining-coupling-drain.runtime-facade-contract-split.docs], r[remaining-coupling-drain.runtime-facade-contract-split.validation], r[remaining-coupling-drain.runtime-facade-contract-split.closeout]
Status: pass

## Scope

Moved simple runtime extension selector enums to neutral message contracts:

- Added `clanker_message::AuthStoreOperation` for host auth-store operation labels.
- Added `clanker_message::ExtensionRuntimeKind` for plugin/MCP/gateway runtime kind labels.
- Re-exported both enums through `clankers-runtime::services` / crate root so existing runtime public API paths remain available.
- Kept `AuthStoreAccessRequest`, `ExtensionRuntimeRequest`, service traits, extension runtime execution, and host authority-bearing behavior in `clankers-runtime`; only reusable selector enums moved.
- Regenerated `docs/src/generated/runtime-facade-api.md` after the ownership change.

## Validation

Commands run from repository root:

```text
cargo check -p clanker-message -p clankers-runtime -p clankers --tests
cargo test -p clanker-message auth_store_operation_roundtrip_preserves_snake_case --lib
cargo test -p clanker-message extension_runtime_kind_roundtrip_preserves_snake_case --lib
cargo test -p clankers-runtime host_supplied_extension_services_are_explicit_capabilities --lib
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
