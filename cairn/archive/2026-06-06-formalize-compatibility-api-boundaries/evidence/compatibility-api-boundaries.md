Artifact-Type: implementation-validation-evidence
Task-ID: I1,I2,I3,V1
Covers: sdk-message-contract-boundary.transcript-compat-feature.owner-fixtures
Status: complete

## Reviewed-Evidence

Compatibility inventory:

- `crates/clanker-message/src/lib.rs` keeps transcript records behind the non-default `transcript-compat` feature and avoids crate-root transcript reexports.
- `crates/clanker-message/src/message.rs` is documented as a legacy compatibility import module.
- `crates/clanker-message/src/transcript.rs` documents transcript records as Clankers session/provider/controller compatibility data, not generic embedded SDK message contracts.
- `docs/src/generated/embedded-sdk-api.md` labels stable, optional-support, and unsupported-internal message rows.
- `docs/src/tutorials/embedded-agent-sdk.md` documents the stable SDK subset and the explicit transcript compatibility feature/import boundary.

Commands run:

```text
scripts/check-message-contract-boundary.rs
ok: message contract boundary rail passed

scripts/check-provider-router-boundary.rs
ok: provider/router boundary rail passed
```

`check-message-contract-boundary.rs` also verifies that default green SDK examples and public green APIs do not consume transcript-internal tokens unless they explicitly opt into compatibility paths.

## Decision

Transcript and legacy message APIs remain compatibility surfaces. Default green SDK contracts use `content`, `contracts`, `streaming`, `tool_result`, and `semantic_event` data without transcript records.

## Follow-Up

Add the same owner/fixture metadata before promoting any future provider/session compatibility shim.
