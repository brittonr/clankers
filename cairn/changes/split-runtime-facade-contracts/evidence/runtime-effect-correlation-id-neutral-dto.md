Artifact-Type: validation-log
Task-ID: I42,V41
Covers: r[remaining-coupling-drain.runtime-facade-contract-split.green-contracts], r[remaining-coupling-drain.runtime-facade-contract-split.docs], r[remaining-coupling-drain.runtime-facade-contract-split.validation], r[remaining-coupling-drain.runtime-facade-contract-split.closeout]
Status: pass

## Scope

Moved effect correlation ID ownership and cleaned the adjacent green contract boundary:

- Added `clanker_message::EffectCorrelationId` as the neutral serializable request/result/receipt correlation identifier.
- Re-exported `EffectCorrelationId` through `clankers-runtime::effects` and the runtime crate root so existing runtime effect envelopes keep their stable API path.
- Kept opaque ID minting authority out of `clanker-message`: the neutral DTO accepts host-supplied strings and deterministic replay IDs but does not depend on `uuid`.
- Kept chrono timestamp conversion at the runtime/root shell edge by moving process-job public contracts to `ProcessJobTimestamp`, removing the production `chrono` dependency from `clankers-tool-host`, and preserving the runtime `process_job_timestamp(DateTime<Utc>)` compatibility helper.
- Regenerated `docs/src/generated/runtime-facade-api.md` after the ownership changes.

## Validation

Commands run from repository root:

```text
TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= cargo check --tests -p clanker-message -p clankers-tool-host -p clankers-runtime -p clankers-ucan -p clankers
TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= cargo test -p clanker-message --lib effect_correlation_id_is_stable_for_replay_and_serialization
TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= cargo test -p clankers-runtime --lib effect_request_carries_policy_metadata_and_hash_dependencies
TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= cargo test -p clankers-runtime --lib fake_backend_contract_covers_projection_and_mutations
TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= cargo test -p clankers-runtime --lib native_log_layout_is_append_only_bounded_and_safe
TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= cargo test -p clankers-tool-host --lib backend_status_contract_preserves_backend_ref_status_and_logs
TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= cargo test -p clankers-tool-host --lib log_retention_policy_projects_safe_log_reference_without_host_io
TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= scripts/check-message-contract-boundary.rs
TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= scripts/check-runtime-facade-boundary.rs --write-inventory
TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= scripts/check-runtime-facade-boundary.rs
TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= scripts/check-workspace-layering-rails.rs
TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= scripts/check-lego-architecture-boundaries.rs
TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= cargo test -p clankers-controller --test fcis_shell_boundaries
rustfmt --check crates/clanker-message/src/contracts.rs crates/clankers-runtime/src/effects.rs crates/clankers-runtime/src/process_jobs.rs crates/clankers-tool-host/src/process_jobs.rs
nix run .#cairn -- gate tasks split-runtime-facade-contracts --root .
nix run .#cairn -- validate --root .
git diff --check
```

All listed commands exited 0.
