## Why

`clankers-engine` is intended to be the reusable embeddable turn engine, but its current dependency surface still pulls Clankers runtime concerns through shared type ownership. `clanker-message` depends on `clanker-router`, `clankers-engine` depends on `clankers-provider` for `ThinkingConfig`, and engine prompt submission accepts `AgentMessage`. That makes the engine hard to embed in smaller agents and risks dragging Tokio, network, router DB, auth, and provider implementation details into the nominally reusable layer.

This change cleans the lowest-level contract surface before extracting larger async host runners or tool hosts.

## What Changes

- Move generic LLM contract ownership into `clanker-message` so router/provider code depends on the message contract rather than the message contract depending on router runtime crates.
- Keep `clanker-router` and `clankers-provider` compatibility aliases/re-exports where downstream code expects those names; they must resolve to the canonical `clanker-message` definitions, not wrapper/newtype identities.
- Remove direct `clanker-router` and `clankers-provider` dependencies from `clankers-engine` by using engine/message-native request types.
- Change engine prompt submission to accept `Vec<EngineMessage>` rather than `Vec<AgentMessage>`; move Clankers transcript filtering into the `clankers-agent::turn` adapter seam before engine submission.
- Add cargo-tree and source-inventory rails that fail if engine or message contracts regain provider/router/runtime dependencies.

## Non-Goals

- Do not extract the async engine host runner, tool host, or stream accumulator in this change.
- Do not move built-in tools, WASM/stdio plugin supervision, daemon protocol conversion, TUI rendering, session storage, or system-prompt assembly.
- Do not introduce wrapper/newtype identities for moved router/provider compatibility types; compatibility names remain aliases/re-exports of canonical `clanker-message` types.
- Do not intentionally change provider wire JSON shape while moving type ownership.

## Capabilities

### Modified Capabilities

- `embeddable-agent-engine`: tightens the engine contract so embedders can consume the turn reducer without router/provider/runtime dependencies.

## Impact

- **Crates**: `clanker-message`, `clanker-router`, `clankers-provider`, `clankers-engine`, `clankers-agent`, boundary scripts/tests.
- **APIs**: Canonical ownership for `Usage`, `ToolDefinition`, `ThinkingConfig`, stream metadata/deltas, and engine prompt submission changes.
- **Compatibility**: All current workspace call sites using the existing router/provider public contract paths should continue to compile; compatibility paths are aliases/re-exports, not duplicate wrapper types.
- **Testing**: New positive/negative contract tests plus dependency-boundary checks over `cargo tree` and non-test source imports. Compatibility tests pin inline golden JSON values for moved contracts and provider request shapes.
