## ADDED Requirements

### Requirement: Explicit Live Voice Capture [r[voice.live-capture]]
The system MUST provide an explicit start/stop voice capture flow with visible status and no background microphone capture by default.

#### Scenario: Start capture [r[voice.live-capture.scenario.start-capture]]
- GIVEN voice mode is enabled and the user starts capture
- WHEN audio capture begins
- THEN clankers shows active capture status and records safe source/session metadata

#### Scenario: Stop capture [r[voice.live-capture.scenario.stop-capture]]
- GIVEN capture is active
- WHEN the user stops capture
- THEN clankers stops the device stream and closes any provider request handles

### Requirement: STT to Session Prompt Flow [r[voice.stt-session]]
The system MUST route accepted transcripts into ordinary clankers prompt/session paths with configurable reply policy.

#### Scenario: Transcript prompt [r[voice.stt-session.scenario.transcript-prompt]]
- GIVEN STT produces a transcript and the user accepts or policy auto-submits it
- WHEN the transcript is submitted
- THEN clankers sends it as a normal user prompt and optionally emits TTS according to reply policy
