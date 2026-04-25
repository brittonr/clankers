## Why

`clankers-engine` is intended to be the reusable embeddable turn engine, but its current dependency surface still pulls Clankers runtime concerns through shared type ownership. `clanker-message` depends on `clanker-router`, `clankers-engine` depends on `clankers-provider` for `ThinkingConfig`, and engine prompt submission accepts `AgentMessage`. That makes the engine hard to embed in smaller agents and risks dragging Tokio, network, router DB, auth, and provider implementation details into the nominally reusable layer.

This change cleans the lowest-level contract surface before extracting larger async host runners or tool hosts.

## What Changes

- Move generic LLM contract ownership into `clanker-message` so router/provider code depends on the message contract rather than the message contract depending on router runtime crates.
- Keep `clanker-router` and `clankers-provider` compatibility re-exports where downstream code expects those names, but make them wrappers around the canonical `clanker-message` definitions.
- Remove direct `clanker-router` and `clankers-provider` dependencies from `clankers-engine` by using engine/message-native request types.
- Change engine prompt submission to accept `Vec<EngineMessage>` rather than `Vec<AgentMessage>`; move Clankers transcript filtering into adapter code.
- Add cargo-tree and source-inventory rails that fail if engine or message contracts regain provider/router/runtime dependencies.

## Capabilities

### Modified Capabilities

- `embeddable-agent-engine`: tightens the engine contract so embedders can consume the turn reducer without router/provider/runtime dependencies.

## Impact

- **Crates**: `clanker-message`, `clanker-router`, `clankers-provider`, `clankers-engine`, `clankers-agent`, boundary scripts/tests.
- **APIs**: Canonical ownership for `Usage`, `ToolDefinition`, `ThinkingConfig`, stream metadata/deltas, and engine prompt submission changes.
- **Compatibility**: Provider/router compatibility re-exports should preserve most call sites while type ownership moves.
- **Testing**: New positive/negative contract tests plus dependency-boundary checks over `cargo tree` and non-test source imports.
