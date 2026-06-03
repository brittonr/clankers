## ADDED Requirements

### Requirement: Duplicate provider/router concerns are inventoried [r[provider-router-abstraction-collapse.duplicate-inventory]]

Provider/router request, stream, auth, discovery, routing, retry, cost, and error concerns MUST have a single named policy owner or an explicit compatibility convergence condition.

#### Scenario: inventory names one owner per concern [r[provider-router-abstraction-collapse.duplicate-inventory.owner-map]]
- GIVEN provider and router code expose similar abstractions
- WHEN the inventory rail runs
- THEN each concern MUST have one policy owner
- AND compatibility-only adapters MUST be identified separately

### Requirement: Selected concern has one policy owner [r[provider-router-abstraction-collapse.single-owner]]

A selected provider/router concern MUST delegate policy to its single owner and avoid duplicate shaping, auth, routing, retry, fallback, or stream-normalization logic in compatibility layers.

#### Scenario: adapter is policy-thin [r[provider-router-abstraction-collapse.single-owner.policy-thin]]
- GIVEN the selected concern is exercised
- WHEN source and fixture rails inspect the adapter
- THEN the adapter MUST translate DTOs/errors/events only
- AND policy MUST remain with the named owner

### Requirement: Compatibility adapter remains covered [r[provider-router-abstraction-collapse.thin-adapter]]

Compatibility adapters MUST have literal fixtures or parity rails proving request/stream/error shapes stay synchronized with the owner.

#### Scenario: literal fixture catches drift [r[provider-router-abstraction-collapse.thin-adapter.literal-fixture]]
- GIVEN a request or stream contract changes
- WHEN adapter tests run
- THEN a pinned literal fixture or parity rail MUST catch missing fields or policy duplication

### Requirement: Provider/router validation passes [r[provider-router-abstraction-collapse.verification]]

Focused and broad provider/router validation MUST pass after collapsing a selected duplicate concern.

#### Scenario: focused adapter tests pass [r[provider-router-abstraction-collapse.verification.focused]]
- GIVEN the selected concern moved to a single owner
- WHEN focused provider/router tests run
- THEN they MUST cover initial, retry, refresh, and error projection paths when applicable
