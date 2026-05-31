# Agent turn port boundary rail evidence

Evidence-ID: agent-turn-port-boundary-rails
Artifact-Type: command-output-summary
Task-ID: V2
Covers: coupling-hotspot-remediation.agent-port-boundary
Date: 2026-05-31
Status: PASS

## Commands

```text
./scripts/check-agent-turn-port-boundary.rs
TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= cargo test -p clankers-agent --lib fake_runtime_service_bundle_turn_runs_without_desktop_systems
TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= cargo test -p clankers-agent --lib standalone_agent_shell_adapter_parity_cases_preserve_engine_inputs_and_terminal_outcomes
```

## Relevant output

```text
ok: agent turn port boundary rail passed

running 1 test
test turn::tests::fake_runtime_service_bundle_turn_runs_without_desktop_systems ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 184 filtered out

running 1 test
test turn::tests::standalone_agent_shell_adapter_parity_cases_preserve_engine_inputs_and_terminal_outcomes ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 184 filtered out
```

## Coverage notes

The static rail requires `crates/clankers-agent/src/turn/ports.rs` to own explicit runtime ports for model execution, tool inventory/execution, cost tracking, and cancellation, plus service receipts for model, tool registry, storage, prompt context, hooks, skills, cost, and cancellation. It also checks that `run_turn_loop` takes `TurnLoopContext`/`AgentRuntimeServices` instead of exposing concrete provider, tool-map, DB, TUI, or auth state in its runtime signature.

The fake-port turn fixture runs a full prompt/model/completion path with fake model, tool, cost, and cancellation ports and without constructing concrete provider/router/auth/db/TUI state. The shell adapter parity fixture keeps the existing desktop adapter behavior covered against the engine turn inputs and terminal outcomes.
