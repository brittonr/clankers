# Agent concrete dependency budget evidence

Evidence-ID: agent-concrete-dependency-budget
Artifact-Type: command-output-summary
Task-ID: V2
Covers: agent-concrete-dependency-drain.dependency-budget
Date: 2026-06-02
Status: PASS

## Commands

```text
TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= cargo nextest run -p clankers-agent concrete_dependency_budget
TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= ./scripts/check-lego-architecture-boundaries.rs
nix run .#cairn -- gate tasks agent-concrete-dependency-drain --root .
nix run .#cairn -- validate --root .
git diff --check
```

## Relevant output

```text
cargo nextest run -p clankers-agent concrete_dependency_budget
PASS clankers-agent turn::ports::tests::concrete_dependency_budget_names_current_agent_edges_and_selected_config_slice
Summary: 1 test run: 1 passed, 194 skipped

./scripts/check-lego-architecture-boundaries.rs
lego architecture dependency ownership inventory written to target/lego-architecture/dependency-ownership-inventory.json

nix run .#cairn -- gate tasks agent-concrete-dependency-drain --root .
"valid": true,
"verdict": "PASS"

nix run .#cairn -- validate --root .
"valid": true

git diff --check
exit 0
```

## Coverage notes

`AGENT_CONCRETE_DEPENDENCY_BUDGET` now names provider, storage/search, config, procmon, display/protocol, and router families with owners and convergence conditions. The selected config slice records that Steel tool substrate settings are converted to `AgentToolSteelSubstrateSettings` at the agent shell edge, and the architecture rail rejects `clankers_config::` imports in `turn/steel_tool_substrate.rs`.
