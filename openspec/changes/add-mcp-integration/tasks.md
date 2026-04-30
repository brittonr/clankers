## Phase 1: Discovery and API Shape

- [x] Inventory existing clankers modules that should own MCP Integration. ✅ 52s (started: 2026-04-30T22:21:54Z → completed: 2026-04-30T22:22:46Z) [evidence=evidence/mcp-module-inventory.md]
- [x] Define the user-facing CLI/TUI/tool/config surface and document unsupported first-pass cases. ✅ 1m21s (started: 2026-04-30T22:23:40Z → completed: 2026-04-30T22:25:01Z) [evidence=design.md, specs/integrations-mcp/spec.md]
- [x] Add focused tests for parsing, configuration, and policy boundaries. ✅ 2m6s (started: 2026-04-30T22:28:04Z → completed: 2026-04-30T22:30:10Z) [evidence=`CARGO_TARGET_DIR=target cargo nextest run -p clankers-config mcp_ --no-fail-fast`, `CARGO_TARGET_DIR=target cargo check --tests -p clankers-config`]

## Phase 2: Implementation

- [x] Implement the minimal backend or adapter for MCP Integration. ✅ 3m59s (started: 2026-04-30T22:30:10Z → completed: 2026-04-30T22:34:09Z) [evidence=`CARGO_TARGET_DIR=target cargo nextest run -p clankers mcp --no-fail-fast`]
- [x] Wire the capability through standalone prompt, interactive TUI, and daemon/session paths where applicable. ✅ 5m2s (started: 2026-04-30T22:36:17Z → completed: 2026-04-30T22:41:19Z) [evidence=`CARGO_TARGET_DIR=target cargo nextest run -p clankers mcp --no-fail-fast`, `CARGO_TARGET_DIR=target cargo nextest run -p clankers build_all_tiered_tools --no-fail-fast`, `CARGO_TARGET_DIR=target cargo check --tests -p clankers`]
- [x] Persist or log session metadata needed for replay and debugging. ✅ 1m47s (started: 2026-04-30T22:42:02Z → completed: 2026-04-30T22:43:49Z) [evidence=`CARGO_TARGET_DIR=target cargo nextest run -p clankers mcp --no-fail-fast`, `CARGO_TARGET_DIR=target cargo check --tests -p clankers`]

## Phase 3: Verification and Documentation

- [ ] Add integration tests for the primary successful path and at least one failure path.
- [ ] Update README/docs and any relevant built-in tool or command lists.
- [ ] Run `cargo fmt`, targeted `cargo nextest`, `cargo check --tests`, and `git diff --check`.
