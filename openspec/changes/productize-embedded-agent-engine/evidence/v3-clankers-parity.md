Evidence-ID: v3-clankers-parity
Task-ID: V3
Artifact-Type: machine-check-log
Covers: embeddable-agent-engine.embedding-acceptance-bundle.clankers-parity
Created: 2026-04-25T23:50:19Z
Status: pass

# V3 Clankers host-runner parity evidence

## Engine-host behavior rail

Covers streaming, tool feedback, retry, cancellation, usage, and terminal outcome behavior in reusable host runner tests.

```text
    Finished `test` profile [optimized + debuginfo] target(s) in 0.20s
     Running unittests src/lib.rs (/home/brittonr/.cargo-target/debug/deps/clankers_engine_host-6bdf73bce7757e64)

running 26 tests
test stream::tests::folds_text_thinking_tool_usage_model_and_stop ... ok
test runtime::tests::tool_feedback_uses_default_error_when_text_missing ... ok
test runtime::tests::tool_feedback_converts_success_and_error_inputs ... ok
test stream::tests::preserves_provider_error_status_and_retryability ... ok
test stream::tests::rejects_delta_before_start ... ok
test stream::tests::rejects_duplicate_index ... ok
test stream::tests::rejects_late_delta_after_stop ... ok
test stream::tests::rejects_malformed_tool_json ... ok
test stream::tests::rejects_non_object_tool_json ... ok
test stream::tests::usage_only_and_empty_stop_normalize ... ok
test tests::cancellation_before_model_maps_to_cancel_turn ... ok
test tests::event_sink_failures_become_diagnostics_without_reducer_failure ... ok
test tests::malformed_stream_maps_to_non_retryable_model_failure ... ok
test tests::cancellation_races_ignore_late_model_tool_and_retry_results ... ok
test tests::reducer_rejection_is_reported_without_local_terminalization ... ok
test tests::provider_stream_error_preserves_status_and_retryability ... ok
test tests::retryable_model_failure_sleeps_before_retry_ready ... ok
test tests::runner_completes_model_success_and_records_usage ... ok
test tests::retryable_model_failure_is_single_flight_and_retries_after_sleep ... ok
test tests::streamed_model_events_fold_into_completion_and_usage_order ... ok
test tests::sequential_tool_requests_execute_in_engine_order_before_followup_model ... ok
test tests::stream_malformed_matrix_maps_to_non_retryable_model_failures ... ok
test tests::tool_missing_maps_to_engine_feedback ... ok
test tests::usage_observer_failure_records_diagnostic_without_terminalizing ... ok
test tests::usage_only_and_empty_stop_streams_complete_successfully ... ok
test tests::tool_host_outcomes_map_to_correlated_engine_feedback ... ok

test result: ok. 26 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

```

## Clankers agent turn parity rail

Covers default `clankers-agent` turn assembly, host-runner tool roundtrips/failures, capability denial, hooks, usage, retry/backoff, cancellation, and terminal failures.

```text
warning: field `schema` is never read
  --> crates/clankers-db/src/search_index.rs:39:5
   |
36 | pub struct SearchIndex {
   |            ----------- field in this struct
...
39 |     schema: Schema,
   |     ^^^^^^
   |
   = note: `#[warn(dead_code)]` (part of `#[warn(unused)]`) on by default

warning: `clankers-db` (lib) generated 1 warning
warning: function `request_model_effect` is never used
   --> crates/clankers-agent/src/turn/mod.rs:485:4
    |
485 | fn request_model_effect(outcome: &EngineOutcome) -> Result<EngineModelRequest> {
    |    ^^^^^^^^^^^^^^^^^^^^
    |
    = note: `#[warn(dead_code)]` (part of `#[warn(unused)]`) on by default

warning: function `schedule_retry_effect` is never used
   --> crates/clankers-agent/src/turn/mod.rs:511:4
    |
511 | fn schedule_retry_effect(outcome: &EngineOutcome) -> Result<Option<(EngineCorrelationId, std::time::Duration)>> {
    |    ^^^^^^^^^^^^^^^^^^^^^

warning: function `emit_engine_notice_effects` is never used
   --> crates/clankers-agent/src/turn/mod.rs:578:4
    |
578 | fn emit_engine_notice_effects(outcome: &EngineOutcome, event_tx: &broadcast::Sender<AgentEvent>) {
    |    ^^^^^^^^^^^^^^^^^^^^^^^^^^

warning: function `update_engine_model` is never used
   --> crates/clankers-agent/src/turn/mod.rs:603:4
    |
603 | fn update_engine_model(engine_state: &mut EngineState, active_model: &str) {
    |    ^^^^^^^^^^^^^^^^^^^

warning: function `tool_feedback_input` is never used
   --> crates/clankers-agent/src/turn/mod.rs:610:4
    |
610 | fn tool_feedback_input(message: &ToolResultMessage) -> EngineInput {
    |    ^^^^^^^^^^^^^^^^^^^

warning: function `cancel_active_engine_turn` is never used
   --> crates/clankers-agent/src/turn/mod.rs:726:4
    |
726 | fn cancel_active_engine_turn(
    |    ^^^^^^^^^^^^^^^^^^^^^^^^^

warning: `clankers-agent` (lib test) generated 6 warnings
    Finished `test` profile [optimized + debuginfo] target(s) in 0.29s
     Running unittests src/lib.rs (/home/brittonr/.cargo-target/debug/deps/clankers_agent-08e2a2390a29d715)

running 46 tests
test turn::tests::accepted_prompt_submission_reduces_engine ... ok
test turn::tests::decide_model_completion_accepts_execute_tool_effects ... ok
test turn::tests::decide_model_completion_accepts_turn_finish_effect ... ok
test turn::tests::decide_model_completion_rejects_ambiguous_effect_sets ... ok
test turn::tests::accumulator_collects_chunks_from_tool ... ok
test turn::tests::controller_filtered_tool_inventory_replaces_available_tools_without_turn_local_state ... ok
test turn::tests::direct_result_used_when_no_chunks ... ok
test turn::tests::engine_feedback_model_tool_retry_and_cancel_reduce_through_engine ... ok
test turn::tests::engine_retry_stop_policy_max_tokens_terminalizes_without_follow_up_work ... ok
test turn::tests::engine_retry_stop_policy_zero_budget_rejects_before_provider_io ... ok
test turn::tests::output_truncation_preserves_existing_clankers_limit_metadata ... ok
test turn::tests::engine_retry_stop_policy_budget_exhaustion_accepts_tool_feedback_without_follow_up_model ... ok
test turn::tests::run_turn_loop_applies_model_switch_and_emits_usage_updates ... ok
test turn::tests::run_turn_loop_dispatches_pre_tool_hooks_through_host_runner ... ok
test turn::tests::test_content_block_builder_mismatched_delta_ignored ... ok
test turn::tests::test_content_block_builder_signature_delta ... ok
test turn::tests::test_content_block_builder_text_delta ... ok
test turn::tests::run_turn_loop_executes_engine_requested_tool_roundtrip ... ok
test turn::tests::run_turn_loop_feeds_tool_failures_back_through_engine ... ok
test turn::tests::test_content_block_builder_tool_use_empty_json ... ok
test turn::tests::test_content_block_builder_tool_use_invalid_json_fallback ... ok
test turn::tests::test_content_block_builder_thinking_delta ... ok
test turn::tests::test_content_block_builder_tool_use_json_delta ... ok
test turn::tests::run_turn_loop_preserves_capability_gate_denials_through_host_runner ... ok
test turn::tests::test_parse_stop_reason_end_turn ... ok
test turn::tests::test_parse_stop_reason_max_tokens ... ok
test turn::tests::test_parse_stop_reason_stop ... ok
test turn::tests::test_parse_stop_reason_tool_use ... ok
test turn::tests::test_parse_stop_reason_unknown_defaults_to_stop ... ok
test turn::tests::test_tool_result_empty_content ... ok
test turn::tests::test_tool_result_image_conversion ... ok
test turn::tests::test_tool_result_mixed_content ... ok
test turn::tests::test_tool_result_text_conversion ... ok
test turn::tests::turn_request_includes_session_id_extra_param ... ok
test turn::tests::turn_request_reuses_session_id_after_resume ... ok
test turn::tests::turn_retry_non_retryable_error_skips_retry ... ok
test turn::tests::turn_request_reuses_session_id_across_later_turns ... ok
test turn::tests::user_tool_filter_blocks_unlisted_tools ... ok
test turn::tests::user_tool_filter_none_allows_all ... ok
test turn::tests::user_tool_filter_allows_listed_tools ... ok
test turn::tests::user_tool_filter_applies_latest_allowlist_per_call ... ok
test turn::tests::engine_retry_stop_policy_cancellation_during_retry_backoff_stops_retry_ready ... ok
test turn::tests::turn_retry_cancellation_during_backoff ... ok
test turn::tests::engine_retry_stop_policy_retryable_recovery_uses_engine_retry_effect ... ok
test turn::tests::turn_retry_recovers_on_second_attempt ... ok
test turn::tests::engine_retry_stop_policy_terminal_failures_preserve_original_error_and_messages ... ok

test result: ok. 46 passed; 0 failed; 0 ignored; 0 measured; 100 filtered out; finished in 5.00s

```

## Controller/FCIS runner-route rail

Proves agent turn runtime delegates runner policy to `run_engine_turn` / `HostAdapters` instead of duplicating policy.

```text
warning: field `schema` is never read
  --> crates/clankers-db/src/search_index.rs:39:5
   |
36 | pub struct SearchIndex {
   |            ----------- field in this struct
...
39 |     schema: Schema,
   |     ^^^^^^
   |
   = note: `#[warn(dead_code)]` (part of `#[warn(unused)]`) on by default

warning: `clankers-db` (lib) generated 1 warning
    Finished `test` profile [optimized + debuginfo] target(s) in 0.28s
     Running tests/fcis_shell_boundaries.rs (/home/brittonr/.cargo-target/debug/deps/fcis_shell_boundaries-7f08c44dd1beb864)

running 1 test
test agent_turn_delegates_runner_policy_to_host_runner ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 33 filtered out; finished in 0.03s

```
