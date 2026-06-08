Artifact-Type: validation-log
Task-ID: I12,V11
Covers: r[remaining-coupling-drain.runtime-facade-contract-split.green-contracts], r[remaining-coupling-drain.runtime-facade-contract-split.docs], r[remaining-coupling-drain.runtime-facade-contract-split.validation], r[remaining-coupling-drain.runtime-facade-contract-split.closeout]
Status: pass

## Scope

Moved runtime tool-response DTO ownership to neutral message contracts:

- Added `clanker_message::RuntimeToolStatus` and `clanker_message::RuntimeToolResponse` next to the existing neutral content/tool contracts.
- Re-exported the DTOs through `clankers-runtime::adapters` / crate root so existing runtime public API paths remain available.
- Kept `RuntimeToolRequest` and the executable `RuntimeToolAdapter` trait in `clankers-runtime`; only reusable response/status data moved.
- Regenerated `docs/src/generated/runtime-facade-api.md`; the runtime host-adapter group now keeps the adapter trait/request while the reusable response DTO lives in the neutral message contract crate.

## Validation

Commands run from repository root:

```text
cargo check -p clanker-message -p clankers-runtime -p clankers --tests
cargo test -p clanker-message runtime_tool_response_failed_helper_preserves_message --lib
cargo test -p clanker-message runtime_tool_response_roundtrip_preserves_status_and_details --lib
cargo test -p clankers-runtime runtime_facade_tool_feedback_uses_engine_host_turn_loop --lib
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

All listed commands exited 0. An earlier attempted runtime test filter `runtime_facade_invokes_tool_adapter` matched 0 tests and was not used as evidence.
