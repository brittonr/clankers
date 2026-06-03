# Design: Drain Agent Shell Coupling Behind SDK Ports

## Summary

The engine and engine-host crates are already the reusable turn core. The next SDK boundary is to make `clankers-agent` an application shell over those bricks instead of a place where provider, config, DB, hook, skill, and model-routing policy enter reusable turn logic.

## Current coupling points

- `crates/clankers-agent/src/lib.rs::Agent` owns `Arc<dyn clankers_provider::Provider>`, `Settings`, `Db`, `HookPipeline`, routing policy, cost tracker, tools, cancellation, and event broadcast.
- `crates/clankers-agent/src/turn/execution.rs` converts engine requests to provider-native `CompletionRequest` and still imports provider traits directly.
- Settings-to-turn policy conversion happens inside the agent crate instead of at the app edge.
- Hook status/error projection is owned by the agent shell rather than a host adapter.

## Decisions

### 1. Treat `clankers-agent` as yellow shell, not green SDK

The generic SDK remains `clanker-message`, `clankers-engine`, `clankers-engine-host`, `clankers-tool-host`, `clankers-adapters`, and optional `clankers-core`. This change does not advertise `clankers-agent`; it makes agent coupling explicit and drainable.

### 2. Ports are the migration unit

Model execution, tools, prompt/config sources, storage/search, hooks, skills, cost, and cancellation should be represented as explicit ports or service bundles. Concrete desktop implementations live in root/daemon/runtime adapters.

### 3. Compatibility constructor stays edge-owned

Existing desktop code may keep building an `Agent` with concrete services during migration. The constructor must be documented and railed as an app-edge adapter, not a reusable SDK API.

## Validation plan

- Add a source inventory for concrete `clankers-agent` dependency families and adapter owners.
- Add one focused migration that removes or narrows a concrete dependency from reusable turn modules.
- Preserve default prompt/tool/stream behavior through parity tests.
- Update `policy/lego-architecture/dependency-ownership-baseline.json` and SDK docs if the public boundary changes.
