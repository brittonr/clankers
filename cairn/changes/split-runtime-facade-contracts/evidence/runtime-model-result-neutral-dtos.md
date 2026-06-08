Artifact-Type: validation-log
Task-ID: I26,V25
Covers: r[remaining-coupling-drain.runtime-facade-contract-split.green-contracts], r[remaining-coupling-drain.runtime-facade-contract-split.docs], r[remaining-coupling-drain.runtime-facade-contract-split.validation], r[remaining-coupling-drain.runtime-facade-contract-split.closeout]
Status: pass

## Scope

Moved small runtime model adapter result/metadata DTOs to neutral message contracts:

- Added `clanker_message::ModelRequestMetadata` for provider generation metadata carried with runtime model requests.
- Added `clanker_message::ModelFailure` for retryable/terminal model adapter failures.
- Re-exported those DTOs through `clankers-runtime::prompt` and the runtime crate root so existing runtime public API paths remain available.
- Kept `ModelRequest`, `ModelResponse`, `ModelAdapter`, prompt/session identifiers, session events, retry scheduling, and error projection behavior in `clankers-runtime`; only serde-friendly record ownership moved.
- Regenerated `docs/src/generated/runtime-facade-api.md` after the ownership change.

## Validation

Commands run from repository root:

```text
cargo check -p clanker-message -p clankers-runtime -p clankers --tests
cargo test -p clanker-message model_request_metadata_roundtrip_preserves_generation_settings --lib
cargo test -p clanker-message model_failure_helpers_preserve_retryability --lib
cargo test -p clankers-runtime runtime_facade_projects_model_failure_to_error_event --lib
cargo test -p clankers-runtime runtime_facade_retryable_model_failure_uses_retry_adapter --lib
scripts/check-runtime-facade-boundary.rs --write-inventory
scripts/check-runtime-facade-boundary.rs
cargo -q -Zscript scripts/check-runtime-facade-split.rs
scripts/check-message-contract-boundary.rs
scripts/check-lego-architecture-boundaries.rs
scripts/check-workspace-layering-rails.rs
git diff --check
cargo test -p clankers --no-run
nix run .#cairn -- gate tasks split-runtime-facade-contracts --root .
nix run .#cairn -- validate --root .
git diff --check
```

All listed commands exited 0.
