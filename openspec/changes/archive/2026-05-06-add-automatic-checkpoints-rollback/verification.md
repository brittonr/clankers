# Verification

## Drain Verification Matrix

| Rail | Command | Status | Scope rationale |
| --- | --- | --- | --- |
| format | `cargo fmt --check` | pass | Rust source formatting for touched files. |
| targeted unit | `CARGO_TARGET_DIR=target cargo test --lib checkpoints::tests -- --nocapture` | pass: 9 passed | Checkpoint policy/model/receipt/runtime behavior. |
| targeted integration | `CARGO_TARGET_DIR=target cargo test --test checkpoint -- --nocapture` | pass: 2 passed | Explicit checkpoint CLI/library round trips still pass. |
| targeted tools | `CARGO_TARGET_DIR=target cargo test --lib tools::edit -- --nocapture && CARGO_TARGET_DIR=target cargo test --lib tools::patch -- --nocapture` | pass: 4 edit + 5 patch tests | Existing file-mutating tool tests remain compatible with automatic checkpoint skip outside git. |
| compile | `CARGO_TARGET_DIR=target cargo check --tests` | pass | Affected workspace test targets compile. |
| openspec | `openspec validate add-automatic-checkpoints-rollback --strict` | pass | Active change remains parser-valid before archive. |
| whitespace | `git diff --check` | pass | No whitespace errors in source/docs/spec artifacts. |

## Inventory

Current checkpoint/rollback seams inspected for this slice:

- `src/checkpoints.rs` owns git-backed checkpoint records, metadata, create/list/rollback behavior, and unit tests. This is the right home for automatic checkpoint policy, request, and receipt models.
- `src/tools/checkpoint.rs` exposes explicit agent-facing create/list/rollback operations and already attaches checkpoint metadata via `ToolResult::with_details`.
- `src/commands/checkpoint.rs` exposes the CLI checkpoint commands.
- `src/tools/write.rs`, `src/tools/edit.rs`, and `src/tools/patch.rs` are the first protected file-mutating built-in tools. Each writes directly after computing a diff preview, so the automatic checkpoint guard must run after parameter/path validation but before the write.
- `README.md` documents the user-facing checkpoint safety boundary.
- `tests/checkpoint.rs` covers explicit checkpoint CLI/library round trips; focused new model/adapter tests live next to `src/checkpoints.rs`.

Implementation files touched in this slice:

- `src/checkpoints.rs`
- `src/tools/mod.rs`
- `src/tools/write.rs`
- `src/tools/edit.rs`
- `src/tools/patch.rs`
- `README.md`
- `openspec/changes/add-automatic-checkpoints-rollback/tasks.md`
- `openspec/changes/add-automatic-checkpoints-rollback/verification.md`
