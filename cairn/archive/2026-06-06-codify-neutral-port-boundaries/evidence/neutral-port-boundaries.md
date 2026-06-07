Artifact-Type: implementation-validation-evidence
Task-ID: I1,I2,I3,V1
Covers: remaining-coupling-drain.agent-concrete-dependencies.port-boundary-rule
Status: complete

## Reviewed-Evidence

Implementation inventory:

- `crates/clankers-agent/src/turn/ports.rs` owns the neutral agent turn port inventory: `AgentModelPort`, `AgentToolPort`, `AgentCostPort`, `AgentCancellationPort`, `AgentRuntimeServices`, `ProviderModelPort`, `ControllerToolPort`, `CostTrackerPort`, `TokenCancellationPort`, and `DESKTOP_AGENT_SERVICE_RECEIPTS`.
- `crates/clankers-agent/src/turn/mod.rs` owns `TurnLoopContext { services: AgentRuntimeServices<'_>, ... }` and the socketless/fake seam test `fake_runtime_service_bundle_turn_runs_without_desktop_systems`.
- `crates/clankers-agent/src/turn/execution.rs` converts engine-native model requests at the shell boundary via `completion_request_from_engine_request`.
- `docs/src/tutorials/embedded-agent-sdk.md` documents adapter-only modular coupling rules and explicit host/service injection for generic SDK crates.

Commands run:

```text
scripts/check-agent-turn-port-boundary.rs
ok: agent turn port boundary rail passed

scripts/check-lego-architecture-boundaries.rs
lego architecture dependency ownership inventory written to target/lego-architecture/dependency-ownership-inventory.json

env TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= cargo nextest run -p clankers-controller --test fcis_shell_boundaries
Summary [1.181s] 44 tests run: 44 passed, 0 skipped
```

## Decision

The first enforced seam is the agent turn loop. Reusable turn policy now consumes injected neutral ports, while desktop provider/tool/cost/cancellation behavior remains in adapter structs and root/app-edge assembly.

## Follow-Up

Future drain slices should reduce the remaining `clankers-agent` concrete dependency budget family by family and refresh `policy/lego-architecture/dependency-ownership-baseline.json` when an edge shrinks.
