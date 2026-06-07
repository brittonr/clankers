## ADDED Requirements

### Requirement: Controller concrete services are split behind ports [r[remaining-coupling-drain.controller-service-ports]]

`clankers-controller` MUST keep concrete agent, provider, DB, hook, session, and protocol dependencies behind separately testable runtime, persistence, hook, and projection ports rather than allowing command policy to own every layer.

#### Scenario: Controller dependency responsibilities are inventoried [r[remaining-coupling-drain.controller-service-ports.inventory]]
- GIVEN controller code imports concrete orchestration, storage, provider, hook, session, or protocol crates
- WHEN a controller drain slice begins
- THEN the slice MUST classify each touched edge as command translation, authorization, core input, runtime dispatch, persistence/search, hook dispatch, continuation, or projection
- AND it MUST name the target service port or projection owner for that edge

#### Scenario: Runtime adapter owns agent and provider execution [r[remaining-coupling-drain.controller-service-ports.runtime-adapter]]
- GIVEN controller command policy needs to execute prompts, controls, thinking changes, model metadata, or provider-backed runtime behavior
- WHEN the behavior crosses into the agent/provider runtime
- THEN the command policy MUST emit neutral intents or effects to a controller runtime adapter
- AND provider-native request or thinking compatibility types MUST NOT be required in command policy paths

#### Scenario: Persistence uses a session service port [r[remaining-coupling-drain.controller-service-ports.persistence-port]]
- GIVEN controller logic needs session history, summaries, replay, migration, search, or persistence
- WHEN that behavior is implemented
- THEN DB and session-format details MUST be isolated behind a typed persistence service port
- AND command/event processing code MUST consume neutral persistence results rather than opening stores directly

#### Scenario: Projection constructors remain edge-owned [r[remaining-coupling-drain.controller-service-ports.projection-owners]]
- GIVEN controller behavior emits user-visible or transport-visible output
- WHEN daemon, protocol, or TUI DTOs are constructed
- THEN constructors MUST remain in declared projection owners such as `convert.rs` or `transport_convert.rs`
- AND reusable command/runtime/persistence policy MUST emit neutral outputs

#### Scenario: Controller behavior validation covers the split [r[remaining-coupling-drain.controller-service-ports.behavior-validation]]
- GIVEN service ports replace direct dependencies
- WHEN focused tests run
- THEN command/effect/runtime adapter behavior, persistence replay/search behavior, and request-metadata resume behavior MUST match the previous user-visible contract

#### Scenario: Controller closeout validation runs [r[remaining-coupling-drain.controller-service-ports.closeout]]
- GIVEN the controller service-port change is ready to close
- WHEN closeout validation runs
- THEN FCIS shell-boundary rails, transport-construction rails, affected cargo checks, Cairn gates, Cairn validation, and diff checks MUST pass
