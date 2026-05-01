## Phase 1: Discovery and API Shape

- [x] Inventory existing clankers modules that should own ACP IDE Integration. ✅ completed: 2026-05-01T01:12:36Z; evidence: `openspec/changes/add-acp-ide-integration/evidence/acp-module-inventory.md` plus delegated read-only inspection.
- [x] Define the user-facing CLI/TUI/tool/config surface and document unsupported first-pass cases. ✅ completed: 2026-05-01T01:18:44Z; evidence: `openspec/changes/add-acp-ide-integration/evidence/api-surface.md`, design decisions 3-4, and narrowed delta spec capability wording.
- [ ] Add focused tests for parsing, configuration, and policy boundaries.

## Phase 2: Implementation

- [ ] Implement the minimal backend or adapter for ACP IDE Integration.
- [ ] Wire the capability through standalone prompt, interactive TUI, and daemon/session paths where applicable.
- [ ] Persist or log session metadata needed for replay and debugging.

## Phase 3: Verification and Documentation

- [ ] Add integration tests for the primary successful path and at least one failure path.
- [ ] Update README/docs and any relevant built-in tool or command lists.
- [ ] Run `cargo fmt`, targeted `cargo nextest`, `cargo check --tests`, and `git diff --check`.
