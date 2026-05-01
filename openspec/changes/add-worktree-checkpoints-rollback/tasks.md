## Phase 1: Discovery and API Shape

- [x] Inventory existing clankers modules that should own Working Directory Checkpoints and Rollback. ✅ completed: 2026-05-01T22:31:40Z
  - Evidence: `openspec/changes/add-worktree-checkpoints-rollback/evidence/module-inventory.md` maps existing owners (`src/worktree/`, `src/tools/git_ops/`, mutating tools, CLI/commands, session persistence, and config), identifies gaps, and recommends the first-pass git-only checkpoint boundary.
- [ ] Define the user-facing CLI/TUI/tool/config surface and document unsupported first-pass cases.
- [ ] Add focused tests for parsing, configuration, and policy boundaries.

## Phase 2: Implementation

- [ ] Implement the minimal backend or adapter for Working Directory Checkpoints and Rollback.
- [ ] Wire the capability through standalone prompt, interactive TUI, and daemon/session paths where applicable.
- [ ] Persist or log session metadata needed for replay and debugging.

## Phase 3: Verification and Documentation

- [ ] Add integration tests for the primary successful path and at least one failure path.
- [ ] Update README/docs and any relevant built-in tool or command lists.
- [ ] Run `cargo fmt`, targeted `cargo nextest`, `cargo check --tests`, and `git diff --check`.
