# group-c-types Specification

## Purpose
TBD - created by archiving change crate-extraction-2. Update Purpose after archive.
## Requirements
### Requirement: tui-types Extraction
The `clankers-tui-types` crate MUST be extracted to `clanker-tui-types`. This crate defines the UI event, action, block, completion, cost, display, menu, merge, panel, peer, plugin, process, progress, registry, selector, subagent, and syntax types used by many workspace crates.

#### Scenario: TUI type crate compiles under standalone namespace
- GIVEN `crates/clankers-tui-types/` with zero internal dependencies and workspace dependencies on chrono, serde, serde_json, rat-branches, and rat-leaderkey
- WHEN it is extracted to the `clanker-tui-types` repo
- THEN all type modules compile and export their public types
- AND the rat-branches and rat-leaderkey path dependencies are converted to git dependencies pointing at the subwayrat repo
- AND all `clankers_tui_types` references are renamed to `clanker_tui_types`
- AND reverse dependents compile via a temporary re-export wrapper during migration

### Requirement: tui-types Reverse Dep Migration
After extraction, each reverse dependent MUST be migrated from `use clankers_tui_types::` to `use clanker_tui_types::` directly. The thin wrapper MUST be removed once all callers are migrated.

#### Scenario: TUI type wrapper is removed after direct imports land
- GIVEN the re-export wrapper at `crates/clankers-tui-types/src/lib.rs`
- WHEN all direct dependents import `clanker_tui_types`
- THEN the `crates/clankers-tui-types/` directory can be deleted
- AND the workspace `members` list in root `Cargo.toml` is updated
- AND `cargo check && cargo nextest run` passes

### Requirement: message Extraction
The `clankers-message` crate MUST be extracted to `clanker-message`. This crate depends on `clanker-router`, which was already extracted in phase 1, and defines conversation message types used by multiple workspace crates.

#### Scenario: Message crate shares router source graph after extraction
- GIVEN `crates/clankers-message/` with one internal dependency on the extracted router
- WHEN it is extracted to the `clanker-message` repo
- THEN the router dependency is declared as a git dependency in the new repo
- AND the main workspace patches that git source back to `vendor/clanker-router` while the vendored snapshot remains authoritative locally
- AND all message types, including `Message`, `Role`, `Content`, `ToolUse`, `ToolResult`, `Usage`, and related helpers, are public and serialize identically to the pre-extraction format
- AND all `clankers_message` references are renamed to `clanker_message`
- AND reverse dependents compile via a temporary re-export wrapper during migration

### Requirement: message Reverse Dep Migration
After extraction, each reverse dependent MUST be migrated from `use clankers_message::` to `use clanker_message::` directly.

#### Scenario: Message wrapper is removed after direct imports land
- GIVEN the re-export wrapper at `crates/clankers-message/src/lib.rs`
- WHEN all direct dependents import `clanker_message`
- THEN the `crates/clankers-message/` directory can be deleted
- AND `cargo check && cargo nextest run` passes
