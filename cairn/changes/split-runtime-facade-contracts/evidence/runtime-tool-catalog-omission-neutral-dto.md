Artifact-Type: validation-log
Task-ID: I56,V55
Covers: r[remaining-coupling-drain.runtime-facade-contract-split.green-contracts], r[remaining-coupling-drain.runtime-facade-contract-split.docs], r[remaining-coupling-drain.runtime-facade-contract-split.validation], r[remaining-coupling-drain.runtime-facade-contract-split.closeout]
Status: pass

## Scope

Moved the reusable tool-catalog omission receipt DTO to neutral message contracts:

- Added `clanker_message::ToolCatalogOmission` with the existing safe string sanitization constructor.
- Re-exported the DTO through `clankers-runtime::tools` so the runtime tool catalog API remains source-compatible.
- Kept capability-pack expansion, tool descriptor effect-class mapping, collision policy, and effect-handler routing in `clankers-runtime`.
- Regenerated `docs/src/generated/runtime-facade-api.md` after the ownership change.

## Validation

Commands run from repository root:

```text
TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= cargo test -p clanker-message --lib tool_catalog_omission_sanitizes_safe_reason_contract
TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= cargo check --tests -p clanker-message -p clankers-runtime -p clankers-agent -p clankers
TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= scripts/check-runtime-facade-boundary.rs --write-inventory
TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= scripts/check-runtime-facade-boundary.rs
TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= scripts/check-message-contract-boundary.rs
TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= scripts/check-workspace-layering-rails.rs
TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= scripts/check-lego-architecture-boundaries.rs
TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= cargo test -p clankers-controller --test fcis_shell_boundaries
```

All listed commands exited 0.
