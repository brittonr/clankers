## ADDED Requirements

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
