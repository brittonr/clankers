# Provider edge closeout validation evidence

Evidence-ID: sdk-provider-edge-boundary-closeout
Artifact-Type: command-output-summary
Task-ID: V3
Covers: sdk-provider-edge-boundary.verification,sdk-provider-edge-boundary.verification.literal-fixtures,sdk-provider-edge-boundary.verification.dependency-rails
Date: 2026-06-03
Status: PASS

## Commands

```text
TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= cargo nextest run -p clankers-provider inventory_names_owner_for_each_provider_router_concern
TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= cargo nextest run -p clankers-provider provider_request_shared_fields_match_inline_golden
TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= cargo nextest run -p clankers-provider provider_and_router_request_shared_schema_fields_stay_in_parity
TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= cargo nextest run -p clankers-provider builds_router_request_with_provider_native_message_json
./scripts/check-provider-router-boundary.rs
./scripts/check-provider-adapter-kit.rs
./scripts/check-embedded-sdk-deps.rs
TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= rustfmt --check crates/clankers-provider/src/provider_router_responsibility.rs scripts/check-provider-router-boundary.rs scripts/check-embedded-agent-sdk.rs
nix run .#cairn -- gate proposal sdk-provider-edge-boundary --root .
nix run .#cairn -- gate design sdk-provider-edge-boundary --root .
nix run .#cairn -- gate tasks sdk-provider-edge-boundary --root .
nix run .#cairn -- validate --root .
git diff --check
```

## Relevant output

```text
inventory_names_owner_for_each_provider_router_concern
PASS clankers-provider provider_router_responsibility::tests::inventory_names_owner_for_each_provider_router_concern
Summary: 1 test run: 1 passed, 179 skipped

provider_request_shared_fields_match_inline_golden
PASS clankers-provider tests::provider_request_shared_fields_match_inline_golden
Summary: 1 test run: 1 passed, 179 skipped

provider_and_router_request_shared_schema_fields_stay_in_parity
PASS clankers-provider tests::provider_and_router_request_shared_schema_fields_stay_in_parity
Summary: 1 test run: 1 passed, 179 skipped

builds_router_request_with_provider_native_message_json
PASS clankers-provider router_request_bridge::tests::builds_router_request_with_provider_native_message_json
Summary: 1 test run: 1 passed, 179 skipped

./scripts/check-provider-router-boundary.rs
ok: provider/router boundary rail passed

./scripts/check-provider-adapter-kit.rs
provider-adapter-kit receipt written to target/embedded-sdk-release/provider-adapter-kit-receipt.json

./scripts/check-embedded-sdk-deps.rs
ok: embedded SDK example dependency graph has 180 packages and excludes forbidden runtime crates

rustfmt --check ...
exit 0

nix run .#cairn -- gate proposal sdk-provider-edge-boundary --root .
"valid": true,
"verdict": "PASS"

nix run .#cairn -- gate design sdk-provider-edge-boundary --root .
"valid": true,
"verdict": "PASS"

nix run .#cairn -- gate tasks sdk-provider-edge-boundary --root .
"valid": true,
"verdict": "PASS"

nix run .#cairn -- validate --root .
"valid": true

git diff --check
exit 0
```

## Coverage notes

The closeout bundle covers the provider concern inventory, no-display-DTO rail, provider adapter fixture kit, provider/router request-shape parity, embedded SDK dependency checks, Cairn proposal/design/tasks gates, Cairn validation, formatting check, and whitespace check for `sdk-provider-edge-boundary`.
