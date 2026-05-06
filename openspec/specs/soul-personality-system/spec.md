# SOUL Personality System Specification

## Purpose

Define the first-pass SOUL/personality validation surface for local SOUL file discovery, safe personality preset validation, explicit unsupported cases, and replay-safe metadata without mutating active prompt assembly.
## Requirements
### Requirement: SOUL Personality System Capability [r[soul-personality-system.capability]]
The system MUST provide Add SOUL.md and personality presets as a first-class identity/persona layer.

#### Scenario: Primary path succeeds [r[soul-personality-system.scenario.primary-path]]
- GIVEN clankers is configured for the capability
- WHEN the user or agent invokes the documented primary path
- THEN clankers performs the operation and returns a structured, user-visible result

#### Scenario: Unsupported configuration is explicit [r[soul-personality-system.scenario.unsupported-config]]
- GIVEN the user invokes the capability without required configuration or platform support
- WHEN clankers cannot safely proceed
- THEN clankers MUST return an actionable error instead of silently falling back or dropping work

### Requirement: SOUL Personality System Session Observability [r[soul-personality-system.observability]]
The system MUST record enough normalized metadata for audit, replay, and troubleshooting without leaking secrets.

#### Scenario: Session records useful metadata [r[soul-personality-system.scenario.session-metadata]]
- GIVEN the capability runs inside a persisted session
- WHEN the operation completes or fails
- THEN the session record includes status, timing or backend identity when useful, and redacted error details when applicable

### Requirement: SOUL Personality System Verification [r[soul-personality-system.verification]]
The implementation MUST include automated tests and documentation for the supported first-pass behavior.

#### Scenario: Regression suite covers happy and failure paths [r[soul-personality-system.scenario.regression-suite]]
- GIVEN the feature is implemented
- WHEN the targeted test suite runs
- THEN tests cover at least one successful operation and one policy/configuration failure

### Requirement: SOUL Prompt Assembly [r[soul.prompt-assembly]]
The system MUST integrate validated local SOUL.md content and curated personality presets into system prompt assembly when enabled.

#### Scenario: Local soul discovered [r[soul.prompt-assembly.scenario.local-soul-discovered]]
- GIVEN SOUL integration is enabled and SOUL.md exists in an allowed discovery path
- WHEN a session prompt is assembled
- THEN clankers includes the SOUL content in the documented precedence order

#### Scenario: Disabled means absent [r[soul.prompt-assembly.scenario.disabled-means-absent]]
- GIVEN SOUL integration is disabled
- WHEN a prompt is assembled
- THEN clankers does not include SOUL or personality preset content

### Requirement: SOUL Precedence and Safe Metadata [r[soul.precedence-metadata]]
The system MUST define precedence relative to AGENTS.md/CLAUDE.md and record safe metadata about persona sources.

#### Scenario: Metadata safe [r[soul.precedence-metadata.scenario.metadata-safe]]
- GIVEN a SOUL source is included
- WHEN session metadata is recorded
- THEN clankers stores source kind, path hash or safe path, preset id, status, and precedence without raw persona text or secrets
