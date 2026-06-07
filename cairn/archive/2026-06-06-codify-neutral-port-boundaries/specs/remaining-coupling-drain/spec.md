## MODIFIED Requirements

### Requirement: Agent concrete dependencies shrink behind ports [r[remaining-coupling-drain.agent-concrete-dependencies]]

`clankers-agent` MUST keep turn policy behind model, tool, storage, prompt, hook, skill, cost, and cancellation ports, and MUST reduce direct concrete dependencies on provider/router/DB/config/procmon/TUI systems as those adapters move to application edges.

#### Scenario: neutral ports separate policy from adapters [r[remaining-coupling-drain.agent-concrete-dependencies.port-boundary-rule]]
- GIVEN reusable agent, controller, runtime, or engine-host policy needs external behavior
- WHEN the seam is touched for decoupling
- THEN the policy MUST express the need as typed DTOs, effects, or service traits injected by the host
- AND concrete provider, tool, storage, hook, plugin, config, process, or display implementations MUST remain in named adapter modules
