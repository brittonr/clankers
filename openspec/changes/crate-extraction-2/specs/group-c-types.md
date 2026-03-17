# Group C: High-Impact Type Crate Extractions — Spec

## Purpose

Contracts for the two type crates with the highest reverse dependency counts.
Extracting these converts the most path deps into git deps in a single move.
They require careful sequencing because so many crates import them.

## Requirements

### tui-types Extraction

The `clankers-tui-types` crate MUST be extracted to `clanker-tui-types`.
This crate defines the UI event, action, block, completion, cost, display,
menu, merge, panel, peer, plugin, process, progress, registry, selector,
subagent, and syntax types used by 10 other workspace crates.

GIVEN `crates/clankers-tui-types/` with zero internal deps
  AND workspace deps on chrono, serde, serde_json, rat-branches, rat-leaderkey
WHEN extracted to `clanker-tui-types` repo
THEN all 18 type modules compile and export their public types
AND the rat-branches and rat-leaderkey path deps are converted to git deps
    pointing at the subwayrat repo
AND all `clankers_tui_types` references are renamed to `clanker_tui_types`
AND 10 reverse dependents compile via re-export wrapper:
    root, agent, config, controller, model-selection, plugin, procmon,
    provider, tui, util

### tui-types Reverse Dep Migration

After extraction, each of the 10 reverse dependents SHOULD be migrated
from `use clankers_tui_types::` to `use clanker_tui_types::` directly.
The thin wrapper MAY be removed once all callers are migrated.

GIVEN the re-export wrapper at `crates/clankers-tui-types/src/lib.rs`
WHEN all 10 dependents have been updated to import `clanker_tui_types`
THEN the `crates/clankers-tui-types/` directory can be deleted
AND the workspace `members` list in root `Cargo.toml` is updated
AND `cargo check && cargo nextest run` passes

### message Extraction

The `clankers-message` crate MUST be extracted to `clanker-message`. This
crate depends on `clanker-router` (already extracted in phase 1). It
defines conversation message types used by 6 other workspace crates.

GIVEN `crates/clankers-message/` with 1 internal dep: clanker-router (workspace git dep)
WHEN extracted to `clanker-message` repo
THEN the clanker-router dependency is declared as a git dep in the new repo
AND all message types (Message, Role, Content, ToolUse, ToolResult, Usage, etc.)
    are public and serialize identically to pre-extraction
AND all `clankers_message` references are renamed to `clanker_message`
AND 6 reverse dependents compile via re-export wrapper:
    root, agent, controller, provider, session, util

### message Reverse Dep Migration

After extraction, each of the 6 reverse dependents SHOULD be migrated
from `use clankers_message::` to `use clanker_message::` directly.

GIVEN the re-export wrapper at `crates/clankers-message/src/lib.rs`
WHEN all 6 dependents have been updated to import `clanker_message`
THEN the `crates/clankers-message/` directory can be deleted
AND `cargo check && cargo nextest run` passes

### Extraction Ordering

Group C extractions MUST happen after Group A and B because:
- `clankers-message` depends on `clanker-router` (phase 1, done)
- `clankers-tui-types` depends on `rat-branches` and `rat-leaderkey`
  (currently path deps to `../subwayrat/`, must be resolved first)

The tui-types extraction MUST resolve the subwayrat path deps before or
during extraction. Options:
1. Publish rat-branches and rat-leaderkey to crates.io
2. Move them to standalone git repos
3. Keep them as git deps pointing to the subwayrat repo
