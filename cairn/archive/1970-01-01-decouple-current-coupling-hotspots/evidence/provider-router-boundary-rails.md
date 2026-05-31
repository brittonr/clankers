# Provider/router boundary rail evidence

Evidence-ID: provider-router-boundary-rails
Artifact-Type: command-output-summary
Task-ID: V7
Covers: coupling-hotspot-remediation.provider-router-boundary
Date: 2026-05-31
Status: PASS

## Commands

```text
./scripts/check-provider-router-boundary.rs
TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= cargo test -p clankers-provider --lib router_request_bridge
TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= cargo test -p clankers-provider --lib test_compat_adapter_uses_provider_native_message_json_for_representative_history
TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= cargo test -p clankers-provider --lib test_missing_openai_codex_prefix_fails_closed
```

## Relevant output

```text
ok: provider/router boundary rail passed

running 2 tests
test router_request_bridge::tests::preserves_branch_and_compaction_summaries_as_user_context ... ok
test router_request_bridge::tests::builds_router_request_with_provider_native_message_json ... ok

test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 175 filtered out

running 1 test
test router::tests::test_compat_adapter_uses_provider_native_message_json_for_representative_history ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 176 filtered out

running 1 test
test router::tests::test_missing_openai_codex_prefix_fails_closed ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 176 filtered out
```

## Coverage notes

The static rail requires `crates/clankers-provider/src/router_request_bridge.rs` to be the single runtime constructor of `clanker_router::CompletionRequest` and to own provider-native message JSON conversion for user, assistant, tool-result, branch-summary, and compaction-summary history. It also checks `RouterCompatAdapter` and the RPC provider delegate through that bridge, while `RouterProvider` owns fallback, cooldown, explicit-prefix fail-closed, and retryability selection instead of compatibility adapters duplicating routing policy.

The request-shape tests pin literal expected router message JSON, and the adapter fixture captures the real routed request through a fake `clanker_router::Provider`. The Codex prefix fixture proves known-but-unavailable routed prefixes fail closed instead of silently falling back to another provider.
