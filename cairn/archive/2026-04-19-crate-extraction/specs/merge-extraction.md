# Merge Crate Extraction (graggle)

## Purpose

Extract `clankers-merge` into a standalone crate implementing
order-independent text merge via graggle theory (Mimram & Di Giusto).

The crate has one dependency (serde), zero workspace dependencies,
and zero clankers-specific types in its source. This is the cleanest
extraction in the set.

## Requirements

### Crate identity

r[merge.identity.name]
The extracted crate MUST be named `graggle`.

r[merge.identity.repo]
The crate MUST live in its own GitHub repository, not as a workspace member.

### Source migration

r[merge.source.files]
The following files MUST be moved to the new repo:

- `src/lib.rs` — re-exports
- `src/diff.rs` — Myers diff implementation
- `src/flatten.rs` — DAG linearization with conflict detection
- `src/graggle.rs` — Graggle (graph-file) data structure
- `src/merge.rs` — categorical pushout merge
- `src/patch.rs` — patch representation and application

r[merge.source.no-clankers-refs]
The source MUST NOT contain the string "clankers" anywhere — not in
module docs, code comments, type names, or test names.

r[merge.source.doc-example]
The crate root documentation MUST include a working doc-test showing
a 3-way merge with conflict-free and conflicting cases.

### API surface

r[merge.api.graggle]
The crate MUST export `Graggle` with at minimum:
- `Graggle::from_text(&str) -> Graggle`
- `Graggle::to_text() -> String`

r[merge.api.merge]
The crate MUST export `merge(base: &Graggle, branches: &[&str]) -> MergeResult`
(or equivalent signature).

r[merge.api.types]
The crate MUST export `Vertex`, `VertexId`, `Patch`, `PatchOp`, `PatchId`,
`FlattenResult`, `FlattenBlock`, `ROOT`, `END`.

r[merge.api.serde]
All public types MUST implement `Serialize` and `Deserialize`.

### Tests

r[merge.tests.existing]
All existing tests from `clankers-merge` MUST pass in the extracted crate
without modification (beyond import path changes).

r[merge.tests.doc]
The crate MUST have passing doc-tests on the root module and on `merge()`.

### Workspace migration

r[merge.migration.re-export]
After extraction, `crates/clankers-merge/` MUST become a thin wrapper:

```toml
[dependencies]
graggle = { git = "https://github.com/brittonr/graggle" }
```

```rust
pub use graggle::*;
```

r[merge.migration.callers-unchanged]
All existing callers (`src/worktree/merge_strategy.rs`, `verus/merge_spec.rs`)
MUST compile without changes after the re-export wrapper is in place.

r[merge.migration.workspace-builds]
`cargo check` and `cargo nextest run` MUST pass on the full workspace
after the migration.
