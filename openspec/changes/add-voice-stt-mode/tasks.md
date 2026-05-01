## Phase 1: Discovery and API Shape

- [x] Inventory existing clankers modules that should own Voice and Speech-to-Text Mode. ✅ completed: 2026-05-01T23:24:00Z
  - Evidence: `openspec/changes/add-voice-stt-mode/evidence/module-inventory.md` maps TTS ownership (`src/tools/tts.rs`, `crates/clankers-tts`), shared tool publication (`src/modes/common.rs`), CLI command surfaces, TUI/daemon/session paths, Matrix media boundaries, and safe `ToolResult::details` replay metadata; it recommends a small first-pass policy module before real microphone/cloud transcription backends.
- [x] Define the user-facing CLI/TUI/tool/config surface and document unsupported first-pass cases. ✅ completed: 2026-05-01T23:26:00Z
  - Evidence: `openspec/changes/add-voice-stt-mode/evidence/api-surface.md` defines `clankers voice status|validate`, a Specialty `voice_mode` status/validate tool, no required first-pass config, text/TTS/none reply-mode validation, and explicit unsupported cases for microphone loops, STT provider execution, remote/cloud audio, Matrix/platform audio outside a dedicated bridge, raw audio persistence, credential/header handling, and automatic spoken reply loops.
- [x] Add focused tests for parsing, configuration, and policy boundaries. ✅ completed: 2026-05-01T23:41:22Z
  - Evidence: added `src/voice_mode.rs` first-pass policy helpers and tests for local file input parsing, safe remote kind parsing, reply mode parsing, unsupported microphone policy, and replay-safe remote error metadata. Verification passed `cargo fmt` and `CARGO_TARGET_DIR=target cargo nextest run -p clankers voice_mode --no-fail-fast` (5 passed).

## Phase 2: Implementation

- [x] Implement the minimal backend or adapter for Voice and Speech-to-Text Mode. ✅ completed: 2026-05-01T23:50:27Z
  - Evidence: added `src/commands/voice.rs` for `clankers voice status|validate` and `src/tools/voice_mode.rs` for the Specialty `voice_mode` validation adapter; verification passed `cargo fmt` and `CARGO_TARGET_DIR=target cargo nextest run -p clankers voice --no-fail-fast` (9 passed).
- [x] Wire the capability through standalone prompt, interactive TUI, and daemon/session paths where applicable. ✅ completed: 2026-05-01T23:50:27Z
  - Evidence: wired `Commands::Voice` in `src/cli.rs`, main command dispatch in `src/main.rs`, tool exports in `src/tools/mod.rs`, and Specialty tool publication in `src/modes/common.rs`; shared tool construction covers standalone prompt, TUI, and daemon/session agent paths.
- [x] Persist or log session metadata needed for replay and debugging. ✅ completed: 2026-05-01T23:52:00Z
  - Evidence: `src/tools/voice_mode.rs` attaches serialized `VoiceValidation` to `ToolResult::details`; `openspec/changes/add-voice-stt-mode/evidence/session-metadata.md` documents the replay-safe metadata boundary and exclusions for raw audio, transcripts, full paths, URLs, credentials, and Matrix payloads.

## Phase 3: Verification and Documentation

- [ ] Add integration tests for the primary successful path and at least one failure path.
- [ ] Update README/docs and any relevant built-in tool or command lists.
- [ ] Run `cargo fmt`, targeted `cargo nextest`, `cargo check --tests`, and `git diff --check`.
