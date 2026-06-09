Artifact-Type: validation-log
Task-ID: I50,V49
Covers: r[remaining-coupling-drain.runtime-facade-contract-split.green-contracts], r[remaining-coupling-drain.runtime-facade-contract-split.docs], r[remaining-coupling-drain.runtime-facade-contract-split.validation], r[remaining-coupling-drain.runtime-facade-contract-split.closeout]
Status: pass

## Scope

Moved reusable Steel self-mutation selector and status DTOs to neutral message contracts:

- Added `clanker_message::{SteelMutationPatchFormat, SteelMutationUcanExpiryStatus, SteelMutationDecisionOutcome, SteelMutationReasonCode, SteelMutationHostPreflightStatus, SteelMutationHostPreflightReason, SteelMutationApplyStatus, SteelMutationApplyReason, SteelMutationVerificationStatus, SteelMutationRollbackStatus, SteelMutationRollbackReason}`.
- Re-exported those DTOs through `clankers-runtime::steel_mutation` and the runtime crate root so existing runtime API paths remain available.
- Kept mutation policy parsing, path/UCAN authorization, host preflight, apply/rollback shell logic, verification, backup handling, and receipt hashing in `clankers-runtime`.
- Regenerated `docs/src/generated/runtime-facade-api.md` after the ownership change.

## Validation

Commands run from repository root:

```text
TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= cargo test -p clanker-message --lib steel_mutation_selector_status_dtos_roundtrip_preserve_wire_case
TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= cargo check --tests -p clanker-message -p clankers-runtime -p clankers-agent -p clankers
TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= cargo test -p clankers-runtime --lib steel_mutation::tests::
TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= scripts/check-runtime-facade-boundary.rs --write-inventory
TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= scripts/check-runtime-facade-boundary.rs
TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= scripts/check-message-contract-boundary.rs
TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= scripts/check-workspace-layering-rails.rs
TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= scripts/check-lego-architecture-boundaries.rs
TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= cargo test -p clankers-controller --test fcis_shell_boundaries
nix run .#cairn -- gate tasks split-runtime-facade-contracts --root .
nix run .#cairn -- validate --root .
git diff --check
```

All listed commands exited 0.
