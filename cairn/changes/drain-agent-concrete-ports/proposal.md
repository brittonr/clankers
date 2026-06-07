# Change: Drain Agent Concrete Ports

## Why

The dependency ownership inventory still reports `clankers-agent` with eight concrete internal dependencies: `clankers-db`, `clankers-hooks`, `clankers-model-selection`, `clankers-procmon`, `clankers-prompts`, `clankers-provider`, `clankers-skills`, and `clankers-util`. The agent has model/tool/cost/cancellation ports, but reusable turn policy still depends on enough concrete desktop/orchestration systems that new features can leak policy back into the agent crate.

## What Changes

- Inventory every remaining concrete agent dependency by adapter family and name whether it is a true turn-policy input or an app-edge construction detail.
- Move prompt, skill, storage/search, hook, procmon, model-selection/cost, and utility behavior behind explicit host-injected service ports or narrower neutral DTOs where appropriate.
- Keep provider-native request construction limited to the model adapter seam and prevent provider/router/auth details from appearing in reusable turn policy.
- Lower or split the `AGENT_CONCRETE_DEPENDENCY_BUDGET` once a family drains, and update the ownership receipt so future regressions fail deterministically.

## Impact

- **Files**: `crates/clankers-agent/src/{lib.rs,builder.rs,turn,tool,context,compaction,events}.rs`, agent Cargo manifest, root construction adapters, architecture/source rails, and generated ownership receipts.
- **Testing**: focused agent port tests, concrete-dependency budget rail, provider-neutral DTO rail, `cargo check --tests` for agent/root callers, Cairn gates, and diff checks.
