# Voice/STT Module Inventory

## Existing ownership candidates

- `src/tools/tts.rs` owns the current agent-facing text-to-speech surface. It already wraps `clankers_tts::TtsRouter`, emits progress, validates required text input, and returns user-visible audio-file output. Voice/STT should reuse this as the reply-output half rather than duplicating TTS provider routing.
- `crates/clankers-tts/` owns provider abstractions and concrete synthesis backends. It is TTS-only today (`TtsProvider`, `TtsRequest`, `TtsResponse`) and has no microphone capture or transcription trait. A first-pass STT policy/metadata module should stay outside this crate unless/until a real transcription provider abstraction is added.
- `src/modes/common.rs` owns shared tool construction (`build_tiered_tools`) used by prompt, TUI, and daemon/session paths. Any first-pass `voice`/`stt` Specialty tool should be registered there so it appears consistently across local and daemon execution.
- `src/cli.rs`, `src/main.rs`, and `src/commands/` own command parsing and command-mode execution. A first-pass CLI should prefer explicit validation/status commands over implicit microphone access.
- `src/modes/event_loop_runner/` owns interactive TUI event processing and user input submission. A later live voice loop should compose here, but the first pass should not add hidden microphone capture to the event loop.
- `src/modes/daemon/socket_bridge.rs` and controller/session paths own daemon/session construction. Because common tool construction already flows through daemon tool setup, a Specialty tool gives the first-pass capability daemon parity without special socket protocol changes.
- `src/modes/matrix_bridge/` already handles messaging-platform media/file boundaries. Voice-message transcription parity belongs here later, but the first pass should return explicit unsupported behavior for platform audio attachments unless an active bridge-specific implementation exists.
- `crates/clankers-session` / `ToolResult::details` own replay-visible tool metadata. Voice/STT metadata should record safe fields such as action/status/backend/input kind/audio duration or byte counts when known, and sanitized errors; it must not store raw audio, transcripts by default, provider credentials, or platform payloads.

## Recommended first-pass ownership

Add a small Rust module (for example `src/voice_mode.rs`) for policy parsing, input-source validation, and safe metadata. Add a minimal tool/CLI adapter only after the policy module has focused tests. Keep microphone capture, streaming voice loops, cloud transcription, Matrix voice messages, and raw audio persistence explicit unsupported cases until backend support is intentionally added.
