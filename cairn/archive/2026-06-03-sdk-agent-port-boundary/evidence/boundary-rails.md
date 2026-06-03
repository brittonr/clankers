# Agent port boundary rail evidence

Evidence-ID: sdk-agent-port-boundary-rails
Artifact-Type: command-output-summary
Task-ID: V2
Covers: sdk-agent-port-boundary.verification.boundary-rail
Date: 2026-06-03
Status: PASS

## Commands

```text
TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= cargo nextest run -p clankers-agent concrete_dependency_budget_names_current_agent_edges_and_selected_config_slice
./scripts/check-agent-turn-port-boundary.rs
./scripts/check-lego-architecture-boundaries.rs
```

## Relevant output

```text
concrete_dependency_budget_names_current_agent_edges_and_selected_config_slice
PASS clankers-agent turn::ports::tests::concrete_dependency_budget_names_current_agent_edges_and_selected_config_slice
Summary: 1 test run: 1 passed, 193 skipped

./scripts/check-agent-turn-port-boundary.rs
ok: agent turn port boundary rail passed

./scripts/check-lego-architecture-boundaries.rs
lego architecture dependency ownership inventory written to target/lego-architecture/dependency-ownership-inventory.json
```

## Coverage notes

`AGENT_CONCRETE_DEPENDENCY_BUDGET` names provider, storage/search, config, procmon, display/protocol, and router families with owners and convergence conditions. The source rail now also anchors `SDK_AGENT_PORT_BOUNDARY_INVENTORY_REQUIREMENT`, `SDK_AGENT_PORT_BOUNDARY_PORT_REQUIREMENT`, and `SDK_AGENT_PORT_BOUNDARY_RAIL_REQUIREMENT`, so the active Cairn requirement IDs remain tied to the checked agent-port rail.
