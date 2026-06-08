Artifact-Type: validation-log
Task-ID: I22,V21
Covers: r[remaining-coupling-drain.runtime-facade-contract-split.green-contracts], r[remaining-coupling-drain.runtime-facade-contract-split.docs], r[remaining-coupling-drain.runtime-facade-contract-split.validation], r[remaining-coupling-drain.runtime-facade-contract-split.closeout]
Status: pass

## Scope

Moved simple extension-service request DTOs to neutral message contracts:

- Added `clanker_message::AuthStoreAccessRequest` for host auth-store access requests.
- Added `clanker_message::CredentialPoolRequest` for host credential-pool policy requests.
- Added `clanker_message::ExtensionRuntimeRequest` for host extension runtime execution requests.
- Re-exported those DTOs through `clankers-runtime::services` / crate root so existing runtime public API paths remain available.
- Kept extension service traits, receipts, runtime tool descriptors, event metadata, capability reporting, and executable extension behavior in `clankers-runtime`; only reusable request record ownership moved.
- Regenerated `docs/src/generated/runtime-facade-api.md` after the ownership change.

## Validation

Commands run from repository root:

```text
cargo check -p clanker-message -p clankers-runtime -p clankers --tests
cargo test -p clanker-message auth_store_access_request_roundtrip_preserves_operation --lib
cargo test -p clanker-message credential_pool_request_roundtrip_preserves_strategy --lib
cargo test -p clanker-message extension_runtime_request_defaults_missing_arguments_to_null --lib
cargo test -p clankers-runtime host_supplied_extension_services_are_explicit_capabilities --lib
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
