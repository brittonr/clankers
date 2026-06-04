Task-ID: I5a,I5b
Covers: embeddable-agent-engine.agent-default-assembly, embeddable-agent-engine.adapter-rail, embeddable-agent-engine.host-adapter-parity
Artifact-Type: implementation-evidence

# I5 agent host-runner migration evidence

## Implementation

- `crates/clankers-agent/src/turn/mod.rs::run_turn_loop` now builds an accepted `EnginePromptSubmission`, constructs `HostAdapters`, and invokes `clankers_engine_host::run_engine_turn(...)` for the shared agent prompt path.
- New agent-side host adapters keep shell concerns outside host crates:
  - `AgentModelHost` preserves provider request conversion, streaming/non-streaming model execution, model switching, session metadata, and usage extraction.
  - `AgentToolHost` preserves event emission, sequential tool execution through the `ToolExecutor` seam, and tool-result conversion.
  - `AgentRetrySleeper`, `AgentEngineEventSink`, `AgentCancellationSource`, and `AgentUsageObserver` preserve retry sleep, event ordering, cancellation checks, and usage updates.
- Standalone interactive, daemon session, and attach prompt paths continue to call the shared `Agent`/turn loop seam, so the entrypoints inherit the same host-runner path without moving daemon/TUI/hook code into host crates.
- `clankers-engine::EngineEvent::turn_finished_stop_reason()` gives shell code a helper-based terminal observation seam instead of direct stop-policy matching.

## Validation

- `cargo test -p clankers-agent --lib turn::`: PASS (51 tests).
- `cargo check -p clankers-agent`: PASS.
- `cargo test -p clankers-engine --lib`: PASS.
- `cargo test -p clankers-engine-host --lib`: PASS.
- `cargo test -p clankers-controller --test fcis_shell_boundaries`: PASS (34 tests).
- `./scripts/check-llm-contract-boundary.sh`: PASS.

## Result

The public agent prompt path now executes through the composable host runner while preserving existing shell adapters and public `Agent` assembly. Existing entrypoints keep their shared `Agent` seam and therefore share the migrated execution path.
