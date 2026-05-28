# slash-command-composition Specification

## Requirements

### Requirement: Slash command kit detects conflicts and route drift

The slash-command-routing-kit SHALL validate registry conflict handling, prompt-template fallback, and attach routing boundaries.

#### Scenario: slash-command-routing-kit.boundary
- GIVEN slash commands are parsed or routed from attach mode
- WHEN a local, daemon-forwarded, plugin, or prompt-template path is selected
- THEN routing MUST use the explicit boundary for that path.

#### Scenario: slash-command-routing-kit.evidence
- GIVEN the kit fixture runs
- WHEN command conflicts and prompt templates are exercised
- THEN deterministic tests MUST cover priority, fallback, and oversized input cases.

#### Scenario: slash-command-routing-kit.drift
- GIVEN slash routing source, attach routing, docs, or specs drift
- WHEN `scripts/check-slash-command-routing-kit.rs` runs
- THEN the checker MUST fail until the artifacts are updated together.
