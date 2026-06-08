Artifact-Type: validation-log
Task-ID: I27,V26
Covers: r[remaining-coupling-drain.runtime-facade-contract-split.green-contracts], r[remaining-coupling-drain.runtime-facade-contract-split.docs], r[remaining-coupling-drain.runtime-facade-contract-split.validation], r[remaining-coupling-drain.runtime-facade-contract-split.closeout]
Status: pass

## Scope

Moved dynamic-runtime selector/status DTOs to neutral message contracts:

- Added `clanker_message::DynamicRuntimeKind`, `DynamicRuntimeActionKind`, `DynamicRuntimeRedactionClass`, `DynamicRuntimeActionStatus`, `DynamicRuntimeActionReason`, and `WasmToolExecutionStatus`.
- Re-exported those DTOs through `clankers-runtime::dynamic_runtime` and the runtime crate root so existing runtime public API paths remain available.
- Kept action envelopes, authorization contexts, receipts, receipt hashing, Steel ambient access policy, fake Steel/Wasm execution fixtures, and all authorization behavior in `clankers-runtime`; only serde-friendly selector/status enums moved.
- Regenerated `docs/src/generated/runtime-facade-api.md` after the ownership change.

## Validation

Commands run from repository root:

```text
cargo check -p clanker-message -p clankers-runtime -p clankers --tests
cargo test -p clanker-message dynamic_runtime_selector_status_dtos_roundtrip_preserve_snake_case --lib
cargo test -p clankers-runtime steel_host_function_envelope_can_be_authorized_without_side_effects --lib
cargo test -p clankers-runtime fake_wasm_tool_executes_with_explicit_imports_and_budgets --lib
cargo test -p clankers-runtime steel_ambient_access_matrix_fails_before_host_effects --lib
scripts/check-runtime-facade-boundary.rs --write-inventory
scripts/check-runtime-facade-boundary.rs
cargo -q -Zscript scripts/check-runtime-facade-split.rs
scripts/check-message-contract-boundary.rs
scripts/check-lego-architecture-boundaries.rs
scripts/check-workspace-layering-rails.rs
git diff --check
cargo test -p clankers --no-run
nix run .#cairn -- gate tasks split-runtime-facade-contracts --root .
nix run .#cairn -- validate --root .
git diff --check
```

All listed commands exited 0.
