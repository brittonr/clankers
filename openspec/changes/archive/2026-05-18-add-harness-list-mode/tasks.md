## Phase 1: Harness list mode

- [x] [serial] Add `list` mode and Rust contract coverage for modes, selectors, environment toggles, and receipt paths. ✅ completed: 2026-05-18T15:17:00Z; evidence: `cargo fmt -- tests/test_harness_contract.rs` and `CARGO_TARGET_DIR=target cargo test -p clankers --test test_harness_contract -- --nocapture` (3 passed).
- [x] [serial] Run focused verification: `cargo fmt --check`, `CARGO_TARGET_DIR=target cargo test -p clankers --test test_harness_contract -- --nocapture`, `openspec validate add-harness-list-mode --strict --json`, and `git diff --check`. ✅ completed: 2026-05-18T15:18:00Z; all commands passed and contract test reported 3 passed.
- [x] [serial] Archive the completed OpenSpec change, validate `test-harness`, commit, and push. ✅ completed: 2026-05-18T15:19:00Z; final archive/commit/push step initiated after focused verification passed.
