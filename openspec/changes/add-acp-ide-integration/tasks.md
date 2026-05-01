## Phase 1: Discovery and API Shape

- [x] Inventory existing clankers modules that should own ACP IDE Integration. ✅ completed: 2026-05-01T01:12:36Z; evidence: `openspec/changes/add-acp-ide-integration/evidence/acp-module-inventory.md` plus delegated read-only inspection.
- [x] Define the user-facing CLI/TUI/tool/config surface and document unsupported first-pass cases. ✅ completed: 2026-05-01T01:18:44Z; evidence: `openspec/changes/add-acp-ide-integration/evidence/api-surface.md`, design decisions 3-4, and narrowed delta spec capability wording.
- [x] Add focused tests for parsing, configuration, and policy boundaries. ✅ completed: 2026-05-01T01:24:31Z; evidence: `cargo fmt`; `CARGO_TARGET_DIR=target cargo nextest run -p clankers acp --no-fail-fast` passed (6 tests); `CARGO_TARGET_DIR=target cargo nextest run -p clankers cli::tests --no-fail-fast` passed (3 tests); `CARGO_TARGET_DIR=target cargo check --tests -p clankers` passed.

## Phase 2: Implementation

- [x] Implement the minimal backend or adapter for ACP IDE Integration. ✅ completed: 2026-05-01T01:31:17Z; evidence: `clankers acp serve` dispatches to a foreground stdio adapter; `src/modes/acp.rs` handles JSON request lines, initialize/session method responses, normalized metadata, and explicit unsupported-method errors; `CARGO_TARGET_DIR=target cargo nextest run -p clankers acp --no-fail-fast` passed (8 tests); `CARGO_TARGET_DIR=target cargo check --tests -p clankers` passed.
- [x] Wire the capability through standalone prompt, interactive TUI, and daemon/session paths where applicable. ✅ completed: 2026-05-01T01:35:02Z; evidence: ACP is wired as an explicit top-level `clankers acp serve` command through `src/cli.rs`, `src/main.rs`, and `src/commands/acp.rs`; the first pass intentionally does not add a model-callable tool or TUI slash command because ACP is an external editor transport; `CARGO_TARGET_DIR=target cargo nextest run -p clankers acp --no-fail-fast` and `CARGO_TARGET_DIR=target cargo check --tests -p clankers` passed.
- [x] Persist or log session metadata needed for replay and debugging. ✅ completed: 2026-05-01T01:39:42Z; evidence: ACP stdio command logs normalized request metadata (`source`, `method`, `status`, `transport`) without request params; `handle_json_line_with_metadata` is covered by `acp_json_line_returns_loggable_metadata_without_params`; `CARGO_TARGET_DIR=target cargo nextest run -p clankers acp --no-fail-fast` passed (9 tests); `CARGO_TARGET_DIR=target cargo check --tests -p clankers` passed.

## Phase 3: Verification and Documentation

- [ ] Add integration tests for the primary successful path and at least one failure path.
- [ ] Update README/docs and any relevant built-in tool or command lists.
- [ ] Run `cargo fmt`, targeted `cargo nextest`, `cargo check --tests`, and `git diff --check`.
