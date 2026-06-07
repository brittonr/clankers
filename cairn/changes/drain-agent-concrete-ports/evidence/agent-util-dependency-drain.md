Artifact-Type: validation-log
Task-ID: I4,V3
Covers: r[remaining-coupling-drain.agent-concrete-ports.host-injected-services], r[remaining-coupling-drain.agent-concrete-ports.budget-decreases], r[remaining-coupling-drain.agent-concrete-ports.validation]
Status: pass

## Scope

Drained the `clankers-util` concrete dependency from `clankers-agent`:

- Token estimation moved to neutral `clanker-message::token` helpers; `clankers-util::token` now remains as a compatibility reexport.
- Tool path policy moved to `clankers-tool-host::path_policy`, keeping path checks with reusable tool-host execution contracts.
- Root sandbox initialization now reexports the `clankers-tool-host` path policy, so the agent and root initialize/check the same global policy.
- `clankers-agent/Cargo.toml` no longer depends on `clankers-util`.

## Validation

Commands run from repository root:

```text
cargo check -p clankers-agent -p clankers-tool-host -p clankers-util
cargo test -p clankers-agent concrete_dependency_budget_names_current_agent_edges_and_selected_config_slice
cargo test -p clanker-message token
cargo test -p clankers-tool-host path_policy
cargo check -p clankers
scripts/check-lego-architecture-boundaries.rs
scripts/check-workspace-layering-rails.rs
```

All commands exited 0.

## Result

The dependency-ownership rail now records `clankers-agent` with 7 concrete dependencies instead of 8. The removed concrete edge is `clankers-util`.
