Task-ID: V2b,V2c
Covers: embeddable-agent-engine.host-runner-traits, embeddable-agent-engine.tool-host-outcome-verification, embeddable-agent-engine.cancellation-phase-ownership
Artifact-Type: validation-evidence

# V2 host runner diagnostics and tool outcome evidence

## Test paths

- `crates/clankers-engine-host/src/lib.rs` unit tests:
  - `event_sink_failures_become_diagnostics_without_reducer_failure`
  - `usage_observer_failure_records_diagnostic_without_terminalizing`
  - `streamed_model_events_fold_into_completion_and_usage_order`
  - `tool_host_outcomes_map_to_correlated_engine_feedback`
  - `tool_missing_maps_to_engine_feedback`
  - `provider_stream_error_preserves_status_and_retryability`
  - `stream_malformed_matrix_maps_to_non_retryable_model_failures`

## Commands

- `cargo test -p clankers-engine-host --lib`: PASS (22 tests).

## Result

Host observer failures are diagnostics only; usage observations are recorded in stream/final order; tool-host outcomes map through engine feedback/cancellation without reducer rejection; malformed host streams produce non-retryable reducer-visible model failures.
