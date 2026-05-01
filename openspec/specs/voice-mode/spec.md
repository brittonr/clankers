## Purpose
Define the first-pass voice and speech-to-text validation capability on top of the existing TTS surface, including local file policy validation, explicit unsupported microphone/remote/provider cases, safe replay metadata, CLI/tool surfaces, tests, and documentation.

## Requirements

### Requirement: Voice and Speech-to-Text Mode Capability [r[voice-mode.capability]]
The system MUST provide a documented first-pass voice/STT validation surface without recording microphone input, reading raw audio bytes, or contacting transcription providers.

#### Scenario: Primary path succeeds [r[voice-mode.scenario.primary-path]]
- GIVEN clankers is configured with default first-pass voice/STT policy
- WHEN the user invokes `clankers voice validate --input <LOCAL_FILE>` or the agent invokes the `voice_mode` Specialty tool with a local file input
- THEN clankers returns a structured, user-visible supported result that identifies local file-policy validation and reply mode

#### Scenario: Unsupported configuration is explicit [r[voice-mode.scenario.unsupported-config]]
- GIVEN the user or agent invokes microphone capture, remote/cloud audio, provider transcription, Matrix audio outside an active bridge, or automatic spoken reply loops
- WHEN clankers cannot safely proceed in the first-pass implementation
- THEN clankers MUST return an actionable unsupported error instead of silently falling back, recording audio, contacting providers, or dropping work

### Requirement: Voice and Speech-to-Text Mode Session Observability [r[voice-mode.observability]]
The system MUST record enough normalized metadata for audit, replay, and troubleshooting without leaking secrets, raw audio, transcripts, full paths, URLs, credentials, or Matrix payloads.

#### Scenario: Session records useful metadata [r[voice-mode.scenario.session-metadata]]
- GIVEN the capability runs inside a persisted session via the `voice_mode` Specialty tool
- WHEN the operation completes or fails
- THEN the session record includes safe metadata such as source, action, status, backend identity, input kind/label, reply mode, supported flag, and sanitized error class/message when applicable

### Requirement: Voice and Speech-to-Text Mode Verification [r[voice-mode.verification]]
The implementation MUST include automated tests and documentation for the supported first-pass behavior.

#### Scenario: Regression suite covers happy and failure paths [r[voice-mode.scenario.regression-suite]]
- GIVEN the feature is implemented
- WHEN the targeted test suite runs
- THEN tests cover at least one successful local file validation and one policy/configuration failure, including replay-safe metadata boundaries
