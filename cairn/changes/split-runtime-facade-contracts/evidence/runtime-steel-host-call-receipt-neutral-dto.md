Artifact-Type: validation-log
Task-ID: I47,V46
Covers: r[remaining-coupling-drain.runtime-facade-contract-split.green-contracts], r[remaining-coupling-drain.runtime-facade-contract-split.docs], r[remaining-coupling-drain.runtime-facade-contract-split.validation], r[remaining-coupling-drain.runtime-facade-contract-split.closeout]
Status: pass

## Scope

Moved the reusable Steel host-call receipt entry DTO to neutral message contracts:

- Added `clanker_message::SteelHostCallReceipt` with the existing field shape and outcome DTO.
- Re-exported `SteelHostCallReceipt` through `clankers-runtime::steel_runtime` and the runtime crate root so existing runtime API paths remain available.
- Kept full Steel runtime receipts, deterministic fixture evaluation, receipt hashing, budget enforcement, and fail-closed policy in `clankers-runtime`.
- Regenerated `docs/src/generated/runtime-facade-api.md` after the ownership change.

## Validation

Commands run from repository root:

```text
TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= cargo check --tests -p clanker-message -p clankers-runtime -p clankers-agent -p clankers
TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= cargo test -p clanker-message --lib dynamic_runtime_selector_status_dtos_roundtrip_preserve_snake_case
TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= cargo test -p clankers-runtime --lib approved_host_function_requires_registration_and_capability
TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= cargo test -p clankers-runtime --lib denied_host_function_performs_no_fallback_effect
TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= scripts/check-runtime-facade-boundary.rs --write-inventory
TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= scripts/check-runtime-facade-boundary.rs
TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= scripts/check-message-contract-boundary.rs
TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= scripts/check-workspace-layering-rails.rs
TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= scripts/check-lego-architecture-boundaries.rs
TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= cargo test -p clankers-controller --test fcis_shell_boundaries
```

All listed commands exited 0.
