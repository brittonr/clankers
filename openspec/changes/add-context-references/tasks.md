## Phase 1: Discovery and API Shape

- [x] Inventory existing clankers modules that should own Context References. ✅ completed: 2026-05-01T00:42:15Z; elapsed: 2m28s; evidence: `openspec/changes/add-context-references/evidence/module-inventory.md` plus delegated read-only inspection.
- [x] Define the user-facing CLI/TUI/tool/config surface and document unsupported first-pass cases. ✅ completed: 2026-05-01T00:45:01Z; elapsed: 1m51s; evidence: `openspec/changes/add-context-references/evidence/api-surface.md`, design decision 3, and narrowed delta spec capability wording.
- [x] Add focused tests for parsing, configuration, and policy boundaries. ✅ completed: 2026-05-01T00:50:06Z; evidence: `cargo fmt` and `CARGO_TARGET_DIR=target cargo nextest run -p clankers-util at_file --no-fail-fast` (16 passed).

## Phase 2: Implementation

- [ ] Implement the minimal backend or adapter for Context References.
- [ ] Wire the capability through standalone prompt, interactive TUI, and daemon/session paths where applicable.
- [ ] Persist or log session metadata needed for replay and debugging.

## Phase 3: Verification and Documentation

- [ ] Add integration tests for the primary successful path and at least one failure path.
- [ ] Update README/docs and any relevant built-in tool or command lists.
- [ ] Run `cargo fmt`, targeted `cargo nextest`, `cargo check --tests`, and `git diff --check`.
