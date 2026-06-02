## ADDED Requirements

### Requirement: Reusable slash policy returns neutral effects [r[neutral-display-protocol-effects.neutral-effects]]

Reusable slash and mode policy MUST return neutral action/effect DTOs rather than TUI display DTOs or daemon protocol commands.

#### Scenario: selected command family is protocol-neutral [r[neutral-display-protocol-effects.neutral-effects.protocol-neutral]]
- GIVEN a selected slash or mode command family is evaluated
- WHEN reusable policy returns its decision
- THEN the decision MUST be expressed as neutral effect data
- AND protocol/display constructors MUST live only in projection adapters

### Requirement: Projection adapters own display/protocol constructors [r[neutral-display-protocol-effects.projection-adapters]]

Standalone, local attach, remote attach, and daemon adapters MUST project neutral effects into `clanker-tui-types` or `clankers_protocol` values at the edge.

#### Scenario: parity paths use the same neutral effect [r[neutral-display-protocol-effects.projection-adapters.parity]]
- GIVEN standalone and attach paths process the selected command
- WHEN they project the neutral effect
- THEN both paths MUST share policy and differ only in edge projection

### Requirement: Display/protocol drain preserves parity [r[neutral-display-protocol-effects.verification]]

Validation MUST prove parity and prevent display/protocol DTO leakage returning to selected policy modules.

#### Scenario: rail rejects inward constructors [r[neutral-display-protocol-effects.verification.rail]]
- GIVEN source rails inspect the selected policy owner
- WHEN TUI or protocol constructors appear there
- THEN the rail MUST fail and name the projection adapter owner
