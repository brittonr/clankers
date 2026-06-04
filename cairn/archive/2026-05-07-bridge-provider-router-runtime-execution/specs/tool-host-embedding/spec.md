## ADDED Requirements

### Requirement: Host-injected provider router execution
Clankers SHALL allow a host to execute provider/router requests through an explicitly injected runtime extension service instead of requiring ambient daemon/router/provider discovery.

#### Scenario: Embedded runtime fails closed without injected provider router
- **WHEN** an embedded/default-safe runtime provider-router execution is requested without an injected provider/router service
- **THEN** the request fails closed before daemon autostart, OAuth/login verifier writes, credential refresh persistence, or provider network execution can occur

#### Scenario: Desktop adapter executes through injected provider router
- **WHEN** the desktop runtime services are constructed with an explicit provider/router implementation
- **THEN** provider execution is routed through that implementation
- **AND** the receipt includes only sanitized status, safe identifiers, and aggregate stream/event counts
- **AND** the receipt excludes raw prompts, provider request bodies, model output, headers, tokens, environment values, and credentials
