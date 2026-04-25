Artifact-Type: verification-log
Evidence-ID: engine-retry-stop-policy.validation-suite
Task-ID: 4.10
Covers: embeddable-agent-engine.retry-stop-policy-owned, embeddable-agent-engine.adapter-parity-rails, turn-level-retry.engine-authoritative, turn-level-retry.no-duplicate-messages, turn-level-retry.cancellation-during-backoff
Creator: pi
Created: 2026-04-25T01:38:06Z
Status: PASS
Command: RUSTC_WRAPPER= cargo test -p clankers-engine && RUSTC_WRAPPER= cargo test -p clankers-agent --lib engine_retry_stop_policy -- --nocapture && RUSTC_WRAPPER= cargo test -p clankers-controller --test fcis_shell_boundaries

Output:

```text
+ RUSTC_WRAPPER= cargo test -p clankers-engine
    Finished `test` profile [optimized + debuginfo] target(s) in 0.24s
     Running unittests src/lib.rs (/home/brittonr/.cargo-target/debug/deps/clankers_engine-1f4c12f6f182207f)

running 28 tests
test tests::cancel_turn_terminalizes_pending_tool_work ... ok
test tests::failed_retry_attempts_do_not_mutate_canonical_messages ... ok
test tests::cancel_turn_rejects_idle_phase ... ok
test tests::cancel_turn_while_retry_scheduled_clears_work_and_rejects_late_feedback ... ok
test tests::cancel_turn_terminalizes_pending_model_work ... ok
test tests::model_completion_rejects_invalid_phase ... ok
test tests::max_tokens_terminalizes_after_accepting_assistant_content ... ok
test tests::model_completion_rejects_mismatched_request_id ... ok
test tests::model_completion_finishes_turn_for_terminal_stop_reason ... ok
test tests::model_completion_rejects_tool_use_without_tool_call ... ok
test tests::model_completion_schedules_tool_effects_for_tool_use_stop ... ok
test tests::model_failed_terminalizes_pending_request ... ok
test tests::model_continuation_budget_counts_requests_and_terminalizes_after_accepted_tool_feedback ... ok
test tests::non_retryable_model_failure_terminalizes_without_retry_or_message_mutation ... ok
test tests::retry_exhaustion_terminalizes_with_latest_failure_without_message_mutation ... ok
test tests::retry_feedback_rejects_mismatched_request_ids_without_state_mutation ... ok
test tests::retry_feedback_rejects_wrong_phase_duplicate_and_post_terminal_feedback ... ok
test tests::retryable_model_failure_schedules_retry_and_retry_ready_reemits_same_request ... ok
test tests::submit_user_prompt_builds_request_effect ... ok
test tests::submit_user_prompt_preserves_empty_session_id ... ok
test tests::submit_user_prompt_rejects_busy_state ... ok
test tests::submit_user_prompt_strips_non_conversation_metadata_messages ... ok
test tests::successful_model_feedback_resets_retry_counter_for_follow_up_request ... ok
test tests::tool_feedback_rejects_duplicate_call_id ... ok
test tests::tool_feedback_rejects_wrong_phase ... ok
test tests::tool_feedback_rejects_unknown_call_id ... ok
test tests::zero_model_request_budget_rejects_prompt_before_provider_work ... ok
test tests::tool_feedback_waits_for_all_pending_results_before_continuing ... ok

test result: ok. 28 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

   Doc-tests clankers_engine

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

+ RUSTC_WRAPPER= cargo test -p clankers-agent --lib engine_retry_stop_policy -- --nocapture
    Finished `test` profile [optimized + debuginfo] target(s) in 0.26s
     Running unittests src/lib.rs (/home/brittonr/.cargo-target/debug/deps/clankers_agent-4c603ec3256e9e26)

running 6 tests
test turn::tests::engine_retry_stop_policy_zero_budget_rejects_before_provider_io ... ok
test turn::tests::engine_retry_stop_policy_max_tokens_terminalizes_without_follow_up_work ... ok
test turn::tests::engine_retry_stop_policy_budget_exhaustion_accepts_tool_feedback_without_follow_up_model ... ok
test turn::tests::engine_retry_stop_policy_cancellation_during_retry_backoff_stops_retry_ready ... ok
test turn::tests::engine_retry_stop_policy_retryable_recovery_uses_engine_retry_effect ... ok
test turn::tests::engine_retry_stop_policy_terminal_failures_preserve_original_error_and_messages ... ok

test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 126 filtered out; finished in 5.00s

+ RUSTC_WRAPPER= cargo test -p clankers-controller --test fcis_shell_boundaries
    Finished `test` profile [optimized + debuginfo] target(s) in 0.24s
     Running tests/fcis_shell_boundaries.rs (/home/brittonr/.cargo-target/debug/deps/fcis_shell_boundaries-f6b69196b9d0cdde)

running 23 tests
test cfg_attribute_detection_handles_literal_and_composite_test_only_forms ... ok
test collect_non_test_paths_include_runtime_use_tree_paths_and_skip_test_only_uses ... ok
test collect_non_test_constructor_paths_skip_test_only_cfg_expressions ... ok
test collect_non_test_paths_skip_test_only_field_values ... ok
test collect_non_test_constructor_paths_skips_test_only_enum_variants ... ok
test collect_non_test_paths_skip_test_only_locals_and_cfg_fields ... ok
test collect_non_test_paths_skip_test_only_patterns ... ok
test collect_non_test_paths_skip_test_only_match_arms ... ok
test collect_non_test_paths_skip_test_only_stmt_macros ... ok
test collect_non_test_paths_skip_test_only_variants ... ok
test collect_non_test_paths_skips_test_only_modules_without_hiding_later_runtime_items ... ok
test collect_non_test_struct_expr_paths_skips_test_only_constructors ... ok
test agent_turn_runtime_reuses_engine_request_planning_contract ... ok
test embedded_event_loop_runner_stays_adapter_only ... ok
test agent_turn_runtime_reuses_engine_model_completion_contract ... ok
test clankers_engine_surface_stays_shell_native ... ok
test agent_turn_runtime_defers_retry_and_budget_policy_to_engine ... ok
test agent_runtime_files_stay_shell_native ... ok
test controller_input_translation_stays_in_controller_translation_files ... ok
test controller_effect_interpretation_stays_centralized_repo_wide ... ok
test controller_output_and_event_translation_stays_centralized ... ok
test control_protocol_construction_stays_in_pure_conversion_files ... ok
test transport_protocol_construction_stays_in_pure_conversion_files ... ok

test result: ok. 23 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.04s

exit_statuses: engine=0 agent=0 fcis=0

```
