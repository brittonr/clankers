# Voice/STT API Surface

## First-pass CLI surface

```text
clankers voice status [--json]
clankers voice validate --input <SOURCE> [--reply <MODE>] [--json]
```

- `status` reports the voice/STT first-pass boundary: TTS exists, STT/live microphone capture is validation-only until a backend is configured.
- `validate` checks a normalized input source and reply mode without reading microphone devices, uploading audio, or contacting providers.
- `--input` accepts first-pass labels/paths for policy validation:
  - `file:<PATH>` or a plain local path: validates local-audio-file intent only.
  - `microphone`: explicitly unsupported until capture backend/device permissions are implemented.
  - `matrix`, `remote:*`, `http(s)://*`, `cloud:*`: explicitly unsupported in the first pass.
- `--reply` accepts `text` (default), `tts`, or `none`. `tts` is allowed as a reply-mode validation because the existing `tts` tool already owns synthesis; it does not imply an automatic live voice loop in the first pass.

## Agent/TUI/daemon surface

- Publish a Specialty tool, tentatively `voice_mode`, with `status` and `validate` actions once the policy module exists.
- Register the tool through `src/modes/common.rs` so prompt, TUI, and daemon/session paths share the same visibility and policy.
- Do not add background microphone capture to the TUI event loop in the first pass. A later live loop can hook into `src/modes/event_loop_runner/` with explicit start/stop commands and cancellation.
- Keep Matrix voice-message transcription as bridge-specific follow-up work. Outside an active Matrix implementation, `matrix` input returns an explicit unsupported result.

## Configuration surface

No required config for the first pass. Future config can introduce provider/device fields under a `voice` or `speechToText` section, but this slice should avoid accepting credential-bearing provider config until a backend exists.

## Unsupported first-pass cases

The following must return actionable unsupported errors rather than falling back silently:

- live microphone capture and wake/listen loops,
- cloud or local STT provider execution,
- remote HTTP/webhook/cloud audio inputs,
- Matrix/platform audio transcription outside a dedicated active bridge path,
- raw audio persistence or replay,
- credential/header/provider-token handling,
- automatic TTS reply loops that speak every assistant response.

## Safe metadata

Replay/debug metadata should include only normalized fields:

- source/action/status/backend,
- input kind (`file`, `microphone`, `matrix`, `remote`, etc.),
- reply mode (`text`, `tts`, `none`),
- supported flag,
- sanitized error kind/message,
- optional local file extension/size/duration later if measured without storing audio.

It must not store raw audio bytes, transcripts by default, full remote URLs, provider credentials, headers, device IDs, Matrix event payloads, or platform attachment contents.
