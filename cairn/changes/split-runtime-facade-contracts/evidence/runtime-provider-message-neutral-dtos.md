Artifact-Type: validation-log
Task-ID: I13,V12
Covers: r[remaining-coupling-drain.runtime-facade-contract-split.green-contracts], r[remaining-coupling-drain.runtime-facade-contract-split.docs], r[remaining-coupling-drain.runtime-facade-contract-split.validation], r[remaining-coupling-drain.runtime-facade-contract-split.closeout]
Status: pass

## Scope

Moved runtime provider message/stream DTO ownership to neutral message contracts:

- Added `clanker_message::ProviderMessageRole`, `ProviderMessage`, and `ProviderStreamEvent` next to the existing neutral content/usage contracts.
- Re-exported those DTOs through `clankers-runtime::services` / crate root so existing runtime public API paths remain available.
- Kept provider service traits, requests/responses with runtime receipts, and executable service behavior in `clankers-runtime`; only reusable provider message and stream event data moved.
- Regenerated `docs/src/generated/runtime-facade-api.md`; runtime service rows now keep service behavior while neutral provider message/stream DTO ownership lives in the message contract crate.

## Validation

Commands run from repository root:

```text
cargo check -p clanker-message -p clankers-runtime -p clankers --tests
cargo test -p clanker-message provider_message_tool_result_preserves_call_metadata --lib
cargo test -p clanker-message provider_stream_event_usage_roundtrip_preserves_snake_case_type --lib
cargo test -p clankers-runtime provider_model_contract_literal_fixtures_cover_request_stream_failures_and_usage --lib
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
