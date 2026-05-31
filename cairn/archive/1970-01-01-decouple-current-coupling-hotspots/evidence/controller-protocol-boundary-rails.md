# Controller/protocol boundary rail evidence

Evidence-ID: controller-protocol-boundary-rails
Artifact-Type: command-output-summary
Task-ID: V4
Covers: coupling-hotspot-remediation.controller-protocol-boundary
Date: 2026-05-31
Status: PASS

## Commands

```text
./scripts/check-controller-protocol-boundary.rs
TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= cargo test -p clankers-controller --lib effect_interpretation
TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= cargo test -p clankers-controller --lib domain_event
TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= cargo test -p clankers-controller --lib transport_convert
TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= cargo test -p clankers-controller --lib runtime_adapter_fixture_covers_prompt_control_identity_and_semantic_projection
TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= cargo test -p clankers-controller --test fcis_shell_boundaries
```

## Relevant output

```text
ok: controller/protocol boundary rail passed

running 4 tests
test effect_interpretation::tests::prompt_request_interpretation_rejects_mismatched_projection ... ok
test effect_interpretation::tests::prompt_request_interpretation_accepts_matching_start_effect ... ok
test effect_interpretation::tests::thinking_interpretation_separates_apply_effect_from_logical_event ... ok
test effect_interpretation::tests::tool_filter_interpretation_keeps_application_and_ack_projection_distinct ... ok

test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 190 filtered out

running 3 tests
test domain_event::tests::ignores_agent_internal_context_events ... ok
test domain_event::tests::projects_agent_streaming_without_protocol_or_tui_types ... ok
test domain_event::tests::projects_tool_receipts_to_neutral_text_and_images ... ok

test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 191 filtered out

running 7 tests
test transport_convert::tests::client_handshake_uses_protocol_defaults ... ok
test transport_convert::tests::attach_responses_copy_fields ... ok
test transport_convert::tests::session_info_event_copies_session_socket_info ... ok
test transport_convert::tests::control_responses_project_state_and_socket_metadata ... ok
test transport_convert::tests::control_responses_cover_success_and_error_variants ... ok
test transport_convert::tests::session_summary_projects_handle_fields ... ok
test transport_convert::tests::daemon_status_counts_sessions_and_clients ... ok

test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 187 filtered out

running 1 test
test command::tests::runtime_adapter_fixture_covers_prompt_control_identity_and_semantic_projection ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 193 filtered out

running 35 tests
...
test controller_output_and_event_translation_stays_centralized ... ok
test controller_input_translation_stays_in_controller_translation_files ... ok
test controller_effect_interpretation_stays_centralized_repo_wide ... ok
test control_protocol_construction_stays_in_pure_conversion_files ... ok
test transport_protocol_construction_stays_in_pure_conversion_files ... ok

test result: ok. 35 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

## Coverage notes

The static rail checks that controller command handling routes prompt, thinking, disabled-tool, loop, and completion decisions through `clankers_core::CoreInput`/`CoreOutcome`; that pure `effect_interpretation.rs` turns core effects into typed controller work plans; that agent runtime events first project into neutral `SemanticEvent`/domain images in `domain_event.rs`; that `convert.rs` owns semantic/domain event projection to daemon and TUI protocol events; and that `transport_convert.rs` owns wire `ControlResponse`, `AttachResponse`, `SessionInfo`, `SessionSummary`, and daemon-status constructors.

Focused controller tests cover domain input/effect interpretation, semantic event projection, protocol projection, and the runtime adapter prompt seam. The FCIS boundary rail verifies input translation, event/output translation, effect interpretation, and transport/control protocol constructors stay centralized in the intended files.
