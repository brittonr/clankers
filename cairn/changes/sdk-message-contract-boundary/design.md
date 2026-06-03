# Design: Split Stable Message Contracts From Clankers Transcript Internals

## Summary

The SDK needs shared content and event contracts, not every Clankers desktop transcript variant. This change draws a boundary inside `clanker-message` so stable SDK DTOs are clear and internal transcript records do not become accidental compatibility promises.

## Current coupling points

- `Content`, `ToolDefinition`, `ThinkingConfig`, `Usage`, streaming deltas, and `SemanticEvent` are reusable.
- `AgentMessage`, `MessageId`, `BashExecutionMessage`, `BranchSummaryMessage`, `CompactionSummaryMessage`, and `CustomMessage` encode Clankers session/display history.
- `MessageId::generate` and timestamps pull ID/time concerns into a message crate that SDK users may not want.
- Provider, controller, root restore, and session code still use `AgentMessage` as canonical transcript state.

## Decisions

### 1. Stable contracts stay green

Content blocks, tool definitions/results, usage, streaming deltas, stop reasons, thinking config, and semantic events remain the shared SDK message contracts.

### 2. Transcript internals are compatibility-only

Clankers-specific transcript variants should move to a transcript/session module or stay explicitly unsupported-internal with migration notes.

### 3. Embedders own IDs and timestamps

Generic SDK contracts should not force random message IDs or wall-clock timestamps. Host/session adapters may add those at persistence edges.

## Validation plan

- Generated API inventory support-label update.
- Serialization fixtures for existing `AgentMessage` compatibility.
- Green SDK examples using stable contracts without transcript internals.
- Source rails rejecting `AgentMessage` in green API declarations except allowed adapters.
