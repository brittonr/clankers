## Phase 1: Discovery and API Shape

- [x] Inventory existing clankers modules that should own External Memory Providers. ✅ completed: 2026-05-01T02:30:24Z
  - Evidence: `openspec/changes/add-external-memory-providers/evidence/module-inventory.md` maps existing local memory, config, tool publication, prompt/session integration, metadata, and test ownership boundaries.
- [x] Define the user-facing CLI/TUI/tool/config surface and document unsupported first-pass cases. ✅ completed: 2026-05-01T02:31:39Z
  - Evidence: `openspec/changes/add-external-memory-providers/evidence/api-surface.md` defines disabled-by-default `externalMemory` config, `external_memory` tool actions, TUI/slash UX, metadata contract, and unsupported cases.
- [ ] Add focused tests for parsing, configuration, and policy boundaries.

## Phase 2: Implementation

- [ ] Implement the minimal backend or adapter for External Memory Providers.
- [ ] Wire the capability through standalone prompt, interactive TUI, and daemon/session paths where applicable.
- [ ] Persist or log session metadata needed for replay and debugging.

## Phase 3: Verification and Documentation

- [ ] Add integration tests for the primary successful path and at least one failure path.
- [ ] Update README/docs and any relevant built-in tool or command lists.
- [ ] Run `cargo fmt`, targeted `cargo nextest`, `cargo check --tests`, and `git diff --check`.
