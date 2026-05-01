## Phase 1: Discovery and API Shape

- [x] Inventory existing clankers modules that should own Batch Processing and Trajectory Export. ✅ completed: 2026-05-01T02:09:31Z; evidence: `openspec/changes/add-batch-trajectory-runner/evidence/module-inventory.md` plus delegated read-only inspection.
- [x] Define the user-facing CLI/TUI/tool/config surface and document unsupported first-pass cases. ✅ completed: 2026-05-01T02:15:42Z; evidence: `openspec/changes/add-batch-trajectory-runner/evidence/api-surface.md` plus design/spec updates scoping the first pass to foreground local `clankers batch run`.
- [x] Add focused tests for parsing, configuration, and policy boundaries. ✅ completed: 2026-05-01T02:23:41Z; evidence: `src/modes/batch.rs` covers JSONL parsing, metadata validation, local path policy, and bounded concurrency; CLI tests cover `clankers batch run`; `cargo fmt`; `CARGO_TARGET_DIR=target cargo nextest run -p clankers batch --no-fail-fast` passed (9 tests); `CARGO_TARGET_DIR=target cargo check --tests -p clankers` passed.

## Phase 2: Implementation

- [x] Implement the minimal backend or adapter for Batch Processing and Trajectory Export. ✅ completed: 2026-05-01T02:36:51Z; evidence: `src/modes/batch.rs` adds bounded job execution via `BatchJobExecutor`, stable result ordering, JSONL trajectory rendering, and safe metadata; `cargo fmt`; `CARGO_TARGET_DIR=target cargo nextest run -p clankers batch --no-fail-fast` passed (11 tests); `CARGO_TARGET_DIR=target cargo check --tests -p clankers` passed.
- [ ] Wire the capability through standalone prompt, interactive TUI, and daemon/session paths where applicable.
- [ ] Persist or log session metadata needed for replay and debugging.

## Phase 3: Verification and Documentation

- [ ] Add integration tests for the primary successful path and at least one failure path.
- [ ] Update README/docs and any relevant built-in tool or command lists.
- [ ] Run `cargo fmt`, targeted `cargo nextest`, `cargo check --tests`, and `git diff --check`.
