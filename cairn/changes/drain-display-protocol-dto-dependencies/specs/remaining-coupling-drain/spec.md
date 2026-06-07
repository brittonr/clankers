## ADDED Requirements

### Requirement: Display and protocol DTO dependencies drain to edge adapters [r[remaining-coupling-drain.display-protocol-dependency-drain]]

Reusable policy crates MUST NOT depend on display-only or transport-only DTOs except through declared projection adapters; non-edge dependencies on `clanker-tui-types` and `clankers-protocol` MUST be inventoried, justified, or drained to neutral DTO owners.

#### Scenario: Display and protocol dependency inventory is explicit [r[remaining-coupling-drain.display-protocol-dependency-drain.inventory]]
- GIVEN workspace dependency inventory reports crates that depend on `clanker-tui-types` or `clankers-protocol`
- WHEN display/protocol coupling is reviewed
- THEN every touched dependency MUST be classified as display edge, transport edge, shared neutral DTO, or drain target
- AND drain targets MUST name a neutral owner, adapter edge, and focused validation path

#### Scenario: Display-only DTOs become neutral policy DTOs [r[remaining-coupling-drain.display-protocol-dependency-drain.neutral-display-dtos]]
- GIVEN reusable config, model-selection, procmon, util, plugin, controller, root, or runtime policy needs thinking, loop, progress, status, or display summary state
- WHEN the dependency is drained
- THEN reusable policy MUST use neutral DTOs or domain events
- AND TUI/display-specific projection MUST happen only in display-edge adapters

#### Scenario: Protocol DTOs stay at transport edges [r[remaining-coupling-drain.display-protocol-dependency-drain.protocol-edge]]
- GIVEN reusable logic emits events, responses, summaries, or commands
- WHEN wire protocol DTOs are constructed
- THEN construction MUST happen only in declared projection owners or transport adapters
- AND reusable logic MUST emit neutral domain outputs instead of protocol variants

#### Scenario: Rails catch new inward dependencies [r[remaining-coupling-drain.display-protocol-dependency-drain.rails]]
- GIVEN a reusable crate adds a display/protocol dependency or constructor
- WHEN architecture rails run
- THEN the rail MUST fail unless the dependency is a declared edge adapter or shared neutral DTO owner
- AND diagnostics MUST name the source owner, target DTO crate, and expected replacement path

#### Scenario: Display/protocol validation runs [r[remaining-coupling-drain.display-protocol-dependency-drain.validation]]
- GIVEN display/protocol DTO dependencies are drained or reclassified
- WHEN focused validation runs
- THEN source rails, constructor-owner rails, attach/daemon projection tests, and neutral DTO tests for touched crates MUST pass

#### Scenario: Display/protocol closeout is gated [r[remaining-coupling-drain.display-protocol-dependency-drain.closeout]]
- GIVEN the display/protocol dependency drain is ready to close
- WHEN closeout validation runs
- THEN affected cargo checks, Cairn gates, Cairn validation, and diff checks MUST pass
