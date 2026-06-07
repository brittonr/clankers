## ADDED Requirements

### Requirement: Agent concrete ports drain by dependency family [r[remaining-coupling-drain.agent-concrete-ports]]

Clankers MUST reduce `clankers-agent` concrete dependencies by moving reusable turn-policy behavior behind neutral service ports or DTOs and leaving concrete desktop/orchestration crates at named adapter seams.

#### Scenario: Current concrete dependency families are inventoried [r[remaining-coupling-drain.agent-concrete-ports.inventory]]
- GIVEN dependency ownership inventory reports concrete `clankers-agent` dependencies
- WHEN an agent drain slice begins
- THEN the slice MUST list every concrete dependency family it touches
- AND it MUST name the current import owner, target service port or neutral DTO owner, convergence condition, and focused validation rail

#### Scenario: Host-injected services replace concrete turn-policy dependencies [r[remaining-coupling-drain.agent-concrete-ports.host-injected-services]]
- GIVEN reusable turn policy needs prompt, skill, storage/search, hook, procmon, model-selection, cost, or utility behavior
- WHEN the dependency family is drained
- THEN the reusable policy MUST receive that behavior through an agent-owned service port, neutral DTO, or host-injected adapter
- AND concrete desktop or orchestration implementations MUST remain in app-edge construction modules

#### Scenario: Provider-native types stay in the model adapter [r[remaining-coupling-drain.agent-concrete-ports.provider-adapter-only]]
- GIVEN model execution still uses provider/router implementations
- WHEN source-boundary rails inspect reusable agent turn modules
- THEN provider-native request, stream, auth, routing, and router types MUST appear only in the declared model adapter seam
- AND reusable turn policy MUST use neutral model request, message, usage, and stream DTOs

#### Scenario: Concrete dependency budget decreases [r[remaining-coupling-drain.agent-concrete-ports.budget-decreases]]
- GIVEN a dependency family is drained
- WHEN the ownership rail regenerates its receipt
- THEN the concrete dependency budget MUST decrease, split into smaller tracked budgets, or record a narrower adapter exception with a new convergence condition
- AND source hashes and owner diagnostics MUST be included in deterministic evidence

#### Scenario: Agent port validation runs [r[remaining-coupling-drain.agent-concrete-ports.validation]]
- GIVEN implementation is complete for a dependency family
- WHEN focused validation runs
- THEN agent port tests, provider-neutral DTO rails, concrete-dependency budget rails, and affected caller checks MUST pass or record an explicit environmental limitation

#### Scenario: Agent port closeout is gated [r[remaining-coupling-drain.agent-concrete-ports.closeout]]
- GIVEN the change is ready to close
- WHEN closeout validation runs
- THEN Cairn gates, Cairn validation, source diff checks, and updated ownership receipts MUST pass
