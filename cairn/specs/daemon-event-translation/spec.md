# daemon-event-translation Specification

## Requirements

### Requirement: daemon-event-translation-kit preserves streaming-replay and app-edge semantics

The daemon-event-translation-kit SHALL centralize daemon-to-TUI event projection without leaking raw provider or session internals.

#### Scenario: streaming-replay
- GIVEN daemon history replay includes user, assistant, tool, branch, and compaction messages
- WHEN attach clients receive replayed events
- THEN translation MUST preserve streaming/replay order through shared conversion helpers.

#### Scenario: app-edge
- GIVEN daemon events include app-edge system messages or history boundary markers
- WHEN attach handles those events
- THEN app-edge handling MUST remain explicit and redacted.
