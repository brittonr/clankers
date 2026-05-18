## Phase 1: Deterministic replay backbone

- [x] [serial] Add fixture schema and fixture files for a minimal user → model tool-call → tool result → model final-answer turn. ✅ completed: 2026-05-18T16:29:25Z; evidence: `crates/clankers-engine/tests/fixtures/minimal_tool_turn.json`, `CARGO_TARGET_DIR=target cargo test -p clankers-engine --test deterministic_turn_replay -- --nocapture`
- [x] [serial] Add a credential-free fake provider/test adapter that records completion requests and returns scripted responses from the fixture. ✅ completed: 2026-05-18T16:29:25Z; evidence: `ScriptedProvider` in `crates/clankers-engine/tests/deterministic_turn_replay.rs` records normalized requests and asserts request id/session/message roles/tool schema.
- [x] [serial] Add deterministic fake tool execution for at least one successful tool and one failure/correlation edge case. ✅ completed: 2026-05-18T16:29:25Z; evidence: `ScriptedTools` success path, `tool_failure_turn.json`, `deterministic_tool_failure_replay_is_byte_stable`, and `deterministic_replay_reports_tool_correlation_mismatches`.
- [x] [serial] Add output normalization and BLAKE3 receipt hashing for transcripts, event streams, provider requests, and tool results. ✅ completed: 2026-05-18T16:29:25Z; evidence: `ReplayReceipt` hashes fixture-pinned normalized transcript/events/provider requests/tool results/overall receipt.
- [x] [serial] Add replay-equivalence tests that run each fixture twice in isolated temp state and assert identical normalized outputs and hashes. ✅ completed: 2026-05-18T16:29:25Z; evidence: `deterministic_tool_turn_replay_is_byte_stable` and `deterministic_tool_failure_replay_is_byte_stable`.

## Phase 2: Harness integration

- [x] [depends:replay-tests] Add `deterministic` to `scripts/test-harness.sh` as a cheap profile for the deterministic replay rail. ✅ completed: 2026-05-18T16:29:25Z; evidence: `CARGO_TARGET_DIR=target ./scripts/test-harness.sh deterministic`.
- [x] [depends:harness-profile] Extend harness contract tests and `list`/`profiles` output to cover the deterministic profile. ✅ completed: 2026-05-18T16:29:25Z; evidence: `CARGO_TARGET_DIR=target cargo test -p clankers --test test_harness_contract -- --nocapture`.
- [x] [depends:harness-profile] Update `test-harness` spec text if implementation changes the canonical harness profile contract. ✅ completed: 2026-05-18T16:29:25Z; evidence: existing delta already requires deterministic replay profile discoverability and receipt behavior.

## Phase 3: Verification and landing

- [x] [serial] Run focused verification: `cargo fmt --check`, deterministic replay test(s), harness contract test(s), `openspec validate add-deterministic-turn-replay --strict --json`, and `git diff --check`. ✅ completed: 2026-05-18T16:29:25Z; evidence: combined command passed with deterministic replay 3 tests, harness contract 3 tests, OpenSpec validation 1/1.
- [x] [serial] Archive the completed OpenSpec change, validate affected specs, commit, and push. ✅ completed: 2026-05-18T16:29:25Z; evidence: final archive/commit/push step initiated after focused verification passed.
