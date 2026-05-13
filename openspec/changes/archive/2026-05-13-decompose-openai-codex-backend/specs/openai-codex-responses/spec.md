## ADDED Requirements

### Requirement: OpenAI Codex Backend Decomposition [r[openai-codex.decomposition]]

The OpenAI Codex backend MUST be decomposed into focused provider modules that preserve fail-closed entitlement/auth behavior and streaming normalization.

#### Scenario: Entitlement/auth fail closed [r[openai-codex.decomposition.scenario.1]]

- GIVEN a Codex account is not entitled or auth probing fails
- WHEN the decomposed backend handles a request
- THEN the provider returns the same explicit unavailable/fail-closed outcome without falling back to API-key OpenAI

#### Scenario: Streaming normalization parity [r[openai-codex.decomposition.scenario.2]]

- GIVEN a Responses API stream includes text, reasoning, errors, or provider metadata
- WHEN the decomposed backend consumes the stream
- THEN the emitted StreamEvent sequence and safe metadata match existing router/provider tests
