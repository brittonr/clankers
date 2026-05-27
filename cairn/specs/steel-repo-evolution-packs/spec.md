# Steel Repo Evolution Packs Specification

## Purpose

Defines the `steel-repo-evolution-packs` capability.

## Requirements

### Requirement: Repo-local Steel evolution packs are runtime loaded [r[steel-repo-evolution-packs.discovery]]
Clankers MUST support repo-local Steel evolution packs that are discovered and validated at runtime without requiring Rust recompilation.

#### Scenario: absent pack is default-deny [r[steel-repo-evolution-packs.discovery.default-deny]]
- GIVEN a repository has no `.clankers/steel/evolution-profile.ncl`
- WHEN Clankers starts or plans a turn
- THEN no repo-local Steel evolution pack MUST be active
- AND Clankers MUST continue using the bundled/default orchestration path without claiming repo-local Steel authorship

#### Scenario: present pack validates before activation [r[steel-repo-evolution-packs.discovery.validate-before-activation]]
- GIVEN a repository contains `.clankers/steel/evolution-profile.ncl` and referenced Steel scripts
- WHEN Clankers discovers the pack
- THEN Rust MUST validate the Nickel contract, exported schema, script paths, BLAKE3 hashes, budgets, allowed host calls, receipt root, and fallback policy before activation
- AND invalid packs MUST fail closed without executing Steel script code

#### Scenario: pack reload is hash-bound [r[steel-repo-evolution-packs.discovery.hash-bound-reload]]
- GIVEN a repo-local Steel pack changes on disk
- WHEN Clankers reloads the pack
- THEN the new pack MUST become active only after validation succeeds
- AND activation receipts MUST record old and new profile/script hashes without raw script or prompt material

### Requirement: Repo-local packs load in real turn paths [r[steel-repo-evolution-packs.runtime-turn-load]]
Clankers MUST evaluate repo-local Steel evolution pack activation from the actual agent turn planning path, not only from standalone validators.

#### Scenario: turn planning checks repo-local pack [r[steel-repo-evolution-packs.runtime-turn-load.turn-path]]
- GIVEN a repository contains `.clankers/steel/evolution-profile.ncl`, exported JSON, and referenced scripts
- WHEN an agent turn begins planning through the normal or orchestrated turn path
- THEN Clankers MUST call Rust repo-pack activation validation before turn planning proceeds
- AND activation status MUST be surfaced only as safe receipt metadata

#### Scenario: absent pack remains silent default-deny [r[steel-repo-evolution-packs.runtime-turn-load.absent]]
- GIVEN a repository has no repo-local Steel evolution profile
- WHEN an agent turn begins
- THEN Clankers MUST leave repo-local evolution inactive without emitting a repo-local authorship claim
- AND bundled/default orchestration MUST remain available

### Requirement: Rust owns the repo evolution host ABI [r[steel-repo-evolution-packs.host-abi]]
Repo-local Steel packs MUST use a narrow versioned host ABI owned by Rust for every effectful or repo-observing action.

#### Scenario: known host calls are typed [r[steel-repo-evolution-packs.host-abi.typed-calls]]
- GIVEN a Steel evolution script requests a host action such as repo context, patch proposal, gate execution, receipt recording, or human checkpoint
- WHEN Rust receives the request
- THEN Rust MUST parse a typed versioned request schema for that host call
- AND free-form textual output MUST NOT become executable authority

#### Scenario: unknown or widened host calls fail closed [r[steel-repo-evolution-packs.host-abi.unknown-denied]]
- GIVEN a repo-local pack names an unknown host call, widens a host-call budget, or requests filesystem, shell, git, network, provider, credential, daemon, TUI, native-tool, session mutation, code mutation, or capability-minting authority outside the stable ABI
- WHEN activation or execution reaches Rust validation
- THEN Clankers MUST deny the request before any host effect
- AND fallback MUST NOT retry the action through broader authority

### Requirement: Higher-order contracts guard host calls [r[steel-repo-evolution-packs.higher-order-contracts]]
Each repo-local evolution host call MUST be wrapped by a higher-order contract declared by the repo-local pack and enforced by Rust before activation or plan acceptance.

#### Scenario: allowed host calls require contracts [r[steel-repo-evolution-packs.higher-order-contracts.allowed-covered]]
- GIVEN a repo-local pack lists allowed host calls
- WHEN Rust validates the pack
- THEN every allowed host call MUST have a matching `host_contracts` entry with `mode = higher_order`
- AND the contract MUST include non-empty preconditions and postconditions

#### Scenario: missing contract blocks plan action [r[steel-repo-evolution-packs.higher-order-contracts.plan-denied]]
- GIVEN a Steel evolution plan requests a host call without a higher-order contract
- WHEN Rust evaluates the typed plan
- THEN Clankers MUST deny the plan before the host effect
- AND the receipt MUST identify the denied host call class without raw prompt or script content

#### Scenario: Nickel source carries contract shape [r[steel-repo-evolution-packs.higher-order-contracts.nickel-source]]
- GIVEN a repo-local Steel evolution profile is authored in Nickel
- WHEN focused verification runs
- THEN verification MUST check that Nickel source carries the pack, script, host-contract, budget, host-call, receipt-root, and fallback-mode contract markers
- AND the exported JSON MUST still pass Rust typed validation before activation

### Requirement: Steel emits typed evolution plans only [r[steel-repo-evolution-packs.typed-evolution-plan]]
Repo-local Steel evolution packs MUST produce typed, versioned evolution plans that Rust validates before any follow-on work.

#### Scenario: valid plan may request bounded Rust actions [r[steel-repo-evolution-packs.typed-evolution-plan.accepted]]
- GIVEN a validated pack emits a `clankers.steel.evolution-plan.v1` plan
- WHEN Rust parses the plan
- THEN Rust MAY run policy-allowed read, gate, receipt, patch-proposal, or human-checkpoint actions through the host ABI
- AND each requested action MUST still pass Rust policy, UCAN/session, path, budget, and disabled-action checks where applicable

#### Scenario: malformed plan blocks or falls back [r[steel-repo-evolution-packs.typed-evolution-plan.malformed]]
- GIVEN a Steel script returns malformed, unsupported, or over-budget evolution output
- WHEN Rust parses the output
- THEN Clankers MUST reject it with a stable issue code
- AND fallback MAY use existing Rust-native planning only when policy explicitly allows fallback

### Requirement: Repo pack receipts are deterministic and redacted [r[steel-repo-evolution-packs.receipts]]
Clankers MUST emit deterministic redacted receipts for pack activation, plan parsing, host-call authorization, gate execution requests, fallback, and denial.

#### Scenario: activation receipt names safe pack identity [r[steel-repo-evolution-packs.receipts.activation]]
- GIVEN a repo-local Steel evolution pack activates or fails validation
- WHEN the receipt is written
- THEN it MUST include safe pack identity, schema version, profile hash, script hashes, ABI version, allowed host calls, receipt path class, validation status, and issue code when denied
- AND it MUST omit raw prompts, credentials, compact UCAN tokens, provider payloads, secrets, raw script source, and uncontrolled absolute paths

#### Scenario: plan receipt links gates and actions [r[steel-repo-evolution-packs.receipts.plan]]
- GIVEN a validated pack emits an evolution plan
- WHEN Rust accepts, rejects, or falls back from that plan
- THEN the receipt MUST include the plan schema, plan hash, selected gate names, requested host action classes, denied action classes, fallback/block status, and receipt hash
- AND repeated identical safe inputs SHOULD produce stable hash fields

### Requirement: Verification proves safe pack behavior [r[steel-repo-evolution-packs.verification]]
Implementation MUST include deterministic positive and negative tests, docs, and a focused checker proving safe discovery, activation, typed host ABI enforcement, typed plan parsing, receipt redaction, and no-recompile repo-local behavior.

#### Scenario: focused fixtures cover activation and denial [r[steel-repo-evolution-packs.verification.fixtures]]
- GIVEN fixture repositories with absent, valid, malformed, hash-mismatched, path-escaped, unknown-host-call, and over-budget Steel evolution packs
- WHEN focused verification runs
- THEN absent packs MUST remain inactive, valid packs MUST activate, and invalid packs MUST fail before Steel execution or host effects

#### Scenario: docs explain repo-local workflow [r[steel-repo-evolution-packs.verification.docs]]
- GIVEN repo-local Steel evolution packs are implemented
- WHEN operator docs are built
- THEN docs MUST describe pack layout, Nickel profile contract, no-recompile reload behavior, supported host ABI, receipts, safety boundaries, and explicit non-authorities
