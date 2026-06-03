## ADDED Requirements

### Requirement: Controller concrete ownership is inventoried [r[sdk-controller-runtime-boundary.inventory]]

`clankers-controller` MUST inventory concrete agent, session, database/search, hook, provider, display, and protocol dependencies with an owner and convergence condition.

#### Scenario: controller fields have owners [r[sdk-controller-runtime-boundary.inventory.fields]]
- GIVEN `SessionController` stores a concrete dependency
- WHEN architecture inventory runs
- THEN the dependency MUST be classified as command state, runtime adapter, persistence service, hook service, or projection edge
- AND reusable command policy MUST name the neutral replacement path

### Requirement: Runtime execution is adapter-owned [r[sdk-controller-runtime-boundary.runtime-adapter]]

Controller prompt and control execution MUST go through a runtime/session adapter seam rather than direct concrete `Agent` mutation in reusable command policy.

#### Scenario: production prompt path uses injected adapter [r[sdk-controller-runtime-boundary.runtime-adapter.production-injection]]
- GIVEN a production session command submits or controls a prompt
- WHEN command handling dispatches runtime work
- THEN it MUST call an injected controller runtime adapter or documented compatibility owner
- AND command policy MUST NOT directly construct provider requests, tool maps, or desktop runtime services

### Requirement: Persistence and projection stay single-purpose [r[sdk-controller-runtime-boundary.persistence]]

Session persistence/search and daemon/TUI/protocol projection MUST be owned by explicit services or conversion modules rather than mixed into command lifecycle logic.

#### Scenario: persistence is service-owned [r[sdk-controller-runtime-boundary.persistence.service-owned]]
- GIVEN controller command handling needs to load, save, search, or resume session state
- WHEN the behavior is reusable command policy
- THEN it MUST call a persistence/session service adapter
- AND direct `SessionManager` or search-index access MUST be limited to the compatibility owner

#### Scenario: projection is centralized [r[sdk-controller-runtime-boundary.projection.centralized]]
- GIVEN controller behavior emits user-visible or transport-visible output
- WHEN daemon/TUI/protocol DTOs are constructed
- THEN construction MUST go through the explicit conversion owner
- AND command policy MUST emit neutral semantic/domain events or runtime results first

### Requirement: Controller runtime boundary is verified [r[sdk-controller-runtime-boundary.verification]]

Verification MUST prove the runtime adapter seam without sockets/providers and preserve the agent-backed daemon path.

#### Scenario: fake runtime proves command lifecycle [r[sdk-controller-runtime-boundary.verification.fake-runtime]]
- GIVEN a fake runtime adapter records requests
- WHEN prompt, cancel, thinking, disabled tools, resume identity, and semantic events are exercised
- THEN controller command lifecycle MUST complete without sockets, providers, TUI state, or desktop storage

#### Scenario: agent-backed parity is preserved [r[sdk-controller-runtime-boundary.verification.agent-parity]]
- GIVEN the desktop daemon path uses an agent-backed adapter
- WHEN the migrated command path runs
- THEN prompt status, daemon events, persistence updates, and busy state MUST match the existing behavior contract
