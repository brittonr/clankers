Artifact-Type: validation-log
Task-ID: I57,V56
Covers: r[remaining-coupling-drain.runtime-facade-contract-split.green-contracts], r[remaining-coupling-drain.runtime-facade-contract-split.docs], r[remaining-coupling-drain.runtime-facade-contract-split.validation], r[remaining-coupling-drain.runtime-facade-contract-split.closeout]
Status: pass

## Scope

Moved reusable dynamic runtime action and execution-envelope DTOs to neutral message contracts:

- Added `clanker_message::{DynamicRuntimeActionEnvelope, DynamicRuntimeAuthorizationContext, DynamicRuntimeActionReceipt, FakeSteelOrchestrationProfile, FakeSteelOrchestrationRequest, FakeSteelOrchestrationReceipt, WasmToolExecutionProfile, WasmToolExecutionRequest, WasmToolExecutionReceipt, CrossLayerFixtureReceipt, DYNAMIC_RUNTIME_ACTION_SCHEMA, DYNAMIC_RUNTIME_RECEIPT_SCHEMA}`.
- Re-exported those DTOs through `clankers-runtime::dynamic_runtime` and the runtime crate root so existing public runtime paths remain source-compatible.
- Kept dynamic runtime authorization, envelope validation, receipt hashing, Steel ambient-access fixtures, and fake Wasm/Steel execution policy in `clankers-runtime`.
- Regenerated `docs/src/generated/runtime-facade-api.md` after the ownership change.

## Validation

Commands run from repository root:

```text
TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= cargo test -p clanker-message --lib dynamic_runtime_action_and_wasm_contracts_roundtrip
TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= cargo test -p clankers-runtime --lib dynamic_runtime::tests::
TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= cargo check --tests -p clanker-message -p clankers-runtime -p clankers-agent -p clankers
TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= scripts/check-runtime-facade-boundary.rs --write-inventory
TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= scripts/check-runtime-facade-boundary.rs
TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= scripts/check-message-contract-boundary.rs
TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= scripts/check-workspace-layering-rails.rs
TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= scripts/check-lego-architecture-boundaries.rs
TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= cargo test -p clankers-controller --test fcis_shell_boundaries
```

All listed commands exited 0.
