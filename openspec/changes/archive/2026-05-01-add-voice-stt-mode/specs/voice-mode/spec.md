## ADDED Requirements

### Requirement: Voice and Speech-to-Text Mode Capability [r[voice-mode.capability]]
The system MUST provide Add speech input/transcription and live voice loops on top of existing TTS.

#### Scenario: Primary path succeeds [r[voice-mode.scenario.primary-path]]
- GIVEN clankers is configured for the capability
- WHEN the user or agent invokes the documented primary path
- THEN clankers performs the operation and returns a structured, user-visible result

#### Scenario: Unsupported configuration is explicit [r[voice-mode.scenario.unsupported-config]]
- GIVEN the user invokes the capability without required configuration or platform support
- WHEN clankers cannot safely proceed
- THEN clankers MUST return an actionable error instead of silently falling back or dropping work

### Requirement: Voice and Speech-to-Text Mode Session Observability [r[voice-mode.observability]]
The system MUST record enough normalized metadata for audit, replay, and troubleshooting without leaking secrets.

#### Scenario: Session records useful metadata [r[voice-mode.scenario.session-metadata]]
- GIVEN the capability runs inside a persisted session
- WHEN the operation completes or fails
- THEN the session record includes status, timing or backend identity when useful, and redacted error details when applicable

### Requirement: Voice and Speech-to-Text Mode Verification [r[voice-mode.verification]]
The implementation MUST include automated tests and documentation for the supported first-pass behavior.

#### Scenario: Regression suite covers happy and failure paths [r[voice-mode.scenario.regression-suite]]
- GIVEN the feature is implemented
- WHEN the targeted test suite runs
- THEN tests cover at least one successful operation and one policy/configuration failure
