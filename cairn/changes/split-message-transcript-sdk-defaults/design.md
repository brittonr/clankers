# Design: Split Message Transcript SDK Defaults

## Context

The SDK guide already labels `clanker-message::transcript`, `message::*`, `AgentMessage`, `MessageId`, and `generate_id` as unsupported/internal. The crate root nevertheless re-exports those items, and the crate manifest makes timestamp/random-ID dependencies unconditional.

## Decisions

### 1. Stable message defaults are transcript-free

The default SDK-facing imports should expose `Content`, `StopReason`, `ToolDefinition`, `ThinkingConfig`, `Usage`, streaming DTOs, tool-result DTOs, and `SemanticEvent` contracts without transcript IDs or clocks.

### 2. Transcript compatibility stays explicit

Desktop/session/provider/controller adapters that need persisted Clankers transcript records should import an explicit compatibility module or enable a named compatibility feature. The split must keep serialization compatibility fixtures for existing transcript records.

### 3. Dependency rails prove the split

The embedded SDK dependency rail should inspect minimal example metadata and fail if `chrono`, `rand`, `hex`, or transcript compatibility modules appear in the default minimal SDK graph without an explicit compatibility opt-in.

## Risks / Trade-offs

- Root re-export removal can be noisy; provide a clear migration note for app-edge callers.
- Feature-gating a module can break tests that rely on root imports; migrate those tests to explicit compatibility imports.
- Dependency graph checks must distinguish default SDK paths from desktop compatibility paths.
