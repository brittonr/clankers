Artifact-Type: validation-log
Task-ID: I11,V10
Covers: r[remaining-coupling-drain.runtime-facade-contract-split.green-contracts], r[remaining-coupling-drain.runtime-facade-contract-split.docs], r[remaining-coupling-drain.runtime-facade-contract-split.validation], r[remaining-coupling-drain.runtime-facade-contract-split.closeout]
Status: pass

## Scope

Moved runtime retry-request DTO ownership to neutral message contracts:

- Added `clanker_message::RuntimeRetryRequest` with the existing delay clamp behavior.
- Re-exported `RuntimeRetryRequest` through `clankers-runtime::adapters` / crate root so existing runtime public API paths remain available.
- Regenerated `docs/src/generated/runtime-facade-api.md`; the runtime host-adapter group now keeps retry adapter behavior while the reusable retry request DTO lives in the neutral message contract crate.

## Validation

Commands run from repository root:

```text
cargo check -p clanker-message -p clankers-runtime -p clankers --tests
cargo test -p clanker-message runtime_retry_request_roundtrip_preserves_delay --lib
cargo test -p clanker-message runtime_retry_request_clamps_large_delays --lib
cargo test -p clankers-runtime runtime_facade_invokes_event_and_usage_adapter_slots --lib
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
