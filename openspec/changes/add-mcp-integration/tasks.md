## Phase 1: Discovery and API Shape

- [x] Inventory existing clankers modules that should own MCP Integration. ✅ 52s (started: 2026-04-30T22:21:54Z → completed: 2026-04-30T22:22:46Z) [evidence=evidence/mcp-module-inventory.md]
- [x] Define the user-facing CLI/TUI/tool/config surface and document unsupported first-pass cases. ✅ 1m21s (started: 2026-04-30T22:23:40Z → completed: 2026-04-30T22:25:01Z) [evidence=design.md, specs/integrations-mcp/spec.md]
- [ ] Add focused tests for parsing, configuration, and policy boundaries.

## Phase 2: Implementation

- [ ] Implement the minimal backend or adapter for MCP Integration.
- [ ] Wire the capability through standalone prompt, interactive TUI, and daemon/session paths where applicable.
- [ ] Persist or log session metadata needed for replay and debugging.

## Phase 3: Verification and Documentation

- [ ] Add integration tests for the primary successful path and at least one failure path.
- [ ] Update README/docs and any relevant built-in tool or command lists.
- [ ] Run `cargo fmt`, targeted `cargo nextest`, `cargo check --tests`, and `git diff --check`.
