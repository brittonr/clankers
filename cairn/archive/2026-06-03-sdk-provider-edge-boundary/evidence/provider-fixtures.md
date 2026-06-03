# Provider edge fixture evidence

Evidence-ID: sdk-provider-edge-boundary-fixtures
Artifact-Type: command-output-summary
Task-ID: V1
Covers: sdk-provider-edge-boundary.verification.literal-fixtures,sdk-provider-edge-boundary.concerns.duplicate-abstractions
Date: 2026-06-03
Status: PASS

## Commands

```text
TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= cargo nextest run -p clankers-provider provider_request_shared_fields_match_inline_golden
TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= cargo nextest run -p clankers-provider provider_and_router_request_shared_schema_fields_stay_in_parity
TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= cargo nextest run -p clankers-provider builds_router_request_with_provider_native_message_json
./scripts/check-provider-adapter-kit.rs
```

## Relevant output

```text
provider_request_shared_fields_match_inline_golden
PASS clankers-provider tests::provider_request_shared_fields_match_inline_golden
Summary: 1 test run: 1 passed, 179 skipped

provider_and_router_request_shared_schema_fields_stay_in_parity
PASS clankers-provider tests::provider_and_router_request_shared_schema_fields_stay_in_parity
Summary: 1 test run: 1 passed, 179 skipped

builds_router_request_with_provider_native_message_json
PASS clankers-provider router_request_bridge::tests::builds_router_request_with_provider_native_message_json
Summary: 1 test run: 1 passed, 179 skipped

./scripts/check-provider-adapter-kit.rs
provider-adapter-kit receipt written to target/embedded-sdk-release/provider-adapter-kit-receipt.json
```

## Coverage notes

The provider request shape tests compare against explicit inline JSON golden values rather than constructing expected values through the body builder. The embedded provider adapter kit consumes `examples/embedded-provider-adapter/fixtures/provider-adapter-fixtures.json`, which is authored fixture data and forbids live credentials, provider discovery, `clankers-provider`, and `clanker-router`.
