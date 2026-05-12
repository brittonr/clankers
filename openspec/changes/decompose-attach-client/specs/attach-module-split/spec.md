## ADDED Requirements

### Requirement: Attach Client Module Decomposition [r[attach-client.decomposition]]

Attach mode MUST be split into focused modules while preserving daemon/session command parity, local TUI behavior, and MCP/attach equivalence evidence.

#### Scenario: Command parity preserved [r[attach-client.decomposition.scenario.1]]

- GIVEN attach local commands and MCP session-control commands share existing parity fixtures
- WHEN attach mode is decomposed
- THEN prompt, abort, capabilities, confirmations, compaction, and history commands still map to the same SessionCommand semantics

#### Scenario: Recovery behavior preserved [r[attach-client.decomposition.scenario.2]]

- GIVEN a daemon session is missing, suspended, or reconnecting
- WHEN an attach client resolves or recovers the session
- THEN the user-visible recovery/status behavior matches the existing attach path
