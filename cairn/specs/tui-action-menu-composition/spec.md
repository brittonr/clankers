# tui-action-menu-composition Specification

## Requirements

### Requirement: tui-action-menu-kit validates typed actions and menu composition

The tui-action-menu-kit SHALL validate typed Action parsing, menu contribution conflicts, and hidden-menu rules.

#### Scenario: conflict-resolution
- GIVEN multiple menu contributors define the same action
- WHEN the leader menu resolves contributions
- THEN deterministic conflict-resolution diagnostics MUST name the winner.

#### Scenario: hidden-menu
- GIVEN a hidden-menu rule disables a contribution
- WHEN menu state is built
- THEN the hidden contribution MUST not be rendered or executed.
