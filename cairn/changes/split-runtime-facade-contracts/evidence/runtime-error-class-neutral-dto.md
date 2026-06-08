Artifact-Type: validation-log
Task-ID: I17,V16
Covers: r[remaining-coupling-drain.runtime-facade-contract-split.green-contracts], r[remaining-coupling-drain.runtime-facade-contract-split.docs], r[remaining-coupling-drain.runtime-facade-contract-split.validation], r[remaining-coupling-drain.runtime-facade-contract-split.closeout]
Status: pass

## Scope

Moved runtime error-class DTO ownership to neutral message contracts:

- Added `clanker_message::ErrorClass` as the neutral serializable runtime error classification enum.
- Re-exported `ErrorClass` through `clankers-runtime::events` / crate root so existing runtime public API paths remain available.
- Kept `RuntimeError::class()`, semantic event projection, `ExtensionReceipt`, event metadata, and executable runtime behavior in `clankers-runtime`; only the reusable error classification enum moved.
- Regenerated `docs/src/generated/runtime-facade-api.md` after the ownership change.

## Validation

Commands run from repository root:

```text
cargo check -p clanker-message -p clankers-runtime -p clankers --tests
cargo test -p clanker-message error_class_roundtrip_preserves_snake_case --lib
cargo test -p clankers-runtime runtime_facade_projects_model_failure_to_error_event --lib
cargo test -p clankers-runtime runtime_extension_service_matrix_injected_error_receipts_are_redacted --lib
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
