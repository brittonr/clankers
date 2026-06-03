# Change: Drain Legacy ToolContext Into Neutral Tool Services

## Problem

Legacy `clankers-agent::ToolContext` still carries desktop services directly: `AgentEvent` broadcast, cancellation token, hook pipeline, session id, database handle, and search index. That keeps built-in tools coupled to agent/runtime internals even though `clankers-tool-host` already has neutral service contracts.

## Goals

- Make production tools prefer neutral `ToolInvocationContext` and `ToolHostServices`.
- Keep legacy `ToolContext` as a shrinking compatibility adapter only.
- Move representative built-in tools off direct DB/hook/event access.
- Rail against new tools depending on `ToolContext` for services that have neutral equivalents.

## Non-goals

- Do not rewrite every built-in tool in one slice.
- Do not remove legacy tool support before all production callers migrate.
- Do not change tool result JSON except for safe receipt metadata.

## Proposed scope

Add a migration matrix for built-in tools, pick high-value storage/search and progress/hook users, and convert them to neutral services. The legacy adapter should remain the only place allowed to construct old `ToolContext` values from concrete desktop services.

## Verification

Validation should include neutral service fixtures, migrated-tool parity tests, missing-service fail-closed tests, and source rails rejecting new direct DB/hook/TUI/protocol imports in reusable tool-host code.
