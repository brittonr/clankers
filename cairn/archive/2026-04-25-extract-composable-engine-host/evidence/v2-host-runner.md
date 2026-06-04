Task-ID: V2a1,V2a2,V2a3,V2b,V2c
Covers: embeddable-agent-engine.host-runner-traits, embeddable-agent-engine.tool-host-outcome-verification, embeddable-agent-engine.cancellation-phase-ownership
Artifact-Type: validation-evidence

# V2 host runner evidence

## Test paths

- `crates/clankers-engine-host/src/lib.rs` unit tests:
  - `runner_completes_model_success_and_records_usage`
  - `retryable_model_failure_sleeps_before_retry_ready`
  - `retryable_model_failure_is_single_flight_and_retries_after_sleep`
  - `sequential_tool_requests_execute_in_engine_order_before_followup_model`
  - `cancellation_before_model_maps_to_cancel_turn`
  - `cancellation_races_ignore_late_model_tool_and_retry_results`
  - `streamed_model_events_fold_into_completion_and_usage_order`
  - `usage_only_and_empty_stop_streams_complete_successfully`
  - `malformed_stream_maps_to_non_retryable_model_failure`
  - `stream_malformed_matrix_maps_to_non_retryable_model_failures`
  - `provider_stream_error_preserves_status_and_retryability`
  - `event_sink_failures_become_diagnostics_without_reducer_failure`
  - `usage_observer_failure_records_diagnostic_without_terminalizing`
  - `tool_host_outcomes_map_to_correlated_engine_feedback`
  - `tool_missing_maps_to_engine_feedback`
  - `reducer_rejection_is_reported_without_local_terminalization`

## Commands

- `cargo test -p clankers-engine-host --lib`: PASS (26 tests).

## Result

Fake model/tool/sleep/event/cancel/usage adapters cover success, non-retryable failure, retry scheduling with sleeper-delayed `RetryReady`, single-flight model retry ordering, sequential tool scheduling, cancellation before model/next-tool and after late model/tool/retry results, terminal outcomes, non-streaming completion, streamed usage-only and empty-stop success mapping, malformed stream failures, provider-error status/retryability preservation, observer diagnostics, final report state, and tool-host outcome mapping.
