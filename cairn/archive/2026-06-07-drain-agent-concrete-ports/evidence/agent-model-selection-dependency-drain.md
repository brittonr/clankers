Artifact-Type: validation-log
Task-ID: I6,V5
Covers: r[remaining-coupling-drain.agent-concrete-ports.host-injected-services], r[remaining-coupling-drain.agent-concrete-ports.budget-decreases], r[remaining-coupling-drain.agent-concrete-ports.validation]
Status: pass

## Scope

Drained the `clankers-model-selection` concrete dependency from `clankers-agent`:

- Added agent-owned routing and cost contracts in `clankers-agent::routing`.
- `AgentBuilderConfig` now accepts neutral `AgentRoutingPolicy`, `AgentCostRecorder`, and display `CostProvider` services instead of concrete `RoutingPolicy` / `CostTracker` values.
- Root `src/agent_config.rs` owns the app-edge adapters from `clankers-model-selection` policy/tracker types into the neutral agent contracts.
- `BudgetEvent` moved to neutral `clanker-message` cost contracts and remains reexported by `clankers-model-selection`.
- `clankers-agent/Cargo.toml` no longer depends on `clankers-model-selection`.

## Validation

Commands run from repository root:

```text
cargo check -p clankers-agent -p clankers-model-selection -p clankers
cargo test -p clankers-agent concrete_dependency_budget_names_current_agent_edges_and_selected_config_slice
cargo test -p clankers agent_builder_config_constructs_routing_and_cost_at_app_edge
cargo check -p clankers-agent -p clankers --tests
cargo test -p clankers-model-selection
scripts/check-lego-architecture-boundaries.rs
scripts/check-workspace-layering-rails.rs
```

All commands exited 0.

## Result

The dependency-ownership rail now records `clankers-agent` with 5 concrete dependencies instead of 6. The removed concrete edge is `clankers-model-selection`.
