# Tasks: Split Runtime Facade Contracts

## Phase 1: Inventory

- [ ] [serial] R1: Generate and record a runtime public API inventory that classifies exports as green contracts, yellow host-injection surfaces, or desktop adapter shells. r[remaining-coupling-drain.runtime-facade-contract-split.inventory] [covers=remaining-coupling-drain.runtime-facade-contract-split.inventory]

## Phase 2: Implementation

- [ ] [serial] I1: Move reusable serializable runtime DTOs to neutral contract owners or no-authority runtime modules while leaving executable policy behind host-injected services. r[remaining-coupling-drain.runtime-facade-contract-split.green-contracts] [covers=remaining-coupling-drain.runtime-facade-contract-split.green-contracts]
- [ ] [serial] I2: Keep provider, auth, plugin, process, prompt filesystem, skill, session, storage, clock, and executable Steel behavior fail-closed unless explicitly injected. r[remaining-coupling-drain.runtime-facade-contract-split.fail-closed-services] [covers=remaining-coupling-drain.runtime-facade-contract-split.fail-closed-services]
- [ ] [serial] I3: Refresh generated SDK/API docs so green contracts are not confused with yellow or desktop adapter shells. r[remaining-coupling-drain.runtime-facade-contract-split.docs] [covers=remaining-coupling-drain.runtime-facade-contract-split.docs]
- [x] [serial] I4: Remove reusable host-crate singleton path-policy state while preserving fail-closed path checks at the tool-host boundary. r[remaining-coupling-drain.runtime-facade-contract-split.fail-closed-services] [covers=remaining-coupling-drain.runtime-facade-contract-split.fail-closed-services] [evidence=cairn/changes/split-runtime-facade-contracts/evidence/tool-host-path-policy-global-state-drain.md]
- [x] [serial] I5: Reuse neutral `clanker_message::PluginSummary` for daemon plugin-list protocol events instead of carrying a duplicate protocol-owned display DTO. r[remaining-coupling-drain.runtime-facade-contract-split.green-contracts] [covers=remaining-coupling-drain.runtime-facade-contract-split.green-contracts] [evidence=cairn/changes/split-runtime-facade-contracts/evidence/protocol-plugin-summary-neutral-dto.md]

## Phase 3: Verification

- [ ] [serial] V1: Run runtime public API inventory rails, fail-closed service tests, Steel contract split fixtures, and SDK/lego docs rails. r[remaining-coupling-drain.runtime-facade-contract-split.validation] [covers=remaining-coupling-drain.runtime-facade-contract-split.validation]
- [ ] [serial] V2: Run affected cargo checks, Cairn gates/validate, and `git diff --check` before closeout. r[remaining-coupling-drain.runtime-facade-contract-split.closeout] [covers=remaining-coupling-drain.runtime-facade-contract-split.closeout]
- [x] [serial] V3: Validate the host path-policy singleton drain with FCIS boundary and path-policy focused tests. r[remaining-coupling-drain.runtime-facade-contract-split.validation] [covers=remaining-coupling-drain.runtime-facade-contract-split.validation] [evidence=cairn/changes/split-runtime-facade-contracts/evidence/tool-host-path-policy-global-state-drain.md]
- [x] [serial] V4: Validate the neutral plugin-summary protocol DTO drain with affected cargo checks, protocol frame round trips, and architecture layering rails. r[remaining-coupling-drain.runtime-facade-contract-split.validation] [covers=remaining-coupling-drain.runtime-facade-contract-split.validation,remaining-coupling-drain.runtime-facade-contract-split.closeout] [evidence=cairn/changes/split-runtime-facade-contracts/evidence/protocol-plugin-summary-neutral-dto.md]
