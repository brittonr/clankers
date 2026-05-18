## Phase 1: Deterministic replay backbone

- [ ] [serial] Add fixture schema and fixture files for a minimal user → model tool-call → tool result → model final-answer turn.
- [ ] [serial] Add a credential-free fake provider/test adapter that records completion requests and returns scripted responses from the fixture.
- [ ] [serial] Add deterministic fake tool execution for at least one successful tool and one failure/correlation edge case.
- [ ] [serial] Add output normalization and BLAKE3 receipt hashing for transcripts, event streams, provider requests, and tool results.
- [ ] [serial] Add replay-equivalence tests that run each fixture twice in isolated temp state and assert identical normalized outputs and hashes.

## Phase 2: Harness integration

- [ ] [depends:replay-tests] Add `deterministic` to `scripts/test-harness.sh` as a cheap profile for the deterministic replay rail.
- [ ] [depends:harness-profile] Extend harness contract tests and `list`/`profiles` output to cover the deterministic profile.
- [ ] [depends:harness-profile] Update `test-harness` spec text if implementation changes the canonical harness profile contract.

## Phase 3: Verification and landing

- [ ] [serial] Run focused verification: `cargo fmt --check`, deterministic replay test(s), harness contract test(s), `openspec validate add-deterministic-turn-replay --strict --json`, and `git diff --check`.
- [ ] [serial] Archive the completed OpenSpec change, validate affected specs, commit, and push.
