## ADDED Requirements

### Requirement: Command responsibilities are inventoried [r[controller-command-responsibility-drain.responsibility-map]]

Controller command handling MUST identify owners for translation, authorization, core input construction, runtime dispatch, persistence, continuation, and projection responsibilities.

#### Scenario: inventory names each responsibility [r[controller-command-responsibility-drain.responsibility-map.named]]
- GIVEN `command.rs` handles a session command
- WHEN the inventory rail runs
- THEN each responsibility MUST have a named owner
- AND ambiguous ownership MUST produce a diagnostic with the expected replacement path

### Requirement: Command modules are single-purpose [r[controller-command-responsibility-drain.single-purpose-module]]

A controller command module SHOULD NOT own wire parsing, authorization, core input construction, runtime mutation, persistence, and daemon/TUI projection for the same behavior.

#### Scenario: extracted cluster has a narrow API [r[controller-command-responsibility-drain.single-purpose-module.narrow-api]]
- GIVEN a responsibility cluster is extracted
- WHEN tests exercise it
- THEN the module MUST expose a narrow API for that responsibility
- AND other responsibilities MUST remain in their owner modules

### Requirement: Projection remains centralized [r[controller-command-responsibility-drain.projection-owner]]

Command policy MUST use explicit projection owners for protocol, daemon, and TUI output.

#### Scenario: no ad hoc protocol reconstruction [r[controller-command-responsibility-drain.projection-owner.no-ad-hoc]]
- GIVEN command behavior emits user-visible or transport-visible output
- WHEN source rails inspect non-projection modules
- THEN they MUST NOT construct protocol DTOs that belong to projection owners

### Requirement: Controller seam validation passes [r[controller-command-responsibility-drain.verification]]

Focused and boundary validation MUST pass after each responsibility extraction.

#### Scenario: deterministic controller tests cover extraction [r[controller-command-responsibility-drain.verification.focused]]
- GIVEN a command responsibility moves
- WHEN focused tests run
- THEN they MUST cover the new owner and the controller call path
