Evidence-ID: focused-turn-loop-tests
Artifact-Type: test-report
Task-ID: V1
Covers: r[steel-core-agent-loop.executor-selection.default], r[steel-core-agent-loop.executor-selection.comparison], r[steel-core-agent-loop.receipts.executor]
Created: 2026-05-30
Status: complete

# Focused Turn Loop Tests

## Command

```text
TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= cargo test -p clankers-agent run_turn_loop_ --lib
```

## Result

```text
running 7 tests
test turn::tests::run_turn_loop_applies_model_switch_and_emits_usage_updates ... ok
test turn::tests::run_turn_loop_dispatches_pre_tool_hooks_through_host_runner ... ok
test turn::tests::run_turn_loop_executes_engine_requested_tool_roundtrip ... ok
test turn::tests::run_turn_loop_preserves_capability_gate_denials_through_host_runner ... ok
test turn::tests::run_turn_loop_emits_steel_plan_turn_receipt_when_configured ... ok
test turn::tests::run_turn_loop_feeds_tool_failures_back_through_engine ... ok
test turn::tests::run_turn_loop_uses_steel_selected_executor_when_default_planner_authorizes ... ok

test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 178 filtered out
STATUS 0
```

The default-mode test asserts both `executor=SteelScheme` receipt text and that the Steel-selected execution adapter was called. The configured comparison test asserts `executor=RustNative` receipt text and prompt redaction.
