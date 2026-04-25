# turn-level-retry Specification

## Purpose
Define turn-level retry behavior for transient model failures, ensuring engine-authoritative retry decisions, bounded retry budgets, message immutability during failed attempts, and cancellation support during backoff.
## Requirements
### Requirement: Agent loop retries retryable turn failures
`clankers-engine` SHALL decide whether a host-classified retryable model failure for an in-flight turn is authorized to retry or must terminalize, using engine-owned retry state and bounded retry policy. `clankers-agent::turn` SHALL execute the retry/backoff effects produced by the engine and SHALL NOT remain the authoritative owner of retry eligibility after the host has reported provider-classified failure details. The preserved default behavior is at most 2 additional attempts, with a 1-second delay before the first retry, a 4-second delay before the second retry, and no jitter.
r[turn-level-retry.engine-authoritative]

#### Scenario: Transient 502 recovered by retry
r[turn-level-retry.transient-retry]
- **WHEN** the host reports a retryable model failure (e.g., status 502) for a pending engine model request on the first attempt
- **AND** the engine retry budget for that pending model request still permits another attempt
- **THEN** the engine returns `EngineEffect::ScheduleRetry { request_id, delay }` for the same pending model request identity with a 1-second retry delay
- **AND** when the host waits for that effect, reports retry-ready, executes the re-emitted model request, and the second attempt succeeds, the turn loop continues normally as if the failed attempt never appended conversation messages

#### Scenario: retry budget is scoped to one pending model request
r[turn-level-retry.retry-budget-scope]
- **WHEN** a pending model request fails retryably and then later succeeds within its retry budget
- **THEN** all retry effects for that request preserve the same pending model request identity
- **THEN** that request's retry-attempt counter is cleared with the pending request
- **AND** if subsequent tool feedback causes the engine to mint a follow-up model request, that follow-up request starts with a fresh retry budget
- **AND** retry attempts for the earlier request do not reduce the follow-up request's retry budget

#### Scenario: All turn-level retries exhausted
r[turn-level-retry.retry-exhaustion]
- **WHEN** the host reports retryable model failures until the engine-owned retry budget is exhausted
- **THEN** the engine clears the pending model work and returns `EngineOutcome.terminal_failure = Some(EngineTerminalFailure { message, status, retryable })` using the latest host-supplied failure details for the host to propagate
- **THEN** the agent runtime does not emit another model request for that failed turn

#### Scenario: Non-retryable error skips retry
r[turn-level-retry.non-retryable]
- **WHEN** the host reports a non-retryable model failure (e.g., status 400) for a pending engine model request
- **THEN** the engine terminalizes the turn immediately without returning a retry effect
- **THEN** the agent runtime propagates the error without attempting a retry

#### Scenario: terminal model failures preserve original structured AgentError
r[turn-level-retry.structured-agent-error]
- **WHEN** a non-retryable model failure or retry-exhausted model failure terminalizes through engine policy
- **THEN** the `clankers-agent::turn` adapter retains the original structured `AgentError` as host-side data while asking the engine to authorize retry or terminalization
- **THEN** after engine terminalization, the adapter returns that original `AgentError` to the caller rather than reconstructing an error from `Notice` text or the engine terminal-failure sidecar
- **THEN** the engine terminal-failure sidecar remains engine-native audit/authorization data with `message`, `status`, and `retryable`, not a lossy replacement for the original shell error

### Requirement: Turn retry does not duplicate messages
A failed model attempt SHALL NOT append assistant or tool-result messages to canonical conversation history. The engine SHALL leave canonical messages unchanged while scheduling a retry or terminalizing a model failure, and only successful model or tool feedback may advance message history for the migrated slice.
r[turn-level-retry.no-duplicate-messages]

#### Scenario: Failed turn leaves messages unchanged
r[turn-level-retry.failed-attempt-no-message-mutation]
- **WHEN** the host reports a retryable model failure and the engine schedules a retry
- **THEN** the next engine state has the same canonical messages as before the failed attempt
- **AND** the shell-visible messages vector remains unchanged until a later successful model or tool result is accepted

### Requirement: Turn retry respects cancellation
Retry backoff waiting SHALL remain a host-shell effect, but cancellation during that wait MUST be reported back through the migrated engine cancellation path and surface as `AgentError::Cancelled` without executing the scheduled retry.
r[turn-level-retry.cancellation-during-backoff]

#### Scenario: User cancels during retry backoff
r[turn-level-retry.cancelled-retry-no-attempt]
- **WHEN** the engine schedules retry work after a retryable model failure
- **AND** the user cancels the operation while the host is waiting for the engine-specified backoff delay
- **THEN** the host reports cancellation through the engine cancellation input for the pending turn
- **THEN** the turn loop returns `AgentError::Cancelled` without attempting the retry

