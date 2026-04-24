# Crate Extraction Protocol — Spec

## Purpose

Defines the behavioral contract for extracting a crate from the clankers
workspace into a standalone repository. Every extraction in
`crate-extraction-3` MUST follow this protocol to maintain workspace stability,
preserve git history, unify shared dependency sources, and refresh generated
workspace artifacts after wrapper cleanup.

## Requirements

### History Preservation

The extracted crate MUST retain its full git history from the clankers
repository. Use `git subtree split` or `git filter-repo` to produce a branch
containing only the crate's directory, then push that branch as the initial
commit of the new repo.

GIVEN a crate at `crates/clankers-foo/`
WHEN `git subtree split -P crates/clankers-foo -b extract-foo` runs
THEN the resulting branch contains all commits that touched files under
     `crates/clankers-foo/`
AND the new repo's `git log` shows the original commit messages and dates

### Namespace Rename

The extracted crate MUST be renamed to drop the `clankers-` prefix. The new
name SHOULD be descriptive and short. The crate name, module paths, doc
comments, and binary names (if any) MUST all be updated.

GIVEN a crate named `clankers-foo`
WHEN it is extracted
THEN its `Cargo.toml` has `name = "clanker-foo"` (or a custom name)
AND no source file contains `clankers_foo` or `clankers-foo` in imports,
    doc comments, or string literals (except historical changelog entries)

### Workspace Continuity

The clankers workspace MUST compile and pass all tests after each individual
extraction. No big-bang migration — one crate at a time.

GIVEN a crate has been extracted to a standalone repo
WHEN the workspace `Cargo.toml` replaces `path = "crates/clankers-foo"`
     with a git dependency on the new repo
THEN `cargo check` succeeds on the full workspace
AND `cargo nextest run` passes with zero regressions

### Shared Dependency Source Unification

When an extracted crate depends on another crate that the workspace already
vendors or patches to a local snapshot, the workspace MUST force a single
source graph.

GIVEN an extracted crate's published `Cargo.toml` points at a git source that
     the workspace already vendors or patches differently
WHEN the workspace consumes that extracted crate
THEN the root workspace adds a matching `[patch."<source-url>"]` entry so the
     shared dependency resolves to one source graph locally
AND validation evidence is taken only after the `[patch]` entry is in place

### Verification Preconditions

Validation evidence MUST be taken from a clean enough dependency graph that
local sibling dirt does not masquerade as an extraction regression.

GIVEN the workspace validation rails use sibling repos such as `../subwayrat`
     or `../ratcore`
WHEN a focused or full verification run is used as evidence for this change
THEN those sibling path dependencies are confirmed clean first
OR the resulting failure is explicitly recorded as contaminated external noise
    rather than attributed to the extraction diff

### Re-export Wrapper

During migration, the old crate directory MAY contain a thin wrapper that
re-exports all items from the extracted crate. This preserves existing import
paths and avoids a mass find-replace.

GIVEN a thin wrapper at `crates/clankers-foo/src/lib.rs`
WHEN it contains `pub use clanker_foo::*;`
THEN all existing `use clankers_foo::` imports continue to resolve
AND the wrapper can be removed once all callers are migrated

### Standalone CI

The extracted crate MUST have its own CI configuration that runs:
- `cargo check`
- `cargo clippy -- -D warnings`
- `cargo fmt -- --check`
- `cargo nextest run` (or `cargo test` if nextest is not configured)

GIVEN an extracted repo on GitHub
WHEN a commit is pushed
THEN CI runs all four checks
AND the repo's README shows a CI badge

### Licensing

The extracted crate MUST carry a LICENSE file. The license SHOULD match the
clankers workspace license (AGPL-3.0-or-later) unless there is a reason to use
a more permissive license for the extracted crate.

### README

The extracted crate MUST have a README.md containing:
- a one-line description
- a usage example
- a link back to the clankers project (for context)

### Generated Artifact Refresh

After wrapper removal, the workspace MUST refresh generated artifacts that can
otherwise drift silently.

GIVEN one or more extraction wrappers have been removed
WHEN the final cleanup phase runs
THEN `build-plan.json` is regenerated with `unit2nix --workspace --force --no-check -o build-plan.json`
AND generated docs are refreshed with `cargo xtask docs`
AND any user-visible TUI snapshots are refreshed only if an extraction rename
    changed rendered text or layout
AND the refreshed artifacts are part of the final verification evidence
