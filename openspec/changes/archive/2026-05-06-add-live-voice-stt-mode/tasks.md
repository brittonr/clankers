## Phase 1: Spec Foundation

- [x] Write proposal, design, tasks, and delta spec for `add-live-voice-stt-mode`.
- [x] Validate the OpenSpec package with `openspec validate add-live-voice-stt-mode --strict` and record any follow-up findings.

## Phase 2: Implementation

- [x] Inventory current `voice-mode` code/docs seams and record the exact files to touch. Evidence: `verification.md#inventory`.
- [x] Add typed policy/config/request/receipt models with unit tests. Evidence: `src/voice_mode.rs` live capture policy/request/receipt and transcript prompt models, plus focused `voice_mode::tests`.
- [x] Implement the first runtime/adapter slice behind deterministic fake tests. Evidence: `src/voice_mode.rs::start_capture`, `stop_capture`, and `session_prompt_from_transcript`, with `LocalFake` provider policy and deterministic tests.
- [x] Wire the feature through the shared clankers surface without bypassing daemon/session/tool policy. Evidence: `src/tools/voice_mode.rs` Specialty actions and `src/cli.rs`/`src/commands/voice.rs` CLI actions.
- [x] Update README and relevant docs for supported behavior, non-goals, and safety policy. Evidence: `README.md` and `docs/src/reference/config.md`.

## Phase 3: Verification and Closeout

- [x] Run targeted package/integration checks for the touched modules. Evidence: `verification.md#drain-verification-matrix`.
- [x] Run `cargo check --tests` for affected crates. Evidence: `verification.md#drain-verification-matrix`.
- [x] Run `git diff --check`. Evidence: `verification.md#drain-verification-matrix`.
- [x] Sync the delta spec into the canonical `voice-mode` spec and archive the change after implementation tasks complete. Evidence: archived change and canonical `openspec/specs/voice-mode/spec.md` validation.
