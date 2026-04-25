Task-ID: V4a1,V4a2,V4a3
Covers: embeddable-agent-engine.agent-default-assembly, embeddable-agent-engine.host-adapter-parity, embeddable-agent-engine.adapter-rail
Artifact-Type: validation-evidence

# V4a shared Agent host-runner parity evidence

## Test paths

- `crates/clankers-agent/src/turn/mod.rs` unit tests:
  - `run_turn_loop_executes_engine_requested_tool_roundtrip`
  - `run_turn_loop_feeds_tool_failures_back_through_engine`
  - `run_turn_loop_applies_model_switch_and_emits_usage_updates`
  - `run_turn_loop_preserves_capability_gate_denials_through_host_runner`
  - `run_turn_loop_dispatches_pre_tool_hooks_through_host_runner`
  - `turn_retry_recovers_on_second_attempt`
  - `turn_retry_non_retryable_error_skips_retry`
  - `turn_retry_cancellation_during_backoff`
  - `engine_retry_stop_policy_retryable_recovery_uses_engine_retry_effect`
  - `engine_retry_stop_policy_terminal_failures_preserve_original_error_and_messages`
  - `engine_retry_stop_policy_cancellation_during_retry_backoff_stops_retry_ready`
  - `engine_retry_stop_policy_zero_budget_rejects_before_provider_io`
  - `engine_retry_stop_policy_budget_exhaustion_accepts_tool_feedback_without_follow_up_model`
  - `engine_retry_stop_policy_max_tokens_terminalizes_without_follow_up_work`
- `crates/clankers-agent/src/turn/execution.rs` stream/adapter tests:
  - `provider_stream_normalizer_feeds_host_accumulator`
  - `completion_request_from_engine_request_converts_native_provider_messages`
  - `tool_host_outcome_round_trips_success_and_error_messages`

## Commands

- `cargo test -p clankers-agent --lib turn::`: PASS (54 tests).
- `cargo test -p clankers-controller --test fcis_shell_boundaries`: PASS (34 tests).
- `./scripts/check-llm-contract-boundary.sh`: PASS.
- `openspec validate extract-composable-engine-host --strict`: PASS.

## Result

The shared `run_turn_loop` host-runner path preserves streaming deltas, non-streaming final completion ordering, tool events and failures, usage updates/final summary, pre-tool hook dispatch, capability-gate denials, model switching, event ordering, sequential tool scheduling, shell-visible retry/backoff, cancellation during retry backoff, budget exhaustion, zero-budget rejection, token-limit terminalization, and terminal behavior.
