## MODIFIED Requirements

### Requirement: Root shell policy drains to owned bricks [r[remaining-coupling-drain.root-shell-policy]]

The root `clankers` crate MUST remain an application-edge shell: it may wire concrete services, but reusable domain policy, storage schemas, provider shaping, process-job policy, rendering semantics, and protocol conversion MUST live in named workspace crates or focused adapter modules with owner receipts.

#### Scenario: root module policy has an owner map [r[remaining-coupling-drain.root-shell-policy.root-module-ownership-map]]
- GIVEN a root `src/` module imports an internal workspace crate or constructs an edge DTO
- WHEN dependency ownership validation inventories that module
- THEN the module MUST be classified as shell wiring, edge projection, adapter exception, or temporary policy with a named drain target
- AND every temporary-policy row MUST include a convergence condition and focused validation path before the slice can close

#### Scenario: root policy drains by slice [r[remaining-coupling-drain.root-shell-policy.policy-slice-drain]]
- GIVEN a root module owns reusable behavior that can be expressed as a neutral service, DTO, or workspace brick
- WHEN a drain slice touches that behavior
- THEN the reusable behavior MUST move to the named owner or become a documented adapter exception
- AND root code MUST retain only parsing, service assembly, or projection responsibilities for that behavior
