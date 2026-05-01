## Phase 1: Discovery and API Shape

- [x] Inventory existing clankers modules that should own Working Directory Checkpoints and Rollback. ✅ completed: 2026-05-01T22:31:40Z
  - Evidence: `openspec/changes/add-worktree-checkpoints-rollback/evidence/module-inventory.md` maps existing owners (`src/worktree/`, `src/tools/git_ops/`, mutating tools, CLI/commands, session persistence, and config), identifies gaps, and recommends the first-pass git-only checkpoint boundary.
- [x] Define the user-facing CLI/TUI/tool/config surface and document unsupported first-pass cases. ✅ completed: 2026-05-01T22:31:40Z
  - Evidence: `openspec/changes/add-worktree-checkpoints-rollback/evidence/api-surface.md` defines the first-pass CLI, future-safe agent tool/TUI wrappers, no-new-config stance, unsupported cases, and metadata policy boundaries.
- [x] Add focused tests for parsing, configuration, and policy boundaries. ✅ completed: 2026-05-01T22:42:48Z
  - Evidence: added `src/checkpoints.rs` policy/metadata helpers and tests for namespace validation, replay-safe metadata, and sanitized errors; added `checkpoint` CLI parser/action tests for create and rollback confirmation boundaries. Verification passed `cargo fmt` and `CARGO_TARGET_DIR=target cargo nextest run -p clankers checkpoint --no-fail-fast` (9 passed).

## Phase 2: Implementation

- [x] Implement the minimal backend or adapter for Working Directory Checkpoints and Rollback. ✅ completed: 2026-05-01T22:48:41Z
  - Evidence: `src/checkpoints.rs` implements local git-backed create/list/rollback using `.git/clankers-checkpoints`; `src/commands/checkpoint.rs` wires CLI actions to the backend. Verification passed `cargo fmt` and `CARGO_TARGET_DIR=target cargo nextest run -p clankers checkpoint --no-fail-fast` (11 passed).
- [x] Wire the capability through standalone prompt, interactive TUI, and daemon/session paths where applicable. ✅ completed: 2026-05-01T22:52:07Z
  - Evidence: added the `checkpoint` specialty tool and registered it in `build_tiered_tools`, making it available through shared prompt/TUI/daemon tool construction; CLI was already wired through `clankers checkpoint`. Verification passed `cargo fmt` and `CARGO_TARGET_DIR=target cargo nextest run -p clankers checkpoint --no-fail-fast` (12 passed).
- [x] Persist or log session metadata needed for replay and debugging. ✅ completed: 2026-05-01T22:54:00Z
  - Evidence: `src/tools/checkpoint.rs` attaches normalized `CheckpointMetadata` via `ToolResult::with_details`; `openspec/changes/add-worktree-checkpoints-rollback/evidence/session-metadata.md` documents replay-safe fields and redaction boundaries. Verification passed `CARGO_TARGET_DIR=target cargo nextest run -p clankers checkpoint --no-fail-fast` (12 passed).

## Phase 3: Verification and Documentation

- [ ] Add integration tests for the primary successful path and at least one failure path.
- [ ] Update README/docs and any relevant built-in tool or command lists.
- [ ] Run `cargo fmt`, targeted `cargo nextest`, `cargo check --tests`, and `git diff --check`.
