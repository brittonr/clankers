# extraction-protocol Specification

## Purpose
Defines invariants every workspace crate extraction must satisfy: history preservation, namespace rename, workspace continuity, re-export wrappers, standalone CI, licensing, and README.

## Requirements
### Requirement: History Preservation
The extracted crate MUST either retain its git history from the clankers repository or record a design-level clean-move exception. Use `git subtree split` or `git filter-repo` when preserving history. Use a clean-move exception only when the design explains why the crate already has an independent identity or why the new repository intentionally starts from a curated snapshot.

#### Scenario: Subtree split preserves crate history
- GIVEN a crate at `crates/clankers-foo/`
- WHEN `git subtree split -P crates/clankers-foo -b extract-foo` runs
- THEN the resulting branch contains all commits that touched files under `crates/clankers-foo/`
- AND the new repo's `git log` shows the original commit messages and dates

#### Scenario: Clean move exception is documented
- GIVEN a crate extraction does not preserve full parent-repo history
- WHEN the extraction is marked complete
- THEN `design.md` records the exception and rationale
- AND `tasks.md` includes a checked task that records the exception or snapshot provenance

### Requirement: Namespace Rename
The extracted crate MUST be renamed to drop the `clankers-` prefix. The new name SHOULD be descriptive and short. The crate name, module paths, doc comments, and binary names, if any, MUST all be updated.

#### Scenario: Extracted crate uses the standalone namespace
- GIVEN a crate named `clankers-foo`
- WHEN it is extracted
- THEN its `Cargo.toml` has `name = "clanker-foo"` or a custom standalone name
- AND no source file contains `clankers_foo` or `clankers-foo` in imports, doc comments, or string literals except historical changelog entries

### Requirement: Workspace Continuity
The clankers workspace MUST compile and pass all tests after each individual extraction. No big-bang migration is allowed.

#### Scenario: Workspace validates after one crate migration
- GIVEN a crate has been extracted to a standalone repo
- WHEN the workspace `Cargo.toml` replaces `path = "crates/clankers-foo"` with a reproducible git dependency or checked-in vendored source dependency
- THEN `cargo check` succeeds on the full workspace
- AND `cargo nextest run` passes with zero regressions

#### Scenario: Historical split uses final continuity bundle
- GIVEN a reduced-scope change was split after implementation was already complete
- WHEN per-extraction validation logs are incomplete or include a documented pre-existing flake
- THEN final closeout evidence MUST record the waiver and rationale
- AND a current full-workspace validation bundle MUST pass with zero known extraction regressions

### Requirement: Re-export Wrapper
During migration, the old crate directory MUST be allowed to contain a thin wrapper that re-exports all items from the extracted crate. This preserves existing import paths and avoids a mass find-replace while callers move to the standalone crate.

#### Scenario: Temporary wrapper preserves old imports
- GIVEN a thin wrapper at `crates/clankers-foo/src/lib.rs`
- WHEN it contains `pub use clanker_foo::*;`
- THEN all existing `use clankers_foo::` imports continue to resolve
- AND the wrapper can be removed once all callers are migrated

### Requirement: Standalone CI
The extracted crate MUST have its own CI configuration that runs `cargo check`, `cargo clippy -- -D warnings`, `cargo fmt -- --check`, and `cargo nextest run`, or `cargo test` if nextest is not configured.

#### Scenario: Extracted source declares validation
- GIVEN an extracted repo on GitHub or a checked-in vendored source tree prepared for publication
- WHEN its CI workflow is inspected
- THEN CI runs all required checks
- AND the repo's README shows a CI badge

### Requirement: Licensing
The extracted crate MUST carry a LICENSE file. The license SHOULD match the clankers workspace license, AGPL-3.0-or-later, unless there is a reason to use a more permissive license for the extracted crate.

#### Scenario: Extracted crate carries an explicit license
- GIVEN an extracted standalone crate repo
- WHEN its repository files are inspected
- THEN a LICENSE file is present
- AND the selected license is compatible with the clankers workspace licensing decision

### Requirement: README
The extracted crate MUST have a README.md containing a one-line description, a usage example, and a link back to the clankers project for context.

#### Scenario: Extracted crate has standalone usage documentation
- GIVEN an extracted standalone crate repo
- WHEN its README.md is inspected
- THEN the README contains a one-line description
- AND the README contains a usage example
- AND the README links back to the clankers project for context
