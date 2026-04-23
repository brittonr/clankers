Artifact-Type: verification-note
Evidence-ID: validation-suite
Task-ID: 3.3
Covers: engine-turn-migration validation commands

+ RUSTC_WRAPPER=
+ cargo test -p clankers-engine --lib
   Compiling ratcore v0.1.0 (/home/brittonr/git/ratcore)
   Compiling rat-leaderkey v0.1.0 (/home/brittonr/git/subwayrat/crates/rat-leaderkey)
   Compiling clankers-tui-types v0.1.0 (/home/brittonr/git/clankers/crates/clankers-tui-types)
   Compiling clankers-provider v0.1.0 (/home/brittonr/git/clankers/crates/clankers-provider)
   Compiling clankers-engine v0.1.0 (/home/brittonr/git/clankers/crates/clankers-engine)
    Finished `test` profile [optimized + debuginfo] target(s) in 3.77s
     Running unittests src/lib.rs (/home/brittonr/.cargo-target/debug/deps/clankers_engine-b7367d02a4107424)

running 17 tests
test tests::cancel_turn_rejects_idle_phase ... ok
test tests::cancel_turn_terminalizes_pending_model_work ... ok
test tests::cancel_turn_terminalizes_pending_tool_work ... ok
test tests::model_completion_finishes_turn_for_terminal_stop_reason ... ok
test tests::model_completion_rejects_invalid_phase ... ok
test tests::model_completion_rejects_mismatched_request_id ... ok
test tests::model_completion_rejects_tool_use_without_tool_call ... ok
test tests::model_completion_schedules_tool_effects_for_tool_use_stop ... ok
test tests::model_failed_terminalizes_pending_request ... ok
test tests::submit_user_prompt_builds_request_effect ... ok
test tests::submit_user_prompt_preserves_empty_session_id ... ok
test tests::submit_user_prompt_strips_non_conversation_metadata_messages ... ok
test tests::submit_user_prompt_rejects_busy_state ... ok
test tests::tool_feedback_rejects_duplicate_call_id ... ok
test tests::tool_feedback_rejects_wrong_phase ... ok
test tests::tool_feedback_rejects_unknown_call_id ... ok
test tests::tool_feedback_waits_for_all_pending_results_before_continuing ... ok

test result: ok. 17 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

+ RUSTC_WRAPPER=
+ cargo test -p clankers-agent run_turn_loop_executes_engine_requested_tool_roundtrip --lib
   Compiling rat-leaderkey v0.1.0 (/home/brittonr/git/subwayrat/crates/rat-leaderkey)
   Compiling clankers-tui-types v0.1.0 (/home/brittonr/git/clankers/crates/clankers-tui-types)
   Compiling clankers-model-selection v0.1.0 (/home/brittonr/git/clankers/crates/clankers-model-selection)
   Compiling clankers-tui v0.1.0 (/home/brittonr/git/clankers/crates/clankers-tui)
   Compiling clankers-provider v0.1.0 (/home/brittonr/git/clankers/crates/clankers-provider)
   Compiling clankers-procmon v0.1.0 (/home/brittonr/git/clankers/crates/clankers-procmon)
   Compiling clankers-util v0.1.0 (/home/brittonr/git/clankers/crates/clankers-util)
   Compiling clankers-config v0.1.0 (/home/brittonr/git/clankers/crates/clankers-config)
   Compiling clankers-engine v0.1.0 (/home/brittonr/git/clankers/crates/clankers-engine)
   Compiling clankers-agent v0.1.0 (/home/brittonr/git/clankers/crates/clankers-agent)
    Finished `test` profile [optimized + debuginfo] target(s) in 6.51s
     Running unittests src/lib.rs (/home/brittonr/.cargo-target/debug/deps/clankers_agent-f333543c2e4731a8)

running 1 test
test turn::tests::run_turn_loop_executes_engine_requested_tool_roundtrip ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 125 filtered out; finished in 0.00s

+ RUSTC_WRAPPER=
+ cargo test -p clankers-controller --test fcis_shell_boundaries
   Compiling rat-leaderkey v0.1.0 (/home/brittonr/git/subwayrat/crates/rat-leaderkey)
   Compiling clankers-tui-types v0.1.0 (/home/brittonr/git/clankers/crates/clankers-tui-types)
   Compiling clankers-tui v0.1.0 (/home/brittonr/git/clankers/crates/clankers-tui)
   Compiling clankers-provider v0.1.0 (/home/brittonr/git/clankers/crates/clankers-provider)
   Compiling clankers-model-selection v0.1.0 (/home/brittonr/git/clankers/crates/clankers-model-selection)
   Compiling clankers-util v0.1.0 (/home/brittonr/git/clankers/crates/clankers-util)
   Compiling clankers-procmon v0.1.0 (/home/brittonr/git/clankers/crates/clankers-procmon)
   Compiling clankers-config v0.1.0 (/home/brittonr/git/clankers/crates/clankers-config)
   Compiling clankers-engine v0.1.0 (/home/brittonr/git/clankers/crates/clankers-engine)
   Compiling clankers-agent v0.1.0 (/home/brittonr/git/clankers/crates/clankers-agent)
   Compiling clankers-controller v0.1.0 (/home/brittonr/git/clankers/crates/clankers-controller)
    Finished `test` profile [optimized + debuginfo] target(s) in 4.66s
     Running tests/fcis_shell_boundaries.rs (/home/brittonr/.cargo-target/debug/deps/fcis_shell_boundaries-afd104007046043d)

running 22 tests
test cfg_attribute_detection_handles_literal_and_composite_test_only_forms ... ok
test collect_non_test_paths_include_runtime_use_tree_paths_and_skip_test_only_uses ... ok
test collect_non_test_constructor_paths_skip_test_only_cfg_expressions ... ok
test collect_non_test_paths_skip_test_only_field_values ... ok
test collect_non_test_constructor_paths_skips_test_only_enum_variants ... ok
test collect_non_test_paths_skip_test_only_stmt_macros ... ok
test collect_non_test_paths_skip_test_only_patterns ... ok
test collect_non_test_paths_skip_test_only_match_arms ... ok
test collect_non_test_paths_skip_test_only_variants ... ok
test collect_non_test_paths_skips_test_only_modules_without_hiding_later_runtime_items ... ok
test collect_non_test_paths_skip_test_only_locals_and_cfg_fields ... ok
test collect_non_test_struct_expr_paths_skips_test_only_constructors ... ok
test agent_turn_runtime_reuses_engine_request_planning_contract ... ok
test embedded_event_loop_runner_stays_adapter_only ... ok
test clankers_engine_surface_stays_shell_native ... ok
test agent_turn_runtime_reuses_engine_model_completion_contract ... ok
test agent_runtime_files_stay_shell_native ... ok
test controller_input_translation_stays_in_controller_translation_files ... ok
test controller_effect_interpretation_stays_centralized_repo_wide ... ok
test controller_output_and_event_translation_stays_centralized ... ok
test control_protocol_construction_stays_in_pure_conversion_files ... ok
test transport_protocol_construction_stays_in_pure_conversion_files ... ok

test result: ok. 22 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.04s

