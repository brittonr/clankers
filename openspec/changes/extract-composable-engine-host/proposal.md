## Why

After the engine contract surface is clean, the next blocker to embedding is that the reusable async execution shell still lives inside `clankers-agent::turn`. Hosts can reuse the reducer, but not the full prompt → model stream → tool execution → retry sleep → event sink loop without depending on the full Clankers agent runtime, hooks, DB, capability gates, model selection, and built-in tools.

This change extracts a composable host layer and tool-host surface so other crates can assemble full agents from engine, router/provider, tools, plugins, and session storage without importing the monolithic Clankers agent crate.

## What Changes

- Add an engine host layer with traits for model execution, tool execution, retry sleeping, event emission, cancellation, and usage observation.
- Move reusable async turn-driving logic out of `clankers-agent` into that host layer while preserving Clankers-specific behavior through adapters.
- Extract a reusable tool-host surface for tool catalogs, tool execution, result accumulation/truncation, capability checks, hook seams, and plugin-backed tool adapters.
- Move deterministic stream accumulation into a reusable module with positive and negative tests.
- Leave built-in Clankers tool bundles, system prompt assembly, session persistence policy, daemon protocol, and TUI behavior in Clankers shells.

## Capabilities

### Modified Capabilities

- `embeddable-agent-engine`: extends the clean reducer contract into a composable async host that can build full agents from independent crates.

## Impact

- **Crates**: new or refactored `clankers-engine-host` and `clankers-tool-host`, plus `clankers-agent`, `clankers-plugin`, `clankers-provider`, and tests.
- **APIs**: new host traits and adapter implementations; existing `Agent` API should remain as the default Clankers assembly.
- **Testing**: adapter parity tests must prove the extracted host preserves streaming, retries, tools, cancellation, usage updates, hooks, and event ordering.
