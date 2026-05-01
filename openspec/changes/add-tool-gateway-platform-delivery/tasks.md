## Phase 1: Discovery and API Shape

- [x] Inventory existing clankers modules that should own Tool Gateway and Platform Delivery. ✅ completed: 2026-05-01T23:10:00Z
  - Evidence: `openspec/changes/add-tool-gateway-platform-delivery/evidence/module-inventory.md` maps shared tool catalog ownership (`src/modes/common.rs`), scheduled prompt delivery (`src/tools/schedule.rs`, `src/modes/event_loop_runner/mod.rs`), daemon tool construction (`src/modes/daemon/socket_bridge.rs`), Matrix media/sendfile delivery (`src/modes/matrix_bridge/*`), tool-result metadata, and docs surfaces; it recommends a minimal policy/metadata module instead of rewriting Matrix or scheduler backends.
- [ ] Define the user-facing CLI/TUI/tool/config surface and document unsupported first-pass cases.
- [ ] Add focused tests for parsing, configuration, and policy boundaries.

## Phase 2: Implementation

- [ ] Implement the minimal backend or adapter for Tool Gateway and Platform Delivery.
- [ ] Wire the capability through standalone prompt, interactive TUI, and daemon/session paths where applicable.
- [ ] Persist or log session metadata needed for replay and debugging.

## Phase 3: Verification and Documentation

- [ ] Add integration tests for the primary successful path and at least one failure path.
- [ ] Update README/docs and any relevant built-in tool or command lists.
- [ ] Run `cargo fmt`, targeted `cargo nextest`, `cargo check --tests`, and `git diff --check`.
