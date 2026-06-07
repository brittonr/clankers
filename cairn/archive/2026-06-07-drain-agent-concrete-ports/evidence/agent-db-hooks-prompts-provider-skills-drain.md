Artifact-Type: validation-log
Task-ID: R1,I1,I2,I3,V1
Covers: r[remaining-coupling-drain.agent-concrete-ports.inventory], r[remaining-coupling-drain.agent-concrete-ports.host-injected-services], r[remaining-coupling-drain.agent-concrete-ports.provider-adapter-only], r[remaining-coupling-drain.agent-concrete-ports.budget-decreases], r[remaining-coupling-drain.agent-concrete-ports.validation]
Status: pass

## Scope

Drained the remaining normal concrete dependencies from `clankers-agent`:

- `clankers-db` and `clankers-hooks` moved behind neutral agent/tool service ports with root/controller adapters.
- `clankers-prompts` and `clankers-skills` discovery moved to the root prompt-resource adapter; `clankers-agent` now owns only neutral prompt DTOs.
- `clankers-provider` request execution moved behind `AgentModelService` / `AgentCompletionRequest`; provider-native translation is at root/controller/test edges.
- `ToolContext` no longer carries concrete `db`, `hook_pipeline`, or `search_index` fields; legacy concrete access is adapter-owned through typed service slots.

## Validation

Commands run from repository root:

```text
cargo test -p clankers-agent concrete_dependency_budget_names_current_agent_edges_and_selected_config_slice
cargo test -p clankers-agent controller_tool_services_build_neutral_invocation_context
cargo check -p clankers-agent --tests
cargo check -p clankers-controller --tests
cargo check -p clankers --tests
cargo check -p clankers
scripts/check-lego-architecture-boundaries.rs
scripts/check-workspace-layering-rails.rs
```

All commands exited 0.

## Result

`crates/clankers-agent/Cargo.toml` has zero normal dependencies on the former concrete families (`clankers-db`, `clankers-hooks`, `clankers-prompts`, `clankers-provider`, `clankers-skills`, plus earlier-drained `clankers-util`, `clankers-procmon`, and `clankers-model-selection`). The dependency-ownership baseline now records `clankers-agent` with `concrete_dependency_count: 0` for normal production dependencies.
