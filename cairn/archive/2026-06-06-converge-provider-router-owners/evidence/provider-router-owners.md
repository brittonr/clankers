Artifact-Type: implementation-validation-evidence
Task-ID: I1,I2,I3,V1
Covers: remaining-coupling-drain.provider-router-convergence.concern-owner-map, remaining-coupling-drain.provider-router-convergence.adapter-delegation
Status: complete

## Reviewed-Evidence

Provider/router concern ownership:

- `crates/clankers-provider/src/provider_router_responsibility.rs` inventories provider-router concern ownership.
- `crates/clankers-provider/src/router_request_bridge.rs` owns compatibility request and cache-key projection helpers for routed providers.
- `crates/clankers-provider/src/router.rs` and `crates/clankers-provider/src/rpc_provider.rs` delegate message projection to the bridge instead of duplicating provider-native body shaping.
- `policy/lego-architecture/dependency-ownership-baseline.json` records `provider_router_bridge.local_adapter_duplicate_message_projection = 0`, `rpc_adapter_duplicate_message_projection = 0`, and `local_adapter_duplicate_cache_key_message_projection = 0` for the selected cache-key/request projection concern.

Commands run:

```text
scripts/check-provider-router-boundary.rs
ok: provider/router boundary rail passed

scripts/check-runtime-extension-service-matrix.rs
router_request_bridge::tests::preserves_branch_and_compaction_summaries_as_user_context ... ok
router_request_bridge::tests::builds_router_request_with_provider_native_message_json ... ok
router_request_bridge::tests::cache_key_uses_router_message_projection_literal ... ok
```

Stream normalization was not touched in this drain pass, so no additional parser-entrypoint seam test was required.

## Decision

The selected duplicated concern is compatibility request/cache-key projection. `clankers-provider` adapters now delegate through the shared bridge and remain DTO/error/event translators for this concern.

## Follow-Up

Future provider work should add literal fixtures for any new request fields and runtime parser-entrypoint tests for touched stream normalization paths.
