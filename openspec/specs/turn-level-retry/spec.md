# turn-level-retry Specification

## Purpose
TBD - created by archiving change improve-model-connection-errors. Update Purpose after archive.
## Requirements
### Requirement: Agent loop retries retryable turn failures
When `execute_turn` returns a retryable error, `run_turn_loop` SHALL retry the turn up to 2 additional times with exponential backoff before propagating the error.

#### Scenario: Transient 502 recovered by retry
- **WHEN** `execute_turn` fails with a retryable error (e.g., status 502) on the first attempt
- **AND** the second attempt succeeds
- **THEN** the turn loop continues normally as if the first attempt never happened

#### Scenario: All turn-level retries exhausted
- **WHEN** `execute_turn` fails with a retryable error on all 3 attempts (1 original + 2 retries)
- **THEN** the error propagates to the caller with the last attempt's error details

#### Scenario: Non-retryable error skips retry
- **WHEN** `execute_turn` fails with a non-retryable error (e.g., status 400)
- **THEN** the error propagates immediately without retry attempts

### Requirement: Turn retry does not duplicate messages
A failed turn attempt SHALL NOT append any messages (assistant or tool result) to the conversation history. Only successful turns modify the message list.

#### Scenario: Failed turn leaves messages unchanged
- **WHEN** `execute_turn` fails with a retryable error
- **THEN** the messages vector has the same length and content as before the attempt

### Requirement: Turn retry respects cancellation
If the `CancellationToken` is cancelled during a turn retry backoff, the retry loop SHALL exit with `AgentError::Cancelled`.

#### Scenario: User cancels during retry backoff
- **WHEN** a turn fails with a retryable error and enters the backoff delay
- **AND** the user cancels the operation during the delay
- **THEN** the turn loop returns `AgentError::Cancelled` without attempting the retry

