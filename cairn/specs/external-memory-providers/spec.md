# External Memory Providers Specification

## Purpose

This spec defines clankers support for provider-backed memory and personalization backends while preserving the curated local-memory boundary.
## Requirements
### Requirement: External Memory Providers Capability [r[external-memory-providers.capability]]

The system MUST provide a provider interface for remote memory/personalization backends while preserving curated local memory.

#### Scenario: Primary path succeeds [r[external-memory-providers.scenario.primary-path]]

- GIVEN clankers is configured for the capability
- WHEN the user or agent invokes the documented primary path
- THEN clankers performs the operation and returns a structured, user-visible result

#### Scenario: Unsupported configuration is explicit [r[external-memory-providers.scenario.unsupported-config]]

- GIVEN the user invokes the capability without required configuration or platform support
- WHEN clankers cannot safely proceed
- THEN clankers MUST return an actionable error instead of silently falling back or dropping work

### Requirement: External Memory Providers Session Observability [r[external-memory-providers.observability]]

The system MUST record enough normalized metadata for audit, replay, and troubleshooting without leaking secrets.

#### Scenario: Session records useful metadata [r[external-memory-providers.scenario.session-metadata]]

- GIVEN the capability runs inside a persisted session
- WHEN the operation completes or fails
- THEN the session record includes status, timing or backend identity when useful, and redacted error details when applicable

### Requirement: External Memory Providers Verification [r[external-memory-providers.verification]]

The implementation MUST include automated tests and documentation for the supported first-pass behavior.

#### Scenario: Regression suite covers happy and failure paths [r[external-memory-providers.scenario.regression-suite]]

- GIVEN the feature is implemented
- WHEN the targeted test suite runs
- THEN tests cover at least one successful operation and one policy/configuration failure

### Requirement: Remote External Memory Adapter [r[external-memory.remote-adapters]]
The system MUST support explicitly configured remote external memory search providers through a disabled-by-default adapter with timeout, credential, and result-limit policy.

#### Scenario: Remote search [r[external-memory.remote-adapters.scenario.remote-search]]
- GIVEN externalMemory is enabled for an HTTP provider with endpoint and credentialEnv
- WHEN the external_memory search action runs
- THEN clankers sends a bounded search request and returns bounded results with provider status metadata

#### Scenario: Missing credential fails closed [r[external-memory.remote-adapters.scenario.missing-credential-fails-closed]]
- GIVEN a remote provider requires credentialEnv and the variable is absent
- WHEN search is requested
- THEN clankers returns a configuration error before network contact

### Requirement: External Memory Prompt Injection Policy [r[external-memory.prompt-injection]]
The system MUST keep remote memory prompt injection opt-in, bounded, and auditable.

#### Scenario: Injection disabled [r[external-memory.prompt-injection.scenario.injection-disabled]]
- GIVEN a remote provider returns results and injectIntoPrompt is false
- WHEN a prompt turn starts
- THEN clankers does not inject remote memory into the prompt and records only tool-visible search results when explicitly called
