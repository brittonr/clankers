## Phase 1: Contract and fixture shape

- [x] [serial] [covers=daemon-event-translation.daemon-event-translation-kit.boundary] [evidence=openspec validate brick-06-daemon-event-translation-kit --strict --json] Finalize the proposal, design, and delta spec for `daemon-event-translation-kit`.
- [x] [serial] [covers=daemon-event-translation.daemon-event-translation-kit.boundary] [evidence=source anchor readback] Identified the minimal anchors as `crates/clankers-controller/src/convert.rs` and `src/modes/attach/events.rs`; drained as a focused conversion fixture plus source/docs/OpenSpec drift rail.

## Phase 2: Implementation evidence

- [x] [serial] [covers=daemon-event-translation.daemon-event-translation-kit.evidence] [evidence=cargo test -p clankers-controller daemon_event_translation_kit_preserves_streaming_replay_and_app_edge_events] Added a positive streaming `DaemonEvent` → `TuiEvent` path and deterministic replay timestamp assertion.
- [x] [parallel] [covers=daemon-event-translation.daemon-event-translation-kit.evidence] [evidence=cargo test -p clankers-controller daemon_event_translation_kit_preserves_streaming_replay_and_app_edge_events] Added app-edge / replay-metadata negative assertions that return `None` or no replay events with redacted evidence.
- [x] [parallel] [covers=daemon-event-translation.daemon-event-translation-kit.drift] [evidence=./scripts/check-daemon-event-translation-kit.rs] Added the source/docs/spec drift rail and documented the brick in `docs/src/reference/daemon.md`.

## Phase 3: Validation and archive

- [x] [depends:implementation] [covers=daemon-event-translation.daemon-event-translation-kit.evidence] [evidence=TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= cargo test -p clankers-controller daemon_event_translation_kit_preserves_streaming_replay_and_app_edge_events] Run the focused verification for `daemon-event-translation-kit` and capture the command in the archive note.
- [x] [depends:implementation] [covers=daemon-event-translation.daemon-event-translation-kit.drift] [evidence=TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= cargo fmt --check && git diff --check] Run formatting and whitespace checks.
- [x] [depends:implementation] [covers=daemon-event-translation.daemon-event-translation-kit.boundary] [evidence=openspec validate daemon-event-translation --strict --json] Promote the spec delta, validate the canonical spec, and archive the change when complete.

Completed: 2026-05-19T02:50:05Z
