Artifact-Type: validation-log
Task-ID: I10,V9
Covers: r[remaining-coupling-drain.runtime-facade-contract-split.green-contracts], r[remaining-coupling-drain.runtime-facade-contract-split.docs], r[remaining-coupling-drain.runtime-facade-contract-split.validation], r[remaining-coupling-drain.runtime-facade-contract-split.closeout]
Status: pass

## Scope

Moved runtime usage-observation DTO ownership to neutral message contracts:

- Added `clanker_message::RuntimeUsageObservation` and `clanker_message::RuntimeUsageObservationKind` next to the existing neutral `Usage` contract.
- Re-exported those DTOs through `clankers-runtime::adapters` / crate root so existing runtime public API paths remain available.
- Regenerated `docs/src/generated/runtime-facade-api.md`; the runtime host-adapter group now only exposes the usage adapter trait/method while the reusable DTO lives in the neutral message contract crate.

## Validation

Commands run from repository root:

```text
cargo check -p clanker-message -p clankers-runtime -p clankers --tests
cargo test -p clanker-message runtime_usage_observation_roundtrip_preserves_kind_and_usage --lib
cargo test -p clankers-runtime runtime_facade_invokes_event_and_usage_adapter_slots --lib
scripts/check-runtime-facade-boundary.rs --write-inventory
scripts/check-runtime-facade-boundary.rs
cargo -q -Zscript scripts/check-runtime-facade-split.rs
scripts/check-lego-architecture-boundaries.rs
scripts/check-workspace-layering-rails.rs
nix run .#cairn -- gate tasks split-runtime-facade-contracts --root .
nix run .#cairn -- validate --root .
git diff --check
```

All listed commands exited 0. An earlier attempted runtime test filter `runtime_usage` matched 0 tests and was not used as evidence.
