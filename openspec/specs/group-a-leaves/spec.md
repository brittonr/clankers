# group-a-leaves Specification

## Purpose
Governs leaf and SDK crates that remain small independent packages inside the clankers workspace.

## Requirements
### Requirement: plugin-sdk Workspace Library
The `clanker-plugin-sdk` crate MUST remain a workspace-local SDK library under `crates/clanker-plugin-sdk/`. Plugin crates MUST consume it through relative path dependencies, not a GitHub dependency or standalone flake input.

#### Scenario: Shared plugin SDK builds from the workspace
- GIVEN `crates/clanker-plugin-sdk/` with `crate-type = ["rlib"]`
- WHEN workspace and plugin builds resolve dependencies
- THEN the SDK compiles as a normal workspace package
- AND standalone plugin crates depend on it through `../../crates/clanker-plugin-sdk` or the correct relative path from their manifest
- AND the `extism-pdk` re-export still works
- AND the prelude module re-exports all protocol types
