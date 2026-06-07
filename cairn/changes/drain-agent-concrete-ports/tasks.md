# Tasks: Drain Agent Concrete Ports

## Phase 1: Inventory

- [ ] [serial] R1: Record the current `clankers-agent` concrete dependency inventory, adapter owners, and drain target for each of the eight concrete dependency families. r[remaining-coupling-drain.agent-concrete-ports.inventory] [covers=remaining-coupling-drain.agent-concrete-ports.inventory]

## Phase 2: Implementation

- [ ] [serial] I1: Move prompt, skill, storage/search, hook, procmon, and model-selection/cost interactions used by reusable turn policy behind host-injected agent service ports or neutral DTOs. r[remaining-coupling-drain.agent-concrete-ports.host-injected-services] [covers=remaining-coupling-drain.agent-concrete-ports.host-injected-services]
- [ ] [serial] I2: Keep provider-native types limited to the declared model adapter seam and prevent provider/router/auth imports from reusable turn policy modules. r[remaining-coupling-drain.agent-concrete-ports.provider-adapter-only] [covers=remaining-coupling-drain.agent-concrete-ports.provider-adapter-only]
- [ ] [serial] I3: Lower or split `AGENT_CONCRETE_DEPENDENCY_BUDGET` and refresh the dependency ownership receipt after each drained family. r[remaining-coupling-drain.agent-concrete-ports.budget-decreases] [covers=remaining-coupling-drain.agent-concrete-ports.budget-decreases]

## Phase 3: Verification

- [ ] [serial] V1: Run focused agent port tests, concrete-dependency budget/source rails, provider-neutral DTO rails, and `cargo check --tests` for affected agent/root callers. r[remaining-coupling-drain.agent-concrete-ports.validation] [covers=remaining-coupling-drain.agent-concrete-ports.validation]
- [ ] [serial] V2: Run Cairn gates, `nix run .#cairn -- validate --root .`, and `git diff --check` before closeout. r[remaining-coupling-drain.agent-concrete-ports.closeout] [covers=remaining-coupling-drain.agent-concrete-ports.closeout]
