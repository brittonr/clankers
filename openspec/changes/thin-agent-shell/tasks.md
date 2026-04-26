## 1. TurnTranscript and TurnTranscriptWriter

- [x] I1 Introduce `TurnTranscript` and `TurnTranscriptWriter` types in `crates/clankers-agent/src/turn/` that own message history, assistant snapshots, turn-index bookkeeping, batch tool-result accumulation, cumulative usage, and active-model tracking. The writer exposes `append_assistant`, `append_tool_result`, `mark_turn_start`, `finish_turn`, `active_model`, `set_active_model`, and `cumulative_usage`. [covers=thin-agent-shell.transcript-isolated-from-adapters,thin-agent-shell.transcript-writer-replaces-arc-mutex,thin-agent-shell.transcript-independent-of-adapter-lifetime]

## 2. Thin adapter structs

- [x] I2 Rewrite `AgentModelHost` to hold provider ref, event_tx ref, cancel token, model_switch_slot ref, and `TurnTranscriptWriter`. Its `execute_model` impl delegates to `check_model_switch` -> `completion_request_from_engine_request` -> `stream_model_request` -> `build_assistant_message` -> transcript writer. No inline streaming, no `Arc<Mutex<TurnHostState>>`. [covers=thin-agent-shell.adapters-are-pure-delegation,thin-agent-shell.model-adapter-delegates-to-modules,embeddable-agent-engine.host-adapters-pure-delegation]
- [x] I3 Rewrite `AgentToolHost` to hold controller_tools ref, event_tx ref, cancel token, hook pipeline, session ID, DB, capability gate, user tool filter, output truncation config, and `TurnTranscriptWriter`. Its `execute_tool` impl delegates to `execute_tools_parallel` -> `apply_output_truncation` -> `tool_result_message_to_host_outcome` -> transcript writer. No inline capability checks or hook calls. [covers=thin-agent-shell.adapters-are-pure-delegation,thin-agent-shell.tool-adapter-delegates-to-modules,embeddable-agent-engine.host-adapters-pure-delegation]
- [x] I4 Rewrite `AgentEngineEventSink` to hold event_tx ref and `TurnTranscriptWriter`. Turn-end detection delegates to the transcript writer's `finish_turn`. [covers=thin-agent-shell.adapters-are-pure-delegation]
- [x] I5 Rewrite `AgentUsageObserver` to hold cost_tracker ref, event_tx ref, and `TurnTranscriptWriter`. Usage accumulation reads/writes through the transcript writer. [covers=thin-agent-shell.adapters-are-pure-delegation]
- [x] I6 Remove `TurnHostState`, `SharedTurnHostState`, and all `Arc<Mutex<TurnHostState>>` usage from adapter structs. [covers=thin-agent-shell.transcript-writer-replaces-arc-mutex]

## 3. Structured run_turn_loop parameters

- [x] I7 Move `TurnConfig` to a standalone struct (already exists, keep it). Introduce `TurnLoopContext` struct carrying provider, controller_tools, event_tx, cancel, cost_tracker, model_switch_slot, hook_pipeline, session_id, db, capability_gate, user_tool_filter references. Collapse `run_turn_loop` signature to `(config: &TurnConfig, ctx: TurnLoopContext<'_>, messages: &mut Vec<AgentMessage>) -> Result<()>`. [covers=thin-agent-shell.structured-turn-loop-params,thin-agent-shell.config-context-replace-positional]
- [x] I8 Update the single call site in `crates/clankers-agent/src/lib.rs` (or wherever `run_turn_loop` is called) to construct `TurnLoopContext` and pass it. [covers=thin-agent-shell.config-context-replace-positional]

## 4. Boundary rails

- [ ] I9 Add a boundary test in `crates/clankers-controller/tests/fcis_shell_boundaries.rs` (or `crates/clankers-agent/tests/`) that parses `impl ModelHost for` and `impl ToolExecutor for` blocks in `crates/clankers-agent/src/turn/mod.rs` and asserts they do not reference `CompletionRequest`, `stream_model_request`, `execute_tools_parallel`, `StreamEvent`, `Arc<Mutex<TurnHostState>>`, or `SharedTurnHostState`. [covers=thin-agent-shell.boundary-rails-enforce-adapter-purity,thin-agent-shell.fcis-rejects-provider-streaming-in-adapters,thin-agent-shell.fcis-rejects-inline-tool-execution-in-adapters]

## 5. Test migration and verification

- [x] V1 Migrate existing `run_turn_loop` tests to the new 3-argument signature. Verify all 5+ existing tests pass with unchanged behavioral coverage. [covers=thin-agent-shell.config-context-replace-positional]
- [ ] V2 Run the boundary rail from I9 and capture passing output as evidence that adapter structs contain no provider streaming, request construction, inline tool execution, or shared mutable state. [covers=thin-agent-shell.fcis-rejects-provider-streaming-in-adapters,thin-agent-shell.fcis-rejects-inline-tool-execution-in-adapters] [evidence=openspec/changes/thin-agent-shell/evidence/v2-boundary-rails.md]
- [ ] V3 Run `scripts/check-embedded-agent-sdk.sh` and verify the existing embedded SDK acceptance bundle still passes after the agent shell restructuring. [covers=embeddable-agent-engine.host-adapters-pure-delegation] [evidence=openspec/changes/thin-agent-shell/evidence/v3-embedded-sdk-parity.md]
