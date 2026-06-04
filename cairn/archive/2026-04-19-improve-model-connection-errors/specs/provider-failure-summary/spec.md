## ADDED Requirements

### Requirement: Exhaustion error includes per-provider details
When `RouterProvider::complete` exhausts all providers (primary + fallbacks), the error message SHALL include a per-provider breakdown listing each provider name, model ID, HTTP status, and a short reason.

#### Scenario: Two providers fail with different errors
- **WHEN** the primary provider returns 429 (rate limited) and the fallback returns 529 (overloaded)
- **THEN** the error message contains lines like:
  - `anthropic:claude-sonnet → 429 rate limited`
  - `openai:gpt-4o → 529 overloaded`

#### Scenario: Provider skipped due to cooldown
- **WHEN** a provider is in rate-limit cooldown and skipped without an HTTP call
- **THEN** the error summary includes the provider with a "in cooldown" indicator

#### Scenario: Single provider with no fallbacks
- **WHEN** only one provider is configured and it fails with status 500
- **THEN** the error message still includes the provider name and status, not just "All providers exhausted"

### Requirement: Last error status preserved on exhaustion
The `ProviderError` returned when all providers are exhausted SHALL carry the HTTP status code from the last attempted provider, not a generic status.

#### Scenario: Status code from last fallback propagates
- **WHEN** primary fails with 429 and fallback fails with 529
- **THEN** the returned `ProviderError` has `status == Some(529)`
