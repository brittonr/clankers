# Steel Repo Evolution Packs Delta

## Requirement: Repo-local packs load in real turn paths [r[steel-repo-evolution-packs.runtime-turn-load]]
Clankers MUST evaluate repo-local Steel evolution pack activation from the actual agent turn planning path, not only from standalone validators.

### Scenario: turn planning checks repo-local pack [r[steel-repo-evolution-packs.runtime-turn-load.turn-path]]
- GIVEN a repository contains `.clankers/steel/evolution-profile.ncl`, exported JSON, and referenced scripts
- WHEN an agent turn begins planning through the normal or orchestrated turn path
- THEN Clankers MUST call Rust repo-pack activation validation before turn planning proceeds
- AND activation status MUST be surfaced only as safe receipt metadata

### Scenario: absent pack remains silent default-deny [r[steel-repo-evolution-packs.runtime-turn-load.absent]]
- GIVEN a repository has no repo-local Steel evolution profile
- WHEN an agent turn begins
- THEN Clankers MUST leave repo-local evolution inactive without emitting a repo-local authorship claim
- AND bundled/default orchestration MUST remain available

## Requirement: Higher-order contracts guard host calls [r[steel-repo-evolution-packs.higher-order-contracts]]
Each repo-local evolution host call MUST be wrapped by a higher-order contract declared by the repo-local pack and enforced by Rust before activation or plan acceptance.

### Scenario: allowed host calls require contracts [r[steel-repo-evolution-packs.higher-order-contracts.allowed-covered]]
- GIVEN a repo-local pack lists allowed host calls
- WHEN Rust validates the pack
- THEN every allowed host call MUST have a matching `host_contracts` entry with `mode = higher_order`
- AND the contract MUST include non-empty preconditions and postconditions

### Scenario: missing contract blocks plan action [r[steel-repo-evolution-packs.higher-order-contracts.plan-denied]]
- GIVEN a Steel evolution plan requests a host call without a higher-order contract
- WHEN Rust evaluates the typed plan
- THEN Clankers MUST deny the plan before the host effect
- AND the receipt MUST identify the denied host call class without raw prompt or script content

### Scenario: Nickel source carries contract shape [r[steel-repo-evolution-packs.higher-order-contracts.nickel-source]]
- GIVEN a repo-local Steel evolution profile is authored in Nickel
- WHEN focused verification runs
- THEN verification MUST check that Nickel source carries the pack, script, host-contract, budget, host-call, receipt-root, and fallback-mode contract markers
- AND the exported JSON MUST still pass Rust typed validation before activation
