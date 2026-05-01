## Phase 1: Discovery and API Shape

- [x] Inventory existing clankers modules that should own ACP IDE Integration. ✅ completed: 2026-05-01T01:12:36Z; evidence: `openspec/changes/add-acp-ide-integration/evidence/acp-module-inventory.md` plus delegated read-only inspection.
- [x] Define the user-facing CLI/TUI/tool/config surface and document unsupported first-pass cases. ✅ completed: 2026-05-01T01:18:44Z; evidence: `openspec/changes/add-acp-ide-integration/evidence/api-surface.md`, design decisions 3-4, and narrowed delta spec capability wording.
- [x] Add focused tests for parsing, configuration, and policy boundaries. ✅ completed: 2026-05-01T01:24:31Z; evidence: `cargo fmt`; `CARGO_TARGET_DIR=target cargo nextest run -p clankers acp --no-fail-fast` passed (6 tests); `CARGO_TARGET_DIR=target cargo nextest run -p clankers cli::tests --no-fail-fast` passed (3 tests); `CARGO_TARGET_DIR=target cargo check --tests -p clankers` passed.

## Phase 2: Implementation

- [x] Implement the minimal backend or adapter for ACP IDE Integration. ✅ completed: 2026-05-01T01:31:17Z; evidence: `clankers acp serve` dispatches to a foreground stdio adapter; `src/modes/acp.rs` handles JSON request lines, initialize/session method responses, normalized metadata, and explicit unsupported-method errors; `CARGO_TARGET_DIR=target cargo nextest run -p clankers acp --no-fail-fast` passed (8 tests); `CARGO_TARGET_DIR=target cargo check --tests -p clankers` passed.
- [ ] Wire the capability through standalone prompt, interactive TUI, and daemon/session paths where applicable.
- [ ] Persist or log session metadata needed for replay and debugging.

## Phase 3: Verification and Documentation

- [ ] Add integration tests for the primary successful path and at least one failure path.
- [ ] Update README/docs and any relevant built-in tool or command lists.
- [ ] Run `cargo fmt`, targeted `cargo nextest`, `cargo check --tests`, and `git diff --check`.
