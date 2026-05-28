## ADDED Requirements

### Requirement: Agent uses explicit runtime services [r[agent-runtime-shell.service-bundle]]

`clankers-agent` MUST obtain model execution, tool registry, storage, prompt/context, hook, skill, cost, and cancellation behavior through explicit service ports or adapter-owned DTO boundaries rather than direct concrete subsystem ownership in turn policy.

#### Scenario: Service ports are explicit [r[agent-runtime-shell.service-bundle.explicit-ports]]
- GIVEN an agent turn requires provider, tool, storage, prompt, hook, skill, cost, or cancellation behavior
- WHEN the turn policy accesses that behavior
- THEN it MUST do so through a named service port, engine-host adapter, or neutral DTO
- AND concrete desktop implementations MUST be assembled outside reusable turn policy

### Requirement: Agent turn policy stays port-owned [r[agent-runtime-shell.turn-policy]]

Reusable agent turn policy MUST NOT encode concrete provider/router/storage/TUI/runtime policy that can be tested as an independent brick.

#### Scenario: Concrete policy is outside turn helpers [r[agent-runtime-shell.turn-policy.port-owned]]
- GIVEN a turn helper builds model requests, executes tools, records usage, handles hooks, or updates transcript state
- WHEN source-boundary rails inspect the helper
- THEN concrete provider/router/auth, database/session, TUI display, prompt-file discovery, and skill-directory lookup code MUST be absent unless the module is a named adapter

### Requirement: Desktop agent remains a compatibility shell [r[agent-runtime-shell.compatibility]]

Existing Clankers shells MAY continue to construct `Agent`, but construction MUST be treated as app-edge wiring around reusable runtime services.

#### Scenario: Desktop shell assembles services [r[agent-runtime-shell.compatibility.desktop-shell]]
- GIVEN standalone, daemon, or attach modes create an agent
- WHEN concrete settings, provider, tools, session stores, hooks, skills, or cost tracking are needed
- THEN those concrete dependencies MUST be assembled in desktop/app-edge construction code
- AND the agent turn path MUST receive interfaces or neutral DTOs

### Requirement: Agent dependency budget is tracked [r[agent-runtime-shell.dependency-budget]]

Remaining concrete agent dependencies MUST have owner receipts and must not grow silently.

#### Scenario: Owner receipts explain concrete edges [r[agent-runtime-shell.dependency-budget.owner-receipts]]
- GIVEN architecture validation inventories `clankers-agent` dependencies
- WHEN a concrete dependency remains or a new one is introduced
- THEN the rail MUST report source crate, target crate, owning adapter, reason, and convergence condition
- AND unowned dependency growth MUST fail validation

### Requirement: Thin-agent verification is deterministic [r[agent-runtime-shell.verification]]

The migration MUST preserve existing behavior and prove at least one fake-service turn can run without concrete desktop systems.

#### Scenario: Fake-service turn runs [r[agent-runtime-shell.verification.fake-service-turn]]
- GIVEN fake runtime services provide model, tool, storage, prompt, hook, skill, cost, and cancellation behavior
- WHEN an agent turn runs
- THEN it MUST complete without live provider/router/auth, database/session files, prompt bundles, skills directories, daemon/TUI construction, or global service lookup

#### Scenario: Closeout preserves parity [r[agent-runtime-shell.verification.closeout]]
- GIVEN implementation is complete
- WHEN focused validation runs
- THEN existing agent turn parity, dependency ownership, Cairn validate/gates, and diff checks MUST pass
