# group-c-types Specification

## Purpose
Governs high-fanout shared type crates consumed across the workspace while preserving direct caller compatibility and source-graph consistency.

## Requirements
### Requirement: tui-types Workspace Library
The `clanker-tui-types` crate MUST remain a workspace-local package under `crates/clanker-tui-types/`. This crate defines the UI event, action, block, completion, cost, display, menu, merge, panel, peer, plugin, process, progress, registry, selector, subagent, and syntax types used by many workspace crates.

#### Scenario: TUI type crate compiles under local namespace
- GIVEN `crates/clanker-tui-types/` with dependencies on chrono, serde, serde_json, rat-branches, rat-leaderkey, and rat-markdown
- WHEN the workspace builds
- THEN all type modules compile and export their public types
- AND reverse dependents import `clanker_tui_types` directly
- AND no temporary `clankers-tui-types` re-export wrapper is required

### Requirement: message Workspace Library
The `clanker-message` crate MUST remain a workspace-local package under `crates/clanker-message/`. This crate depends on the workspace-local `clanker-router` crate and defines conversation message types used by multiple workspace crates.

#### Scenario: Message crate shares router source graph locally
- GIVEN `crates/clanker-message/` with one dependency on `clanker-router`
- WHEN the workspace builds
- THEN the router dependency resolves to `crates/clanker-router/`
- AND no `vendor/clanker-router` patch or external git source is required for `clanker-message`
- AND all message types, including `Message`, `Role`, `Content`, `ToolUse`, `ToolResult`, `Usage`, and related helpers, are public and serialize identically to the pre-localization format
- AND reverse dependents import `clanker_message` directly
- AND no temporary `clankers-message` re-export wrapper is required
