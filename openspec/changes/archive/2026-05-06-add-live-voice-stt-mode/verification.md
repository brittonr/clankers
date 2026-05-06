# Verification

## Drain Verification Matrix

| Rail | Command | Status | Scope rationale |
| --- | --- | --- | --- |
| format | `cargo fmt --check` | pass | Rust formatting for touched CLI/tool/model files. |
| focused unit/integration | `CARGO_TARGET_DIR=target cargo test --lib voice_mode -- --nocapture && CARGO_TARGET_DIR=target cargo test --test voice_mode -- --nocapture` | pass | Covers typed voice models, live capture receipts, Specialty tool actions, and transcript prompt metadata. |
| CLI smoke | `CARGO_TARGET_DIR=target cargo run --quiet --bin clankers -- voice start --enable --json \| python -m json.tool >/dev/null && CARGO_TARGET_DIR=target cargo run --quiet --bin clankers -- voice submit-transcript --transcript 'hello from voice' --json \| python -m json.tool >/dev/null` | pass | Exercises user-facing shared voice CLI surface and JSON receipts. |
| compile | `CARGO_TARGET_DIR=target cargo check --tests` | pass | Verifies affected crate/test targets compile. |
| openspec | `openspec validate add-live-voice-stt-mode --strict` | pass | Validates active delta before archive. |
| whitespace | `git diff --check` | pass | Confirms no whitespace errors before archive. |

## Inventory

Touched seams for `add-live-voice-stt-mode`:

- `src/voice_mode.rs` owns typed live capture policy/request/receipt models, explicit start/stop capture receipt helpers, and transcript-to-session-prompt handoff helpers.
- `src/tools/voice_mode.rs` exposes the Specialty tool actions `status`, `validate`, `start_capture`, `stop_capture`, and `submit_transcript`, with replay-safe `ToolResult::details`.
- `src/cli.rs` and `src/commands/voice.rs` expose shared CLI actions `voice start`, `voice stop`, and `voice submit-transcript` without bypassing normal command policy.
- `tests/voice_mode.rs` covers the integration-level tool/model behavior.
- `README.md` and `docs/src/reference/config.md` document the supported local-first behavior, explicit enablement, raw-audio retention policy, and metadata exclusions.
