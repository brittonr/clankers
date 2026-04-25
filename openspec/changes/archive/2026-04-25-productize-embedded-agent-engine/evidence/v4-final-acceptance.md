Evidence-ID: v4-final-acceptance
Task-ID: V4
Artifact-Type: machine-check-log
Covers: embeddable-agent-engine.embedding-acceptance-bundle, embeddable-agent-engine.embedding-acceptance-bundle.docs-examples, embeddable-agent-engine.embedding-api-stability-rails.dependency-boundary-clean
Created: 2026-04-25T23:51:02Z
Status: pass

# V4 Final acceptance evidence

```text

==> /home/brittonr/git/clankers/.pi/worktrees/session-1777159833772-0vld/scripts/check-embedded-sdk-api.rs
ok: embedded SDK API inventory covers 110 public items (111 rows)

==> /home/brittonr/git/clankers/.pi/worktrees/session-1777159833772-0vld/scripts/check-embedded-sdk-deps.rs
ok: embedded SDK example dependency graph has 56 packages and excludes forbidden runtime crates

==> /home/brittonr/git/clankers/.pi/worktrees/session-1777159833772-0vld/scripts/check-llm-contract-boundary.sh
ok: clankers-engine normal-edge tree excludes forbidden crates
ok: clankers-engine-host normal-edge tree excludes forbidden crates
ok: clankers-tool-host normal-edge tree excludes forbidden crates
ok: clanker-message normal-edge tree excludes forbidden crates
ok: clankers-engine-host direct normal deps exclude forbidden crates
ok: clankers-tool-host direct normal deps exclude forbidden crates
ok: crates/clankers-engine/src excludes forbidden source tokens
ok: crates/clankers-engine-host/src excludes forbidden source tokens
ok: crates/clankers-tool-host/src excludes forbidden source tokens

==> cargo run --locked --manifest-path examples/embedded-agent-sdk/Cargo.toml
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.05s
     Running `/home/brittonr/.cargo-target/debug/embedded-agent-sdk-example`
embedded-agent-sdk example passed

==> cargo test -p clankers-engine-host --lib
    Finished `test` profile [optimized + debuginfo] target(s) in 0.16s
     Running unittests src/lib.rs (/home/brittonr/.cargo-target/debug/deps/clankers_engine_host-6bdf73bce7757e64)

running 26 tests
test runtime::tests::tool_feedback_converts_success_and_error_inputs ... ok
test runtime::tests::tool_feedback_uses_default_error_when_text_missing ... ok
test stream::tests::rejects_delta_before_start ... ok
test stream::tests::folds_text_thinking_tool_usage_model_and_stop ... ok
test stream::tests::preserves_provider_error_status_and_retryability ... ok
test stream::tests::rejects_duplicate_index ... ok
test stream::tests::rejects_late_delta_after_stop ... ok
test stream::tests::rejects_malformed_tool_json ... ok
test stream::tests::rejects_non_object_tool_json ... ok
test stream::tests::usage_only_and_empty_stop_normalize ... ok
test tests::cancellation_before_model_maps_to_cancel_turn ... ok
test tests::event_sink_failures_become_diagnostics_without_reducer_failure ... ok
test tests::malformed_stream_maps_to_non_retryable_model_failure ... ok
test tests::cancellation_races_ignore_late_model_tool_and_retry_results ... ok
test tests::provider_stream_error_preserves_status_and_retryability ... ok
test tests::reducer_rejection_is_reported_without_local_terminalization ... ok
test tests::retryable_model_failure_is_single_flight_and_retries_after_sleep ... ok
test tests::runner_completes_model_success_and_records_usage ... ok
test tests::retryable_model_failure_sleeps_before_retry_ready ... ok
test tests::sequential_tool_requests_execute_in_engine_order_before_followup_model ... ok
test tests::streamed_model_events_fold_into_completion_and_usage_order ... ok
test tests::stream_malformed_matrix_maps_to_non_retryable_model_failures ... ok
test tests::tool_missing_maps_to_engine_feedback ... ok
test tests::tool_host_outcomes_map_to_correlated_engine_feedback ... ok
test tests::usage_observer_failure_records_diagnostic_without_terminalizing ... ok
test tests::usage_only_and_empty_stop_streams_complete_successfully ... ok

test result: ok. 26 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s


==> cargo test -p clankers-agent --lib turn::tests::
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
    Finished `test` profile [optimized + debuginfo] target(s) in 0.27s
     Running unittests src/lib.rs (/home/brittonr/.cargo-target/debug/deps/clankers_agent-08e2a2390a29d715)

running 46 tests
test turn::tests::accepted_prompt_submission_reduces_engine ... ok
test turn::tests::decide_model_completion_accepts_turn_finish_effect ... ok
test turn::tests::decide_model_completion_accepts_execute_tool_effects ... ok
test turn::tests::decide_model_completion_rejects_ambiguous_effect_sets ... ok
test turn::tests::controller_filtered_tool_inventory_replaces_available_tools_without_turn_local_state ... ok
test turn::tests::accumulator_collects_chunks_from_tool ... ok
test turn::tests::direct_result_used_when_no_chunks ... ok
test turn::tests::engine_feedback_model_tool_retry_and_cancel_reduce_through_engine ... ok
test turn::tests::engine_retry_stop_policy_max_tokens_terminalizes_without_follow_up_work ... ok
test turn::tests::engine_retry_stop_policy_zero_budget_rejects_before_provider_io ... ok
test turn::tests::engine_retry_stop_policy_budget_exhaustion_accepts_tool_feedback_without_follow_up_model ... ok
test turn::tests::output_truncation_preserves_existing_clankers_limit_metadata ... ok
test turn::tests::run_turn_loop_dispatches_pre_tool_hooks_through_host_runner ... ok
test turn::tests::test_content_block_builder_text_delta ... ok
test turn::tests::run_turn_loop_executes_engine_requested_tool_roundtrip ... ok
test turn::tests::test_content_block_builder_tool_use_empty_json ... ok
test turn::tests::run_turn_loop_applies_model_switch_and_emits_usage_updates ... ok
test turn::tests::run_turn_loop_preserves_capability_gate_denials_through_host_runner ... ok
test turn::tests::test_content_block_builder_signature_delta ... ok
test turn::tests::test_content_block_builder_tool_use_json_delta ... ok
test turn::tests::test_content_block_builder_mismatched_delta_ignored ... ok
test turn::tests::run_turn_loop_feeds_tool_failures_back_through_engine ... ok
test turn::tests::test_parse_stop_reason_end_turn ... ok
test turn::tests::test_content_block_builder_tool_use_invalid_json_fallback ... ok
test turn::tests::test_content_block_builder_thinking_delta ... ok
test turn::tests::test_parse_stop_reason_max_tokens ... ok
test turn::tests::test_parse_stop_reason_stop ... ok
test turn::tests::test_parse_stop_reason_tool_use ... ok
test turn::tests::test_parse_stop_reason_unknown_defaults_to_stop ... ok
test turn::tests::test_tool_result_empty_content ... ok
test turn::tests::test_tool_result_image_conversion ... ok
test turn::tests::test_tool_result_mixed_content ... ok
test turn::tests::test_tool_result_text_conversion ... ok
test turn::tests::turn_request_reuses_session_id_across_later_turns ... ok
test turn::tests::turn_request_includes_session_id_extra_param ... ok
test turn::tests::turn_request_reuses_session_id_after_resume ... ok
test turn::tests::turn_retry_non_retryable_error_skips_retry ... ok
test turn::tests::user_tool_filter_blocks_unlisted_tools ... ok
test turn::tests::user_tool_filter_applies_latest_allowlist_per_call ... ok
test turn::tests::user_tool_filter_none_allows_all ... ok
test turn::tests::user_tool_filter_allows_listed_tools ... ok
test turn::tests::engine_retry_stop_policy_cancellation_during_retry_backoff_stops_retry_ready ... ok
test turn::tests::turn_retry_cancellation_during_backoff ... ok
test turn::tests::engine_retry_stop_policy_retryable_recovery_uses_engine_retry_effect ... ok
test turn::tests::turn_retry_recovers_on_second_attempt ... ok
test turn::tests::engine_retry_stop_policy_terminal_failures_preserve_original_error_and_messages ... ok

test result: ok. 46 passed; 0 failed; 0 ignored; 0 measured; 100 filtered out; finished in 5.00s


==> cargo test -p clankers-controller --test fcis_shell_boundaries
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
    Finished `test` profile [optimized + debuginfo] target(s) in 0.25s
     Running tests/fcis_shell_boundaries.rs (/home/brittonr/.cargo-target/debug/deps/fcis_shell_boundaries-7f08c44dd1beb864)

running 34 tests
test cfg_attribute_detection_handles_literal_and_composite_test_only_forms ... ok
test collect_non_test_constructor_paths_skip_test_only_cfg_expressions ... ok
test collect_non_test_paths_skip_test_only_field_values ... ok
test collect_non_test_paths_include_runtime_use_tree_paths_and_skip_test_only_uses ... ok
test collect_non_test_paths_skip_test_only_match_arms ... ok
test collect_non_test_paths_skip_test_only_patterns ... ok
test collect_non_test_constructor_paths_skips_test_only_enum_variants ... ok
test collect_non_test_paths_skip_test_only_locals_and_cfg_fields ... ok
test collect_non_test_paths_skip_test_only_variants ... ok
test collect_non_test_paths_skip_test_only_stmt_macros ... ok
test collect_non_test_paths_skips_test_only_modules_without_hiding_later_runtime_items ... ok
test collect_non_test_struct_expr_paths_skips_test_only_constructors ... ok
test host_builtin_tool_path_text_inventory_allows_bare_tool_words ... ok
test tool_host_rejects_engine_reducer_internal_source_leakage ... ok
test core_engine_composition_stays_pure_adapter_source ... ok
test agent_turn_runtime_reuses_engine_request_planning_contract ... ok
test embedded_event_loop_runner_stays_adapter_only ... ok
test host_crates_reject_shell_runtime_source_leakage ... ok
test engine_host_feedback_constructors_stay_in_runtime_module ... ok
test engine_host_rejects_reducer_policy_source_leakage ... ok
test llm_contract_sources_reject_shell_runtime_dependencies ... ok
test core_and_engine_reducer_policy_inventories_stay_closed ... ok
test clankers_engine_surface_stays_shell_native ... ok
test agent_turn_runtime_reuses_engine_model_completion_contract ... ok
test agent_turn_runtime_defers_retry_and_budget_policy_to_engine ... ok
test agent_runtime_files_stay_shell_native ... ok
test controller_input_translation_stays_in_controller_translation_files ... ok
test controller_effect_interpretation_stays_centralized_repo_wide ... ok
test control_protocol_construction_stays_in_pure_conversion_files ... ok
test controller_output_and_event_translation_stays_centralized ... ok
test agent_turn_delegates_runner_policy_to_host_runner ... ok
test transport_protocol_construction_stays_in_pure_conversion_files ... ok
test adapter_constructor_and_feedback_inventories_stay_on_allowed_seams ... ok
test engine_terminal_policy_symbols_stay_inside_engine_source ... ok

test result: ok. 34 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.33s


embedded-agent-sdk acceptance passed
```
