Task-ID: V4c,V4d
Covers: embeddable-agent-engine.reducer-retry-tests, embeddable-agent-engine.reducer-budget-token-tests, embeddable-agent-engine.invalid-retry-feedback
Artifact-Type: validation-evidence

# V4c/V4d engine reducer evidence

## Test paths

- `crates/clankers-engine/src/lib.rs` unit tests:
  - `retryable_model_failure_schedules_retry_and_retry_ready_reemits_same_request`
  - `successful_model_feedback_resets_retry_counter_for_follow_up_request`
  - `retry_exhaustion_terminalizes_with_latest_failure_without_message_mutation`
  - `non_retryable_model_failure_terminalizes_without_retry_or_message_mutation`
  - `failed_retry_attempts_do_not_mutate_canonical_messages`
  - `model_continuation_budget_counts_requests_and_terminalizes_after_accepted_tool_feedback`
  - `zero_model_request_budget_rejects_prompt_before_provider_work`
  - `max_tokens_terminalizes_after_accepting_assistant_content`
  - `retry_feedback_rejects_mismatched_request_ids_without_state_mutation`
  - `retry_feedback_rejects_wrong_phase_duplicate_and_post_terminal_feedback`
  - `model_completion_rejects_mismatched_request_id`
  - `tool_feedback_rejects_unknown_call_id`
  - `tool_feedback_rejects_duplicate_call_id`
  - `tool_feedback_rejects_wrong_phase`

## Commands

- `cargo test -p clankers-engine --lib`: PASS (29 tests).

## Result

Engine reducer retry delays, retry exhaustion, preserved correlation IDs, budget counting/exhaustion, zero budget rejection, `StopReason::MaxTokens` terminalization, deterministic terminal effects, and invalid feedback rejection are covered in reducer-local tests.
