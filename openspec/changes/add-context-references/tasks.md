## Phase 1: Discovery and API Shape

- [x] Inventory existing clankers modules that should own Context References. ✅ completed: 2026-05-01T00:42:15Z; elapsed: 2m28s; evidence: `openspec/changes/add-context-references/evidence/module-inventory.md` plus delegated read-only inspection.
- [x] Define the user-facing CLI/TUI/tool/config surface and document unsupported first-pass cases. ✅ completed: 2026-05-01T00:45:01Z; elapsed: 1m51s; evidence: `openspec/changes/add-context-references/evidence/api-surface.md`, design decision 3, and narrowed delta spec capability wording.
- [x] Add focused tests for parsing, configuration, and policy boundaries. ✅ completed: 2026-05-01T00:50:06Z; evidence: `cargo fmt` and `CARGO_TARGET_DIR=target cargo nextest run -p clankers-util at_file --no-fail-fast` (16 passed).

## Phase 2: Implementation

- [x] Implement the minimal backend or adapter for Context References. ✅ completed: 2026-05-01T00:51:02Z; evidence: `clankers-util::at_file` now returns expanded text, image blocks, and per-reference metadata with explicit unsupported/error states; verified by `CARGO_TARGET_DIR=target cargo nextest run -p clankers-util at_file --no-fail-fast`.
- [x] Wire the capability through standalone prompt, interactive TUI, and daemon/session paths where applicable. ✅ completed: 2026-05-01T00:54:26Z; evidence: prompt/print/json/inline now expand context references before dispatch; attach forwards expanded image references via daemon protocol; `cargo fmt`, `CARGO_TARGET_DIR=target cargo check -p clankers-controller -p clankers-util`, and `CARGO_TARGET_DIR=target cargo check --lib -p clankers` passed.
- [x] Persist or log session metadata needed for replay and debugging. ✅ completed: 2026-05-01T00:57:52Z; evidence: `SessionManager::record_custom` records `context_references` metadata for persisted TUI sessions and attach logs equivalent metadata before daemon submission; `CARGO_TARGET_DIR=target cargo nextest run -p clankers-session custom --no-fail-fast`, `cargo check -p clankers-session -p clankers-controller`, and `cargo check --lib -p clankers` passed.

## Phase 3: Verification and Documentation

- [x] Add integration tests for the primary successful path and at least one failure path. ✅ completed: 2026-05-01T01:00:11Z; evidence: `tests/context_references.rs`; `CARGO_TARGET_DIR=target cargo nextest run -p clankers --test context_references --no-fail-fast` passed (2 tests).
- [x] Update README/docs and any relevant built-in tool or command lists. ✅ completed: 2026-05-01T00:55:22Z; evidence: README documents local `@` reference syntax and unsupported first-pass cases; `docs/src/reference/request-lifecycle.md` documents prompt expansion and metadata ownership in standalone/daemon paths.
- [ ] Run `cargo fmt`, targeted `cargo nextest`, `cargo check --tests`, and `git diff --check`.
