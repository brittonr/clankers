Artifact-Type: validation-log
Task-ID: I5,V4
Covers: r[remaining-coupling-drain.agent-concrete-ports.host-injected-services], r[remaining-coupling-drain.agent-concrete-ports.budget-decreases], r[remaining-coupling-drain.agent-concrete-ports.validation]
Status: pass

## Scope

Drained the `clankers-procmon` concrete dependency from `clankers-agent`:

- `ProcessMeta` and `ProcessEvent` moved to neutral `clanker-message` process observation contracts.
- `clankers-procmon` reexports and emits the neutral process event/meta types.
- `AgentEvent::ProcessSpawn` and `process_event_to_agent()` now use `clanker_message` types, so process monitor construction remains at root/tool edges.
- `clankers-agent/Cargo.toml` no longer depends on `clankers-procmon`.

## Validation

Commands run from repository root:

```text
cargo check -p clankers-agent -p clankers-procmon -p clanker-message
cargo test -p clankers-agent concrete_dependency_budget_names_current_agent_edges_and_selected_config_slice
cargo test -p clankers-procmon
cargo check -p clankers
cargo check -p clankers-agent -p clankers --tests
scripts/check-lego-architecture-boundaries.rs
scripts/check-workspace-layering-rails.rs
```

All commands exited 0.

## Result

The dependency-ownership rail now records `clankers-agent` with 6 concrete dependencies instead of 7. The removed concrete edge is `clankers-procmon`.
