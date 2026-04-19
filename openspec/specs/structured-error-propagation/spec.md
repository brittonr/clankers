# structured-error-propagation Specification

## Purpose
TBD - created by archiving change improve-model-connection-errors. Update Purpose after archive.
## Requirements
### Requirement: AgentError carries HTTP status and retryability
`AgentError::ProviderStreaming` SHALL include an `Option<u16>` HTTP status code and a `bool` retryability flag, preserving the structured context from the originating `ProviderError`.

#### Scenario: 429 from Anthropic reaches the controller with status intact
- **WHEN** Anthropic returns HTTP 429 and all retries are exhausted
- **THEN** the `AgentError` received by the controller has `status == Some(429)` and `retryable == true`

#### Scenario: 400 from Anthropic reaches the controller as non-retryable
- **WHEN** Anthropic returns HTTP 400 (bad request)
- **THEN** the `AgentError` received by the controller has `status == Some(400)` and `retryable == false`

#### Scenario: Network error without status code
- **WHEN** a connection timeout or DNS failure occurs
- **THEN** the `AgentError` has `status == None` and `retryable == true`

### Requirement: RouterCompatAdapter preserves error structure
`RouterCompatAdapter::complete` SHALL convert `clanker_router::Error` to `ProviderError` using the `From` impl that preserves status codes, not `provider_err(e.to_string())`.

#### Scenario: Router error with status 529 passes through adapter
- **WHEN** the router returns `Error::Provider { status: Some(529), .. }`
- **THEN** the resulting `ProviderError` has `status == Some(529)`

### Requirement: RpcProvider preserves error structure
`RpcProvider::complete` SHALL convert errors using a structured conversion that preserves status codes when available.

#### Scenario: RPC completion failure carries status
- **WHEN** the RPC daemon returns an error with status 429
- **THEN** the resulting `ProviderError` has `status == Some(429)` and `is_retryable()` returns true

### Requirement: Retry logging uses tracing
All retry logging in `anthropic/api.rs` SHALL use `tracing::warn!` instead of `eprintln!`.

#### Scenario: Retry log does not corrupt TUI
- **WHEN** the Anthropic HTTP client retries a 429 while the TUI is active
- **THEN** the retry message appears in the tracing log, not on raw stderr

### Requirement: SSE parse failures are logged
When `parse_sse_event` in `anthropic/streaming.rs` fails to deserialize an SSE event, it SHALL emit a `tracing::warn!` with the event type and parse error.

#### Scenario: Malformed SSE event logged
- **WHEN** the Anthropic API sends an SSE event with invalid JSON in the data field
- **THEN** a warning is logged with the event type and error message, and stream processing continues

#### Scenario: Consecutive parse failures surface as stream error
- **WHEN** 5 or more consecutive SSE events fail to parse
- **THEN** a `StreamEvent::Error` is sent through the channel with a message indicating persistent parse failures

