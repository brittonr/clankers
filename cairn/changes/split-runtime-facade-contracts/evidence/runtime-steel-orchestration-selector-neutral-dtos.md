Artifact-Type: validation-log
Task-ID: I48,V47
Covers: r[remaining-coupling-drain.runtime-facade-contract-split.green-contracts], r[remaining-coupling-drain.runtime-facade-contract-split.docs], r[remaining-coupling-drain.runtime-facade-contract-split.validation], r[remaining-coupling-drain.runtime-facade-contract-split.closeout]
Status: pass

## Scope

Moved reusable Steel turn orchestration selector and status DTOs to neutral message contracts:

- Added `clanker_message::{OrchestrationRolloutStage, OrchestrationFallbackMode, OrchestrationPlannerKind, OrchestrationPlanStatus, OrchestrationIssueCode, RustNativeFallbackStatus, SteelTurnPlanningAuthorityStatus, SteelTurnPlanningAuthorityReason, SteelTurnExecutionStatus}`.
- Re-exported those DTOs through `clankers-runtime::steel_orchestration` and the runtime crate root so existing runtime API paths remain available.
- Kept full Steel planning/execution receipts, host-call payloads, UCAN/Basalt authority checks, deterministic receipt hashing, and fallback policy in `clankers-runtime`.
- Regenerated `docs/src/generated/runtime-facade-api.md` after the ownership change.

## Validation

Commands run from repository root:

```text
TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= cargo test -p clanker-message --lib dynamic_runtime_selector_status_dtos_roundtrip_preserve_snake_case
TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= cargo check --tests -p clanker-message -p clankers-runtime -p clankers-agent -p clankers
TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= scripts/check-runtime-facade-boundary.rs --write-inventory
TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= scripts/check-runtime-facade-boundary.rs
TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= scripts/check-message-contract-boundary.rs
TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= cargo test -p clankers-runtime --lib steel_plan_is_default_but_effects_cross_dynamic_runtime_authorization
TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= cargo test -p clankers-runtime --lib execute_turn_authority_requires_execution_capability_and_ucan
TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= scripts/check-workspace-layering-rails.rs
TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= scripts/check-lego-architecture-boundaries.rs
TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= cargo test -p clankers-controller --test fcis_shell_boundaries
nix run .#cairn -- gate tasks split-runtime-facade-contracts --root .
nix run .#cairn -- validate --root .
nix run .#cairn -- gate proposal split-runtime-facade-contracts --root .
nix run .#cairn -- gate design split-runtime-facade-contracts --root .
git diff --check
```

All listed commands exited 0.
