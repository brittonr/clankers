## ADDED Requirements

### Requirement: Provider execution remains behind runtime service contracts
The embeddable runtime SHALL represent model/provider execution as host-owned runtime services and SHALL NOT expose CLI, TUI, daemon, ACP, MCP, or provider-adapter internals as the public embedding boundary.

#### Scenario: Neutral provider request remains adapter-free
- **WHEN** a host constructs a runtime provider execution request
- **THEN** the request uses neutral serializable fields rather than daemon protocol, TUI, CLI, ACP, MCP, or provider-adapter types
