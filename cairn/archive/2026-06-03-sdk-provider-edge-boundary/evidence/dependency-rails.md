# Provider edge dependency rail evidence

Evidence-ID: sdk-provider-edge-boundary-dependency-rails
Artifact-Type: command-output-summary
Task-ID: V2
Covers: sdk-provider-edge-boundary.verification.dependency-rails,sdk-provider-edge-boundary.neutral-model-api.no-display-dtos,sdk-provider-edge-boundary.neutral-model-api.sdk-host-owned
Date: 2026-06-03
Status: PASS

## Commands

```text
./scripts/check-provider-router-boundary.rs
./scripts/check-provider-adapter-kit.rs
./scripts/check-embedded-sdk-deps.rs
TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= cargo nextest run -p clankers-provider inventory_names_owner_for_each_provider_router_concern
```

## Relevant output

```text
./scripts/check-provider-router-boundary.rs
ok: provider/router boundary rail passed

./scripts/check-provider-adapter-kit.rs
provider-adapter-kit receipt written to target/embedded-sdk-release/provider-adapter-kit-receipt.json

./scripts/check-embedded-sdk-deps.rs
ok: embedded SDK example dependency graph has 180 packages and excludes forbidden runtime crates

inventory_names_owner_for_each_provider_router_concern
PASS clankers-provider provider_router_responsibility::tests::inventory_names_owner_for_each_provider_router_concern
Summary: 1 test run: 1 passed, 179 skipped
```

## Coverage notes

The provider/router boundary rail now rejects display/protocol DTO imports in `clankers-provider`, rejects direct router request construction outside the bridge, rejects direct `AgentMessage` serialization for router requests, and anchors the active `sdk-provider-edge-boundary` requirement markers. The embedded provider adapter and SDK dependency rails keep generic SDK examples free of provider/router/auth/discovery dependencies.
