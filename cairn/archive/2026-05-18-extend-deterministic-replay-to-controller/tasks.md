## Phase 1: Controller replay foundation

- [x] [serial] Inspect existing `Agent`, `SessionController`, provider adapter, and tool-dispatch test seams and choose the smallest credential-free path. Evidence: `tests/embedded_controller.rs`, `crates/clankers-agent/src/turn/execution.rs`, and provider/tool traits inspected; root integration test chosen.
- [x] [serial] Add a deterministic controller/agent replay fixture or helper for one user → model tool-call → tool result → model final-answer turn. Evidence: `tests/controller_deterministic_replay.rs`.
- [x] [serial] Add a fake provider/adapter that records controller-built requests and returns scripted responses without live credentials or network. Evidence: `ScriptedProvider` in `tests/controller_deterministic_replay.rs`.
- [x] [serial] Add deterministic fake tool execution and at least one correlation or rejection assertion at the shell boundary. Evidence: `LookupOrderTool` asserts call id, session id, and fixed JSON input.
- [x] [serial] Normalize and BLAKE3-hash controller replay transcript/events/provider requests/tool results. Evidence: controller replay normalized receipt hash `966821dd7fac529fee8f3b08ef7edf1021451f9c8189840e92f858940d85b68d`.

## Phase 2: Harness integration

- [x] [depends:controller-replay] Add the controller replay test to `scripts/test-harness.sh deterministic`.
- [x] [depends:controller-replay] Extend harness contract coverage and list/profile text for engine + controller deterministic replay rails.

## Phase 3: Verification and landing

- [x] [serial] Run focused verification: `cargo fmt --check`, controller replay test(s), engine replay test(s), harness contract test(s), OpenSpec validation, and `git diff --check`.
- [x] [serial] Archive the completed OpenSpec change, validate affected specs, commit, and push. Evidence: completed after verification during landing.
