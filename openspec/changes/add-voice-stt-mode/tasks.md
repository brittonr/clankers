## Phase 1: Discovery and API Shape

- [x] Inventory existing clankers modules that should own Voice and Speech-to-Text Mode. ✅ completed: 2026-05-01T23:24:00Z
  - Evidence: `openspec/changes/add-voice-stt-mode/evidence/module-inventory.md` maps TTS ownership (`src/tools/tts.rs`, `crates/clankers-tts`), shared tool publication (`src/modes/common.rs`), CLI command surfaces, TUI/daemon/session paths, Matrix media boundaries, and safe `ToolResult::details` replay metadata; it recommends a small first-pass policy module before real microphone/cloud transcription backends.
- [ ] Define the user-facing CLI/TUI/tool/config surface and document unsupported first-pass cases.
- [ ] Add focused tests for parsing, configuration, and policy boundaries.

## Phase 2: Implementation

- [ ] Implement the minimal backend or adapter for Voice and Speech-to-Text Mode.
- [ ] Wire the capability through standalone prompt, interactive TUI, and daemon/session paths where applicable.
- [ ] Persist or log session metadata needed for replay and debugging.

## Phase 3: Verification and Documentation

- [ ] Add integration tests for the primary successful path and at least one failure path.
- [ ] Update README/docs and any relevant built-in tool or command lists.
- [ ] Run `cargo fmt`, targeted `cargo nextest`, `cargo check --tests`, and `git diff --check`.
