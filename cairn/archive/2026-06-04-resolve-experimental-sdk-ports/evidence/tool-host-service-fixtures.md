# Tool-host service fixture evidence

Evidence-ID: resolve-experimental-sdk-ports.tool-host-service-fixtures
Artifact-Type: command-output-summary
Task-ID: I3,V1
Covers: neutral-tool-context.supported-service-ports, neutral-tool-context.supported-service-ports.fixtures, neutral-tool-context.supported-service-ports.docs
Date: 2026-06-04
Status: PASS

## Promoted service/context APIs

`clankers-tool-host` neutral service/context APIs are promoted from experimental to supported in `docs/src/generated/embedded-sdk-api.md` after deterministic fixtures exercised them through public APIs:

- `ToolInvocationContext`, `NeutralToolExecutor`, `ToolHostServices`, and `ToolHostServiceHandle`
- storage/search/hook/progress/capability/cancellation/runtime-policy service traits and DTOs
- cancellation and runtime-policy denial DTOs

The SDK guide now documents that hosts must explicitly inject these services, and missing/unavailable services fail closed rather than constructing desktop defaults.

## Commands

```text
env TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= cargo check -p clankers-engine-host -p clankers-tool-host
env TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= cargo test -p clankers-tool-host --lib neutral
scripts/check-engine-host-feature-matrix.rs
scripts/check-tool-catalog-matrix.rs
```

## Relevant output

```text
cargo check -p clankers-engine-host -p clankers-tool-host
Finished `dev` profile
exit=0

cargo test -p clankers-tool-host --lib neutral
running 4 tests
test tests::neutral_context_redacts_secret_progress_and_metadata ... ok
test tests::neutral_hook_service_fixtures_cover_continue_modify_and_deny ... ok
test tests::neutral_context_fixtures_cover_success_progress_storage_denial_cancel_and_truncation ... ok
test tests::neutral_service_contracts_cover_storage_search_hooks_capability_cancellation_and_runtime_policy ... ok
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 10 filtered out
exit=0

scripts/check-engine-host-feature-matrix.rs
exit=0

scripts/check-tool-catalog-matrix.rs
exit=0
```

## Fixture coverage

The neutral service fixtures cover:

- positive storage read/write and search results through public service traits;
- hook continue/modify/deny decisions;
- capability denial, cancellation state, and runtime-policy denial;
- progress emission and redaction of secret-adjacent progress/metadata;
- missing storage service fail-closed behavior through `ToolInvocationContext::require_service`;
- truncation behavior after a required service is available.
