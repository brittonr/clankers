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

**Implementation:** Reverse imports so `clanker-router` and `clankers-provider` consume/re-export `clanker-message` types instead of the other way around. The canonical public contracts are `Usage`, `ToolDefinition`, `ThinkingConfig`, `MessageMetadata`, `ContentDelta`, `StreamDelta`, and adjacent plain streaming/message data. Compatibility exports must be type aliases or `pub use` re-exports of those canonical definitions, not newtype wrappers.

### 2. Engine accepts canonical engine transcripts

**Choice:** Change `EnginePromptSubmission.messages` from `Vec<AgentMessage>` to `Vec<EngineMessage>`.

**Rationale:** Filtering shell-only message variants is adapter policy. Embedders should not need Clankers shell message enums to run a turn.

**Alternative:** Keep accepting `AgentMessage` but hide it behind feature flags. Rejected because it leaves type ownership ambiguous and preserves the shell dependency at the engine boundary.

**Implementation:** Add pure transcript conversion helpers in `crates/clankers-agent/src/turn/execution.rs` or a private sibling module called only from `execution.rs`. Provider request shaping remains in `execution.rs`; `clankers-engine` receives only `EngineMessage` / `EngineModelRequest` data. Tests must cover included user, assistant, and tool-result messages plus excluded `BashExecution`, `Custom`, `BranchSummary`, and `CompactionSummary` history entries.

### 3. Compatibility through re-exports, not duplicate types

**Choice:** Preserve current workspace public contract paths as re-exports of canonical message definitions: `clanker_router::provider::{ToolDefinition, ThinkingConfig, Usage}`, `clanker_router::{ThinkingConfig, Usage}`, `clanker_router::streaming::{MessageMetadata, ContentDelta}`, `clankers_provider::{ThinkingConfig, Usage}`, `clankers_provider::streaming::{MessageMetadata, ContentDelta}`, and any provider/router request fields that expose these types. `StreamDelta` remains a `clanker_message::StreamDelta` public alias of canonical `clanker_message::ContentDelta`; no router/provider `StreamDelta` compatibility path exists today or is required, and router/provider crates must not introduce an independent `StreamDelta` type.

**Rationale:** The diff should be type-ownership migration, not a full workspace API rename.

**Alternative:** Rewrite all call sites to import only from `clanker-message` immediately. Rejected because it creates broad churn and hides the boundary change in import noise.

### 4. Dependency rails are part of the API contract

**Choice:** Add cargo-tree and source-inventory checks to the existing FCIS/boundary validation bundle.

**Rationale:** This work can regress through one innocent re-export. Source and dependency checks catch both direct imports and transitive graph drift.

**Implementation:** Add deterministic checks to the boundary validation bundle. The cargo-tree rail runs normal, non-dev dependency inventories with `cargo tree -p clankers-engine --edges normal` and `cargo tree -p clanker-message --edges normal`. It fails with the matched crate names if `clankers-engine` includes any of `clankers-provider`, `clanker-router`, `tokio`, `reqwest`, `redb`, `iroh`, `ratatui`, `crossterm`, `portable-pty`, or `clankers-agent`, or if `clanker-message` includes any of `clanker-router`, `clankers-provider`, `tokio`, `reqwest`, `reqwest-eventsource`, `redb`, `fs4`, `iroh`, `axum`, `tower-http`, `ratatui`, `crossterm`, or `portable-pty`.

Wire the cargo-tree rail through `scripts/check-llm-contract-boundary.sh`, call that script from `scripts/verify-no-std-functional-core.sh`, and keep the source-inventory rail in `crates/clankers-controller/tests/fcis_shell_boundaries.rs`. The source-inventory rail scans non-test Rust items under `crates/clankers-engine/src/**/*.rs` and contract files under `crates/clanker-message/src/**/*.rs`. It rejects provider-shaped `CompletionRequest`, daemon protocol types (`DaemonEvent`, `SessionCommand`, `ControlResponse`, `AttachResponse`), TUI/runtime types (`clanker_tui_types`, `ratatui`, `crossterm`, `portable_pty`), Tokio handles (`tokio::`, `JoinHandle`, `mpsc`, `oneshot`), timestamp/request-shaping imports (`chrono::Utc`, `chrono::DateTime`, `uuid::Uuid`, `generate_id`), shell request construction, and any non-test `AgentMessage` dependency/import/use inside `clankers-engine`. The allowlist is the `clankers-agent::turn` adapter seam, where shell messages may be converted before engine submission.

## Verification Plan

- Run `openspec validate decouple-llm-contract-surface --strict` plus proposal/design/tasks gates before implementation tasks are marked done.
- Before moving imports, add focused compatibility tests with inline golden `serde_json::json!` fixtures populated from the current pre-migration shapes. These tests assert router/provider compatibility names have the same type identity as canonical `clanker-message` definitions and representative serde JSON for `Usage`, `ToolDefinition`, `ThinkingConfig`, `MessageMetadata`, `ContentDelta`, and provider requests is unchanged. `StreamDelta` is verified as `clanker_message::StreamDelta` aliasing the canonical `ContentDelta`; router/provider crates must not define their own `StreamDelta` type.
- Run positive and negative adapter tests at the `crates/clankers-agent/src/turn/execution.rs` seam for included user/assistant/tool-result messages and excluded `BashExecution`, `Custom`, `BranchSummary`, and `CompactionSummary` entries.
- Run the cargo-tree and source-inventory rails described in Decision 4, then focused engine/message/provider/router/agent adapter tests.

## Risks / Trade-offs

**Type identity drift** → Mitigate by moving one canonical definition at a time and adding required serde projection parity tests for `Usage`, `ToolDefinition`, `ThinkingConfig`, `MessageMetadata`, `ContentDelta`, and representative `StreamEvent`/provider request JSON that exercise the moved types.

**Broad import churn** → Mitigate with compatibility re-exports first, then optional cleanup later.

**Router features still compile heavy defaults** → Mitigate by checking normal dependency trees with the same feature set used by Clankers, and by keeping router runtime features out of `clanker-message` entirely.

**Engine tests accidentally use shell-only helpers** → Mitigate by source rails that ban `AgentMessage` and timestamp/message-ID construction in non-test engine public paths.
