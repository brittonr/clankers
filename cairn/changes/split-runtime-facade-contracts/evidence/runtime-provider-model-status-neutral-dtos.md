Artifact-Type: validation-log
Task-ID: I14,V13
Covers: r[remaining-coupling-drain.runtime-facade-contract-split.green-contracts], r[remaining-coupling-drain.runtime-facade-contract-split.docs], r[remaining-coupling-drain.runtime-facade-contract-split.validation], r[remaining-coupling-drain.runtime-facade-contract-split.closeout]
Status: pass

## Scope

Moved runtime provider model status/failure DTO ownership to neutral message contracts:

- Added `clanker_message::ProviderModelStatus` and `ProviderModelFailure` next to the neutral provider message/stream contracts.
- Preserved the `retryable(...)` / `terminal(...)` constructors and secret-marker redaction behavior in the neutral DTO implementation.
- Re-exported those DTOs through `clankers-runtime::services` / crate root so existing runtime public API paths remain available.
- Kept provider model request/response records, service traits, extension receipts, and executable host behavior in `clankers-runtime`; only reusable provider result status/failure data moved.
- Regenerated `docs/src/generated/runtime-facade-api.md` after the ownership change.

## Validation

Commands run from repository root:

```text
cargo check -p clanker-message -p clankers-runtime -p clankers --tests
cargo test -p clanker-message provider_model_failure_helpers_sanitize_and_mark_retryability --lib
cargo test -p clanker-message provider_model_status_roundtrip_preserves_snake_case --lib
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
