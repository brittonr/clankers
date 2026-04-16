## ADDED Requirements

### Requirement: Structured error taxonomy
The system SHALL classify API errors into a fixed taxonomy of failure reasons: Auth, AuthPermanent, Billing, RateLimit, Overloaded, ServerError, Timeout, ContextOverflow, ModelNotFound, FormatError, and Unknown.

#### Scenario: Rate limit classified
- **WHEN** an API call returns HTTP 429
- **THEN** the error is classified as `RateLimit`

#### Scenario: Billing exhaustion classified
- **WHEN** an API call returns HTTP 402 or an error body containing "insufficient credits"
- **THEN** the error is classified as `Billing`

#### Scenario: Context overflow classified
- **WHEN** an API call fails with a body containing "context length" or "too many tokens"
- **THEN** the error is classified as `ContextOverflow`

#### Scenario: Unknown error
- **WHEN** an API error does not match any known pattern
- **THEN** the error is classified as `Unknown` with `retryable: true`

---

### Requirement: Recovery hints
Each classified error SHALL carry recovery hints: `retryable` (bool), `should_compress` (bool), `should_rotate_credential` (bool), and `should_fallback` (bool). These hints SHALL be determined by the failure reason.

#### Scenario: Context overflow hints
- **WHEN** an error is classified as `ContextOverflow`
- **THEN** `should_compress` is true and `retryable` is true
- **THEN** `should_rotate_credential` is false

#### Scenario: Billing hints
- **WHEN** an error is classified as `Billing`
- **THEN** `should_rotate_credential` is true and `retryable` is false on the current credential
- **THEN** `should_fallback` is true

#### Scenario: Auth permanent hints
- **WHEN** an error is classified as `AuthPermanent`
- **THEN** `retryable` is false and `should_fallback` is true

---

### Requirement: Provider-specific pattern matching
The classifier SHALL include pattern sets for Anthropic, OpenAI, OpenRouter, and generic OpenAI-compatible endpoints. Pattern matching SHALL check HTTP status codes first, then fall back to error message body matching.

#### Scenario: Anthropic overloaded
- **WHEN** an Anthropic API call returns HTTP 529
- **THEN** the error is classified as `Overloaded`

#### Scenario: OpenAI model not found
- **WHEN** an OpenAI API call returns a body containing "model_not_found"
- **THEN** the error is classified as `ModelNotFound` with `should_fallback: true`

---

### Requirement: Disambiguation of ambiguous patterns
The classifier SHALL disambiguate patterns that could indicate either billing or rate limiting (e.g., "quota", "limit exceeded") by checking for transient signals ("try again", "resets at", "retry after"). Presence of transient signals SHALL classify as `RateLimit`; absence SHALL classify as `Billing`.

#### Scenario: Transient quota message
- **WHEN** an error body contains "quota exceeded" and "try again in 60 seconds"
- **THEN** the error is classified as `RateLimit`, not `Billing`

#### Scenario: Permanent quota message
- **WHEN** an error body contains "quota exceeded" with no transient signals
- **THEN** the error is classified as `Billing`
