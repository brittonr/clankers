Artifact-Type: implementation-validation-evidence
Task-ID: I1,I2,I3,V1
Covers: remaining-coupling-drain.runtime-facade-classification.adapter-shell-buckets
Status: complete

## Reviewed-Evidence

Runtime facade classification:

- `policy/embedded-lego/runtime-facade-boundary.json` classifies `clankers-runtime` as a yellow application-edge composition facade, not a generic green SDK crate.
- Runtime groups are bucketed by owner and classification, including `runtime-host-adapters`, `runtime-confirmation`, `runtime-dynamic-execution`, `runtime-effects`, `runtime-process-jobs`, `runtime-prompt-services`, `runtime-session-shell`, `runtime-host-services`, `runtime-steel-orchestration`, and `runtime-tool-catalog`.
- `docs/src/generated/runtime-facade-api.md` is generated from actual public runtime exports and carries owner/classification/stability rows.
- `docs/src/tutorials/embedded-agent-sdk.md` documents `clankers-runtime` as yellow app-edge composition and directs generic SDK users to green crates directly.

Commands run:

```text
scripts/check-runtime-facade-boundary.rs
ok: runtime facade boundary inventories clankers-runtime public API and dependency classifications

scripts/check-runtime-extension-service-matrix.rs
runtime_extension_service_matrix_default_safe_fails_closed_independently ... ok
runtime_extension_service_matrix_mixed_injected_absent_no_ambient_fallback ... ok
runtime_extension_service_matrix_injected_error_receipts_are_redacted ... ok
runtime_extension_service_matrix_safe_receipts_redact_success_denial_and_error ... ok
provider_model_contract_literal_fixtures_cover_request_stream_failures_and_usage ... ok
runtime_services::tests::desktop_runtime_provider_router_* ... 5 passed
router_request_bridge::tests::* ... 3 passed
runtime extension service matrix receipt written to target/embedded-sdk-release/runtime-extension-service-matrix-receipt.json
```

## Decision

Runtime remains a yellow facade. Adapter-heavy runtime service groups are explicitly labeled as host-injection or app-edge adapter surfaces so desktop defaults cannot be mistaken for green SDK contracts.

## Follow-Up

Future runtime API additions must add or update their group classification and rerun `scripts/check-runtime-facade-boundary.rs` before being documented as embedding entrypoints.
