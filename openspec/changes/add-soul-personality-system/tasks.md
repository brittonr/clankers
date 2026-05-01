## Phase 1: Discovery and API Shape

- [x] Inventory existing clankers modules that should own SOUL Personality System. ✅ completed: 2026-05-01T23:59:00Z
  - Evidence: `openspec/changes/add-soul-personality-system/evidence/module-inventory.md` maps prompt assembly ownership (`crates/clankers-agent/src/system_prompt.rs`), runtime prompt mutation seams, CLI/tool surfaces, daemon/TUI/session paths, config paths, and safe metadata boundaries for a first-pass local policy module.
- [ ] Define the user-facing CLI/TUI/tool/config surface and document unsupported first-pass cases.
- [ ] Add focused tests for parsing, configuration, and policy boundaries.

## Phase 2: Implementation

- [ ] Implement the minimal backend or adapter for SOUL Personality System.
- [ ] Wire the capability through standalone prompt, interactive TUI, and daemon/session paths where applicable.
- [ ] Persist or log session metadata needed for replay and debugging.

## Phase 3: Verification and Documentation

- [ ] Add integration tests for the primary successful path and at least one failure path.
- [ ] Update README/docs and any relevant built-in tool or command lists.
- [ ] Run `cargo fmt`, targeted `cargo nextest`, `cargo check --tests`, and `git diff --check`.
