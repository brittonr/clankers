## Verification Summary

This change is ready for implementation when the delta spec, this design, and tasks validate; implementation is done only after cargo-tree rails prove `clankers-engine` and `clanker-message` no longer pull provider/router/runtime dependencies, and adapter tests prove Clankers transcript conversion preserves current behavior.

## Context

The current engine reducer is a good first executable slice, but the type graph still couples it to runtime crates. `clanker-message` re-exports router stream/usage types, `clankers-engine` imports `ToolDefinition` from `clanker-router` and `ThinkingConfig` from `clankers-provider`, and `EnginePromptSubmission` accepts `AgentMessage`. That makes the engine compile as if it were part of the full Clankers app rather than a small embeddable reducer.

## Goals / Non-Goals

**Goals**

- Make `clanker-message` the canonical crate for generic LLM message and request contract types.
- Make `clankers-engine` free of provider/router/runtime dependencies.
- Keep existing router/provider public names available through compatibility re-exports during migration.
- Move shell-history filtering out of the engine and into Clankers adapters.
- Add deterministic rails that catch future coupling regressions.

**Non-Goals**

- Do not extract the async turn runner in this change.
- Do not move built-in tools or plugin runtime code.
- Do not change provider wire contracts beyond type import ownership.
- Do not change system prompt assembly, session storage, daemon protocol, or TUI behavior.

## Decisions

### 1. `clanker-message` owns generic LLM contracts

**Choice:** Move or define `Usage`, `ToolDefinition`, `ThinkingConfig`, stream metadata/deltas, and message/content types in `clanker-message`.

**Rationale:** These are data contracts shared by engine, providers, router, session, and tools. They are not router implementation details.

**Alternative:** Create a new `clanker-llm-types` crate. Rejected for now because `clanker-message` already owns the adjacent content/message surface and can become the small reusable type crate with fewer workspace changes.

**Implementation:** Reverse imports so `clanker-router` and `clankers-provider` consume/re-export `clanker-message` types instead of the other way around.

### 2. Engine accepts canonical engine transcripts

**Choice:** Change `EnginePromptSubmission.messages` from `Vec<AgentMessage>` to `Vec<EngineMessage>`.

**Rationale:** Filtering shell-only message variants is adapter policy. Embedders should not need Clankers shell message enums to run a turn.

**Alternative:** Keep accepting `AgentMessage` but hide it behind feature flags. Rejected because it leaves type ownership ambiguous and preserves the shell dependency at the engine boundary.

**Implementation:** Add adapter conversion in `clankers-agent::turn` or a nearby adapter module and cover user, assistant, tool-result, and excluded shell-only variants.

### 3. Compatibility through re-exports, not duplicate types

**Choice:** Preserve commonly used `clanker-router` and `clankers-provider` type names as re-exports of canonical message definitions.

**Rationale:** The diff should be type-ownership migration, not a full workspace API rename.

**Alternative:** Rewrite all call sites to import only from `clanker-message` immediately. Rejected because it creates broad churn and hides the boundary change in import noise.

### 4. Dependency rails are part of the API contract

**Choice:** Add cargo-tree and source-inventory checks to the existing FCIS/boundary validation bundle.

**Rationale:** This work can regress through one innocent re-export. Source and dependency checks catch both direct imports and transitive graph drift.

**Implementation:** Add a dedicated script or extend `fcis_shell_boundaries.rs` with explicit forbidden crate lists for `clankers-engine` and `clanker-message`.

## Risks / Trade-offs

**Type identity drift** → Mitigate by moving one canonical definition at a time and adding serde projection parity tests where public JSON shape matters.

**Broad import churn** → Mitigate with compatibility re-exports first, then optional cleanup later.

**Router features still compile heavy defaults** → Mitigate by checking normal dependency trees with the same feature set used by Clankers, and by keeping router runtime features out of `clanker-message` entirely.

**Engine tests accidentally use shell-only helpers** → Mitigate by source rails that ban `AgentMessage` and timestamp/message-ID construction in non-test engine public paths.
