## Why

The OpenAI Codex backend combines entitlement probing, auth selection, Responses API request shaping, streaming normalization, retry/error policy, and tests in one volatile provider file. This change turns the decomposition finding into an implementation-ready OpenSpec so the drain can proceed without rediscovering scope.

## What Changes

- Decompose `crates/clanker-router/src/backends/openai_codex.rs` into named modules around stable functional-core / imperative-shell boundaries.
- Preserve current public API, command behavior, receipts, metadata redaction, and regression coverage.
- Add focused parity/negative tests before broad cleanup.

## Capabilities

### Modified Capabilities
- `openai-codex-responses`: Adds a decomposition requirement for the current implementation seam.

## Impact

- **Files**: `crates/clanker-router/src/backends/openai_codex.rs` plus new sibling modules/tests as needed.
- **APIs**: Existing external APIs should remain source-compatible unless the design explicitly documents a compatibility alias.
- **Dependencies**: No new runtime dependency is expected for the decomposition itself.
- **Testing**: Targeted nextest filters for the seam, `cargo check --tests` for touched crates, strict OpenSpec validation, and `git diff --check`.
