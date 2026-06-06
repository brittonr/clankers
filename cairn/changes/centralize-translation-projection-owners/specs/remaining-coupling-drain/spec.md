## MODIFIED Requirements

### Requirement: Controller command seams split by responsibility [r[remaining-coupling-drain.controller-command-seams]]

`clankers-controller` MUST keep command input translation, authorization, core reducer effect interpretation, runtime dispatch, persistence, continuation policy, and protocol/event projection in separately testable modules.

#### Scenario: projection constructors have one owner [r[remaining-coupling-drain.controller-command-seams.constructor-owners]]
- GIVEN controller, daemon, attach, TUI, provider, or session code needs to emit edge-specific DTOs
- WHEN source-boundary rails inventory constructor sites
- THEN reusable logic MUST emit neutral domain DTOs and edge-specific constructors MUST appear only in the declared projection owner module
- AND exceptions MUST be named adapter seams with focused tests
