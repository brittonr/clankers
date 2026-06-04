# Runtime fail-closed service evidence

Evidence-ID: classify-runtime-sdk-facade.runtime-fail-closed-services
Artifact-Type: implementation-evidence
Task-ID: I4,V1,V2
Covers: remaining-coupling-drain.runtime-fail-closed-defaults, remaining-coupling-drain.runtime-fail-closed-defaults.no-ambient, remaining-coupling-drain.runtime-public-api-rail, remaining-coupling-drain.runtime-public-api-rail.leakage
Date: 2026-06-04
Status: PASS

## Service boundary

Runtime provider/auth/credential-pool/extension, prompt/skill/config, and provider-router service seams remain explicit host-injection contracts. Missing or disabled services return typed unavailable/unsupported errors and do not probe desktop config, auth files, daemon sockets, plugin roots, prompt/skill directories, or session stores.

## Command evidence

```text
scripts/check-runtime-extension-service-matrix.rs
runtime extension service matrix receipt written to target/embedded-sdk-release/runtime-extension-service-matrix-receipt.json
exit=0

scripts/check-config-prompt-skill-services.rs
config-prompt-skill-services receipt written to target/embedded-sdk-release/config-prompt-skill-services-receipt.json
exit=0

scripts/check-provider-router-boundary.rs
ok: provider/router boundary rail passed
exit=0
```

## Relevant fixtures exercised

- `runtime_extension_service_matrix_default_safe_fails_closed_independently`
- `runtime_extension_service_matrix_mixed_injected_absent_no_ambient_fallback`
- `runtime_extension_service_matrix_injected_error_receipts_are_redacted`
- `runtime_extension_service_matrix_safe_receipts_redact_success_denial_and_error`
- `config_prompt_skill_service_fixtures_cover_host_desktop_missing_and_redaction`
- `prompt_source_service_injection_is_used_by_runtime_assembly`
- desktop provider-router runtime service fixtures for retryable and terminal failures
