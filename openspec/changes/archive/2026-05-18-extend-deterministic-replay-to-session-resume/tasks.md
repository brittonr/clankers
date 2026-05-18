## Phase 1: Spec and seam inspection

- [x] [serial] Inspect session persistence, resume helpers, and existing session/controller tests to choose the smallest real resume path. Evidence: used `SessionManager::create/open/build_context/record_resume`, `Agent::seed_messages`, and embedded `SessionController` with persistence.

## Phase 2: Persisted replay implementation

- [x] [serial] Add a credential-free persisted-session/resume deterministic replay test. Evidence: `tests/session_resume_deterministic_replay.rs`.
- [x] [serial] Add scripted provider/tool fixtures that record resumed request shape without live credentials or network. Evidence: scripted provider/tool fixtures in the session replay test.
- [x] [serial] Normalize and BLAKE3-hash resumed replay artifacts, and assert replay equivalence across isolated runs. Evidence: pinned hash `ef5a4d5692c8902b374bf5a901572f86b931ec7a13e4e69b084cb891e2c5a11f`.
- [x] [serial] Fix any narrowly exposed session metadata/history propagation defects. Evidence: no production defect beyond existing controller/tool context fix from prior slice; this slice validates persisted resume propagation.

## Phase 3: Harness integration

- [x] [depends:persisted-replay] Add the session resume replay rail to `scripts/test-harness.sh deterministic`. Evidence: `deterministic session resume replay` harness step.
- [x] [depends:persisted-replay] Extend harness contract and list/profile text for engine, controller, and session-resume deterministic rails. Evidence: `tests/test_harness_contract.rs` assertions updated.

## Phase 4: Verification and landing

- [x] [serial] Run focused verification: formatting, engine/controller/session replay tests, harness contract, deterministic harness, OpenSpec validation, and `git diff --check`. Evidence: focused command suite passed before archive.
- [x] [serial] Archive the completed OpenSpec change, validate affected specs, commit, and push. Evidence: completed during landing.
