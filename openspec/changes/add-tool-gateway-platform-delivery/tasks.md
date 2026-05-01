## Phase 1: Discovery and API Shape

- [x] Inventory existing clankers modules that should own Tool Gateway and Platform Delivery. ✅ completed: 2026-05-01T23:10:00Z
  - Evidence: `openspec/changes/add-tool-gateway-platform-delivery/evidence/module-inventory.md` maps shared tool catalog ownership (`src/modes/common.rs`), scheduled prompt delivery (`src/tools/schedule.rs`, `src/modes/event_loop_runner/mod.rs`), daemon tool construction (`src/modes/daemon/socket_bridge.rs`), Matrix media/sendfile delivery (`src/modes/matrix_bridge/*`), tool-result metadata, and docs surfaces; it recommends a minimal policy/metadata module instead of rewriting Matrix or scheduler backends.
- [x] Define the user-facing CLI/TUI/tool/config surface and document unsupported first-pass cases. ✅ completed: 2026-05-01T23:13:00Z
  - Evidence: `openspec/changes/add-tool-gateway-platform-delivery/evidence/api-surface.md` defines `clankers gateway status|validate`, a Specialty `tool_gateway` inspection/validation tool, local/session-only first-pass delivery, schedule metadata validation, Matrix bridge preservation, unsupported remote/platform cases, and replay-safe metadata fields.
- [x] Add focused tests for parsing, configuration, and policy boundaries. ✅ completed: 2026-05-01T23:07:26Z
  - Evidence: added `src/tool_gateway.rs` policy/metadata helpers and tests for toolset parsing, empty/unknown toolset rejection, local/session/Matrix delivery boundaries, unsupported remote target errors, and replay-safe sanitized error strings; added `gateway` CLI parser/help tests in `src/cli.rs`. Verification passed `cargo fmt` and `CARGO_TARGET_DIR=target cargo nextest run -p clankers gateway --no-fail-fast` (6 passed).

## Phase 2: Implementation

- [x] Implement the minimal backend or adapter for Tool Gateway and Platform Delivery. ✅ completed: 2026-05-01T23:16:13Z
  - Evidence: added `src/tools/tool_gateway.rs` as the first-pass validation adapter for `status` and `validate`, returning safe `ToolResult::details` metadata for supported local/session delivery and explicit unsupported remote/Matrix-outside-bridge cases. Verification passed `cargo fmt` and `CARGO_TARGET_DIR=target cargo nextest run -p clankers tool_gateway --no-fail-fast` (7 passed).
- [x] Wire the capability through standalone prompt, interactive TUI, and daemon/session paths where applicable. ✅ completed: 2026-05-01T23:17:00Z
  - Evidence: `src/tools/mod.rs` exports the `tool_gateway` adapter and `src/modes/common.rs` registers `ToolGatewayTool` as a Specialty tool in `build_tiered_tools`, the shared tool-construction path used by standalone prompts, interactive TUI, and daemon/session construction. `modes::common::tests::build_tiered_tools_publishes_tool_gateway_specialty_tool` passed in `CARGO_TARGET_DIR=target cargo nextest run -p clankers tool_gateway --no-fail-fast`.
- [x] Persist or log session metadata needed for replay and debugging. ✅ completed: 2026-05-01T23:18:00Z
  - Evidence: `src/tools/tool_gateway.rs` attaches serialized `GatewayValidation` to `ToolResult::details`; `openspec/changes/add-tool-gateway-platform-delivery/evidence/session-metadata.md` documents the safe metadata boundary and excluded secrets/platform payloads. Focused tests verify supported local metadata and unsupported remote metadata redaction.

## Phase 3: Verification and Documentation

- [x] Add integration tests for the primary successful path and at least one failure path. ✅ completed: 2026-05-01T23:18:03Z
  - Evidence: added `tests/gateway.rs` covering library validation and `ToolGatewayTool` execution for supported local/session delivery plus unsupported remote/webhook failure paths with safe metadata. Verification passed `cargo fmt`, `CARGO_TARGET_DIR=target cargo nextest run -p clankers --test gateway --no-fail-fast`, and `CARGO_TARGET_DIR=target cargo nextest run -p clankers gateway --no-fail-fast` (11 passed).
- [ ] Update README/docs and any relevant built-in tool or command lists.
- [ ] Run `cargo fmt`, targeted `cargo nextest`, `cargo check --tests`, and `git diff --check`.
