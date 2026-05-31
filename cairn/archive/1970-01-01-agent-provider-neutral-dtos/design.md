# Design: Drain Agent Provider Coupling to Neutral DTOs

## Summary

This change separates message/stream DTO ownership from provider execution ownership. Agent turn policy should use `clanker-message`, `clankers-engine`, and `clankers-runtime` DTOs. Provider-native request construction stays in a model adapter that can be replaced by runtime provider services later.

## Decisions

### 1. Message and usage imports come from `clanker-message`

`AgentMessage`, `Content`, `ToolResultMessage`, `StopReason`, `Usage`, and stream deltas should be imported from `clanker-message` when they are neutral conversation or stream DTOs. Provider reexports should not be used in reusable agent policy.

### 2. Provider-native request construction is adapter-owned

`CompletionRequest`, `Provider`, and provider streaming execution may remain temporarily, but only inside a named adapter owner. Turn policy, transcript, compaction summaries, and agent events should not depend on provider-native module paths for neutral data.

### 3. Bridge toward runtime provider services

Where a model port already receives a provider-native `CompletionRequest`, introduce a neutral intermediate or receipt that can later be implemented by `clankers-runtime::ProviderRouterService`. The change should make the next step visible even if it does not remove the provider dependency entirely.

### 4. Rails prefer import/module ownership over brittle text counts

The source rail should parse Rust imports and paths in non-test items. It should allow provider imports in adapter modules and reject them in reusable policy modules with diagnostics that name the replacement DTO owner.

## Validation plan

- Focused compile and tests for agent turn execution, transcript, compaction, and tool-substrate paths.
- Source-boundary rail proving neutral DTO imports in reusable agent modules.
- Dependency ownership baseline update showing a smaller or more precise provider convergence condition.
- No provider/router request-shape changes unless covered by existing provider fixtures.
