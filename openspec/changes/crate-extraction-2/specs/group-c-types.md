# Group C: High-Impact Type Crate Extractions — Spec

## Purpose

Defines the extraction contracts for the two shared type crates with the
highest reverse dependency counts in the reduced-scope `crate-extraction-2`
slice.

## Requirements

### tui-types Extraction

The `clankers-tui-types` crate MUST be extracted to `clanker-tui-types`.
This crate defines the UI event, action, block, completion, cost, display,
menu, merge, panel, peer, plugin, process, progress, registry, selector,
subagent, and syntax types used by many workspace crates.

GIVEN `crates/clankers-tui-types/` with zero internal deps
  AND workspace deps on chrono, serde, serde_json, rat-branches, rat-leaderkey
WHEN extracted to the `clanker-tui-types` repo
THEN all 18 type modules compile and export their public types
AND the rat-branches and rat-leaderkey path deps are converted to git deps
    pointing at the subwayrat repo
AND all `clankers_tui_types` references are renamed to `clanker_tui_types`
AND reverse dependents compile via a temporary re-export wrapper during migration

### tui-types Reverse Dep Migration

After extraction, each reverse dependent SHOULD be migrated from
`use clankers_tui_types::` to `use clanker_tui_types::` directly. The thin
wrapper MAY be removed once all callers are migrated.

GIVEN the re-export wrapper at `crates/clankers-tui-types/src/lib.rs`
WHEN all direct dependents import `clanker_tui_types`
THEN the `crates/clankers-tui-types/` directory can be deleted
AND the workspace `members` list in root `Cargo.toml` is updated
AND `cargo check && cargo nextest run` passes

### message Extraction

The `clankers-message` crate MUST be extracted to `clanker-message`. This
crate depends on `clanker-router` (already extracted in phase 1) and defines
conversation message types used by multiple workspace crates.

GIVEN `crates/clankers-message/` with one internal dep on the extracted router
WHEN extracted to the `clanker-message` repo
THEN the router dependency is declared as a git dep in the new repo
AND the main workspace patches that git source back to `vendor/clanker-router`
    while the vendored snapshot remains authoritative locally
AND all message types (`Message`, `Role`, `Content`, `ToolUse`, `ToolResult`,
    `Usage`, and related helpers) are public and serialize identically to the
    pre-extraction format
AND all `clankers_message` references are renamed to `clanker_message`
AND reverse dependents compile via a temporary re-export wrapper during migration

### message Reverse Dep Migration

After extraction, each reverse dependent SHOULD be migrated from
`use clankers_message::` to `use clanker_message::` directly.

GIVEN the re-export wrapper at `crates/clankers-message/src/lib.rs`
WHEN all direct dependents import `clanker_message`
THEN the `crates/clankers-message/` directory can be deleted
AND `cargo check && cargo nextest run` passes
