# Voice/STT Session Metadata Boundary

## First-pass replay/debug metadata

`src/tools/voice_mode.rs` returns `ToolResult::details` for every `status` and `validate` execution by serializing `voice_mode::VoiceValidation`. This is the session metadata contract for first-pass voice/STT support.

The metadata includes only safe policy fields:

- `source`: fixed value `voice_mode`
- `action`: `validate`
- `status`: `success` or `unsupported`
- `backend`: `local-policy` or `matrix-existing-bridge`
- `input_kind`: `file`, `microphone`, `matrix`, or `remote`
- `input_label`: safe label such as `file:wav`, `microphone`, `matrix`, `https`, or `cloud`
- `reply_mode`: `text`, `tts`, or `none`
- `supported`: boolean
- `error_kind` and sanitized `error_message` for unsupported cases

## Data intentionally excluded

Voice/STT replay metadata must not include:

- raw audio bytes
- transcript text from a provider
- full local file paths
- remote URLs, bucket keys, webhook URLs, query strings, or credentials
- authorization headers or provider tokens
- Matrix event payloads or media IDs
- microphone device identifiers

## Failure-path behavior

Unsupported first-pass inputs still return structured metadata, but only with safe labels. For example, `https://token@example.test/audio.wav` records `input_kind = remote` and `input_label = https`, not the URL. Microphone and inactive Matrix inputs record explicit unsupported policy errors rather than pretending transcription happened.
