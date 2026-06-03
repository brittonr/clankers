## ADDED Requirements

### Requirement: Agent concrete systems are edge-owned [r[sdk-agent-port-boundary.inventory]]

`clankers-agent` MUST track every concrete provider, config, database, hook, skill, prompt, model-selection, cost, and cancellation dependency that crosses into agent turn execution with an explicit owner and convergence condition.

#### Scenario: dependency inventory names owners [r[sdk-agent-port-boundary.inventory.owners]]
- GIVEN `clankers-agent` imports a concrete Clankers system
- WHEN architecture inventory runs
- THEN the inventory MUST classify the import as reusable policy, compatibility adapter, or app-edge shell
- AND concrete app-edge imports MUST name the adapter expected to own them

### Requirement: Reusable turn logic uses explicit ports [r[sdk-agent-port-boundary.ports]]

Reusable agent turn logic MUST receive model, tool, prompt/config, storage/search, hook, skill, cost, and cancellation behavior through explicit ports or service bundles rather than discovering or constructing desktop Clankers systems.

#### Scenario: services cross through ports [r[sdk-agent-port-boundary.ports.explicit-services]]
- GIVEN a turn helper needs model execution, storage, hooks, skills, usage, or cancellation
- WHEN the helper is in a reusable turn module
- THEN it MUST depend on a port, neutral DTO, or service bundle
- AND it MUST NOT instantiate provider discovery, DB handles, global settings, hook pipelines, or skill roots

#### Scenario: concrete adapters stay at shell edge [r[sdk-agent-port-boundary.ports.adapter-owned]]
- GIVEN desktop Clankers preserves existing `Agent` behavior
- WHEN concrete providers, settings, DB, hooks, or skills are supplied
- THEN they MUST be converted at root, daemon, controller, or runtime adapter construction edges
- AND the reusable turn path MUST observe only the selected port interface

### Requirement: Agent boundary rails preserve SDK shape [r[sdk-agent-port-boundary.rails]]

Boundary validation MUST fail when new concrete desktop dependencies enter reusable agent turn modules without an owner receipt.

#### Scenario: owner receipts explain remaining edges [r[sdk-agent-port-boundary.rails.owner-receipts]]
- GIVEN a concrete dependency remains in `clankers-agent`
- WHEN the lego architecture rail reports it
- THEN the diagnostic MUST include the dependency family, allowed owner module, and expected replacement path

### Requirement: Agent port migration preserves behavior [r[sdk-agent-port-boundary.verification]]

Every agent port migration MUST preserve model streaming, tool execution, retry, cancellation, usage, hook, and terminal outcomes for the desktop compatibility path.

#### Scenario: parity tests cover migrated services [r[sdk-agent-port-boundary.verification.parity]]
- GIVEN a concrete service family moves behind a port
- WHEN focused parity tests run through the compatibility adapter
- THEN user-visible events, persisted messages, and terminal prompt status MUST match the pre-migration contract

#### Scenario: boundary rail rejects regressions [r[sdk-agent-port-boundary.verification.boundary-rail]]
- GIVEN reusable turn modules import a forbidden concrete service
- WHEN validation runs
- THEN the rail MUST fail with the offending module, dependency family, owner, and requirement id
