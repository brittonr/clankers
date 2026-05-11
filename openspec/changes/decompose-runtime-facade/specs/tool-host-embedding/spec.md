## ADDED Requirements

    ### Requirement: Runtime Facade Module Decomposition [r[runtime-facade.decomposition]]

    The embeddable runtime facade MUST be split into stable public modules with root-level re-exports that preserve the existing host-facing API and default-safe behavior.

    #### Scenario: Public API preserved [r[runtime-facade.decomposition.scenario.1]]

- GIVEN an embedding host imports existing clankers-runtime root symbols
- WHEN the facade is split into modules
- THEN the imports continue to compile through root re-exports or documented compatibility aliases

#### Scenario: Default-safe services preserved [r[runtime-facade.decomposition.scenario.2]]

- GIVEN an embedding host builds a runtime without extension services
- WHEN the decomposed runtime constructs default services
- THEN router, auth, plugin, MCP, gateway, and external side effects remain disabled unless explicitly injected
