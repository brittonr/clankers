## Why

Clankers now has reusable `clankers-core`, `clankers-engine`, `clankers-engine-host`, and `clankers-tool-host` crates, and Clankers routes its own turns through them. The remaining gap is productization: another project can technically depend on the crates, but there is no documented, versioned, example-backed embedding path that proves a real external consumer can assemble an agent without importing Clankers shell/runtime internals.

## What Changes

- Add a documented embedded-agent SDK path that explains which crates embedders depend on, which adapter traits they implement, and which Clankers app concerns stay out of scope.
- Add at least one checked-in external-consumer example that runs a prompt through `clankers-engine-host` using fake or in-memory model/tool adapters without depending on `clankers-agent`, daemon, TUI, provider discovery, session DB, or prompt-assembly crates.
- Add API stability and crate-boundary checks for the embedding surface, including public API inventory, dependency denylist, feature/default-policy expectations, and docs/examples freshness.
- Add reusable adapter recipes for common embedding needs: model execution, tool execution, cancellation, retry sleeping, usage observation, event emission, and transcript conversion.
- Preserve existing Clankers behavior: `clankers-agent::Agent` remains the default Clankers assembly over the reusable engine and host crates.

## Non-Goals

- Do not move daemon protocol, TUI rendering, provider discovery, session DB ownership, built-in tool bundles, plugin supervision, or Clankers prompt assembly into the generic embedding crates.
- Do not remove or replace `clankers-agent::Agent`; it remains the default Clankers assembly over the reusable engine/host/tool crates.
- Do not promise network/provider integrations as part of the minimal SDK path; embedders wire those through host adapters.

## Capabilities

### New Capabilities

- None.

### Modified Capabilities

- `embeddable-agent-engine`: require a productized SDK surface, external-consumer examples, public API/docs checks, and an embedding acceptance bundle beyond the already-implemented reducer/host-runner internals.

## Impact

- **Crates**: `clankers-engine`, `clankers-engine-host`, `clankers-tool-host`, `clanker-message`, and adapter call sites in `clankers-agent`.
- **Docs/examples**: README/docs and at least one checked-in example or consumer fixture demonstrating external embedding.
- **Validation**: introduce `scripts/check-embedded-agent-sdk.sh` as the single acceptance bundle, backed by `scripts/check-llm-contract-boundary.sh`, `crates/clankers-controller/tests/fcis_shell_boundaries.rs`, docs/example tests, public API inventory checks, and artifact freshness checks.
- **APIs**: no intended breaking change to current Clankers runtime APIs; this change documents and tests the reusable embedding API as a supported surface.
