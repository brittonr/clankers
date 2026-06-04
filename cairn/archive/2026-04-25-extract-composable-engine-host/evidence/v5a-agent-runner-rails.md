Task-ID: V5a
Covers: embeddable-agent-engine.host-extraction-rails, embeddable-agent-engine.no-duplicated-runner-policy, embeddable-agent-engine.host-feedback-construction-seam, embeddable-agent-engine.cross-reducer-source-rail
Artifact-Type: validation-evidence

# V5a agent runner rail evidence

## Rail paths

- `crates/clankers-controller/tests/fcis_shell_boundaries.rs::agent_turn_delegates_runner_policy_to_host_runner`
- `crates/clankers-controller/tests/fcis_shell_boundaries.rs::agent_turn_runtime_reuses_engine_model_completion_contract`
- `crates/clankers-controller/tests/fcis_shell_boundaries.rs::agent_turn_runtime_defers_retry_and_budget_policy_to_engine`
- `crates/clankers-controller/tests/fcis_shell_boundaries.rs::agent_turn_runtime_reuses_engine_request_planning_contract`
- `crates/clankers-controller/tests/fcis_shell_boundaries.rs::adapter_constructor_and_feedback_inventories_stay_on_allowed_seams`
- `crates/clankers-controller/tests/fcis_shell_boundaries.rs::engine_terminal_policy_symbols_stay_inside_engine_source`

## Coverage

The rails assert:

- non-test `clankers-agent::turn` requires `run_engine_turn`, `HostAdapters`, `AgentModelHost`, and `AgentToolHost`;
- duplicated runner-policy segments (`ModelCompleted`, `ModelFailed`, `ToolCompleted`, `ToolFailed`, `RetryReady`, `CancelTurn`, `ScheduleRetry`, pending model/tool fields, retry-budget fields, terminalization helper names, and direct `EngineEvent::TurnFinished`) stay out of non-test agent turn paths;
- non-test loops over `effects` are rejected by an AST rail that skips `#[cfg(test)]` items;
- provider-request conversion remains in the allowed adapter file;
- direct feedback constructors stay in `clankers-engine-host::runtime` / runner seams or tests;
- shell code observes terminal events through helper methods instead of direct stop-policy matching.

## Commands

- `cargo test -p clankers-controller --test fcis_shell_boundaries`: PASS (34 tests).
- `./scripts/check-llm-contract-boundary.sh`: PASS.

## Result

Agent turn source now has persistent rails against duplicated runner policy and feedback-constructor drift. Failures report the matched file plus symbol/path/finding set.
