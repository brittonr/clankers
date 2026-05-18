## Phase 1: Run-Scoped Receipts

- [x] [serial] Add harness run ID/run directory behavior while preserving stable compatibility receipt files. ✅ completed: 2026-05-18T14:56:36Z
- [x] [depends:run-receipts] Extend dry-run receipt contract tests for run-scoped artifacts and stable compatibility copies. ✅ completed: 2026-05-18T14:56:36Z
- [x] [depends:tests] Run focused harness tests, `openspec validate add-harness-run-receipts --strict --json`, and `git diff --check`. ✅ completed: 2026-05-18T14:57:06Z; evidence: `cargo fmt --check`, `CARGO_TARGET_DIR=target cargo test -p clankers --test test_harness_contract -- --nocapture`, `openspec validate add-harness-run-receipts --strict --json`, `git diff --check`
- [x] [depends:verify] Archive the completed OpenSpec change, commit, and push. ✅ completed: 2026-05-18T14:57:25Z; evidence: final archive/commit/push step initiated after all verification passed
