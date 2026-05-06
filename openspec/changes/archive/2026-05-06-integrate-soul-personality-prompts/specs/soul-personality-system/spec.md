## ADDED Requirements

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
