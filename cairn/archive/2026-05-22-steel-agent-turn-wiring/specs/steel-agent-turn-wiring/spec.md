# Steel Agent Turn Wiring Specification

## Purpose

Defines the `steel-agent-turn-wiring` capability for connecting the existing Steel Scheme `steel.host.plan_turn` planner seam to the real Clankers agent-turn path while preserving Rust-owned enforcement, authorization, fallback, and host-effect boundaries.

## Requirements

### Requirement: Real agent turns use a Rust-owned planning port [r[steel-agent-turn-wiring.turn-planning-port]]

Clankers MUST route the selected real agent-turn planning decision through a Rust-owned planning port or adapter that can choose Rust-native or Steel-backed planning without exposing Steel interpreter internals to agent, controller, daemon, TUI, provider, or tool-host shells.

#### Scenario: enabled seam invokes the planning port [r[steel-agent-turn-wiring.turn-planning-port.enabled]]
- GIVEN policy enables `steel.host.plan_turn` for comparison or default mode
- WHEN the real agent-turn path reaches the selected planning decision
- THEN the shell MUST call the Rust-owned planning port
- AND the port MAY invoke the Steel-backed planner through `clankers-runtime::steel_orchestration`
- AND the shell MUST NOT construct or import Steel interpreter internals directly

#### Scenario: disabled seam stays Rust-native [r[steel-agent-turn-wiring.turn-planning-port.disabled]]
- GIVEN policy disables `steel.host.plan_turn`
- WHEN the same real agent-turn decision occurs
- THEN Clankers MUST use the Rust-native planner path
- AND it MUST NOT emit a receipt claiming Steel authored the decision

### Requirement: Policy selects comparison or default mode [r[steel-agent-turn-wiring.policy-selected-mode]]

Nickel-authored policy/profile data MUST select whether `steel.host.plan_turn` is disabled, comparison-only, or default. Steel scripts MUST NOT self-select default status, add host functions, loosen budgets, or override fallback policy.

#### Scenario: comparison mode preserves Rust-native oracle [r[steel-agent-turn-wiring.policy-selected-mode.comparison]]
- GIVEN the profile selects comparison mode
- WHEN a real agent turn is planned
- THEN Clankers MUST collect a Steel planning receipt
- AND Rust-native planning MUST remain the execution oracle unless a later reviewed policy selects default mode

#### Scenario: default mode is explicit [r[steel-agent-turn-wiring.policy-selected-mode.default]]
- GIVEN the profile selects default mode for `steel.host.plan_turn`
- WHEN the real agent turn is planned
- THEN Clankers MAY use the typed Steel plan as the selected plan
- AND only for the named seam
- AND only after Rust validates schema, policy, budget, and authority

### Requirement: Steel turn plans remain typed and bounded [r[steel-agent-turn-wiring.typed-turn-plan]]

Steel-backed turn planning MUST consume bounded, redacted, hashable turn inputs and return typed versioned plan receipts. Free-form Steel text, raw script output, raw prompts, credentials, provider payloads, full transcripts, and unbounded tool output MUST NOT become executable authority or receipt material.

#### Scenario: typed plan is accepted for authorization [r[steel-agent-turn-wiring.typed-turn-plan.accepted]]
- GIVEN Steel returns a supported `steel.host.plan_turn` plan schema
- WHEN Rust parses the plan
- THEN Rust MAY proceed to authorization and fallback selection
- AND the receipt MUST include stable schema, profile, script, policy, plan hash, and redaction metadata

#### Scenario: malformed plan cannot execute [r[steel-agent-turn-wiring.typed-turn-plan.malformed]]
- GIVEN Steel returns malformed, unsupported, over-budget, or unredactable output
- WHEN Rust parses the turn plan
- THEN Rust MUST reject the plan with a stable issue code
- AND no host effect may execute from that plan

### Requirement: Host effects remain Rust authorized [r[steel-agent-turn-wiring.rust-authorized-effects]]

Every effectful action proposed by a Steel turn plan MUST cross existing Rust authorization seams before execution, including dynamic-runtime action authorization, Nickel policy, UCAN/session capabilities, disabled-tool policy, provider/router request ownership, tool-host execution checks, session/daemon mutation checks, and mutation preflight/apply/rollback seams where applicable.

#### Scenario: allowed effect crosses dynamic-runtime authorization [r[steel-agent-turn-wiring.rust-authorized-effects.allowed]]
- GIVEN Steel proposes a supported host action in the typed turn plan
- WHEN Rust evaluates the plan
- THEN Rust MUST authorize the action through the existing dynamic-runtime or host-specific authorization seam before any effect
- AND the receipt MUST record an authorized summary without raw secrets or payloads

#### Scenario: denied effect performs no host work [r[steel-agent-turn-wiring.rust-authorized-effects.denied]]
- GIVEN Steel proposes an unknown, disabled, unauthorized, over-budget, provider, credential, daemon, TUI, native-tool, filesystem, process, git, network, or mutation action without required authority
- WHEN Rust evaluates the plan
- THEN Rust MUST deny the action before any host effect
- AND it MUST NOT bypass denial through fallback filesystem, shell, provider, tool, daemon, or session authority

### Requirement: Fallback is explicit and receipt-backed [r[steel-agent-turn-wiring.fallback-receipts]]

Steel turn-planning failures MUST produce deterministic receipts and use Rust-native fallback only when policy explicitly allows it. Fallback MUST NOT loosen Steel runtime profiles, hide repeated failures, or retry Steel under broader authority.

#### Scenario: allowed fallback records reason [r[steel-agent-turn-wiring.fallback-receipts.allowed]]
- GIVEN Steel planning fails due to disabled profile, script load failure, evaluation failure, parse failure, malformed output, denied authorization, or receipt validation failure
- WHEN policy allows Rust-native fallback
- THEN Clankers MUST emit a bounded receipt recording the failure class and fallback decision class
- AND Rust-native planning MAY continue without executing the failed Steel plan

#### Scenario: fallback-disabled blocks safely [r[steel-agent-turn-wiring.fallback-receipts.blocked]]
- GIVEN Steel planning fails
- WHEN policy disables fallback
- THEN Clankers MUST block the selected planning decision with a stable issue code
- AND no host effect may execute from the failed Steel plan

### Requirement: Wiring has deterministic dogfood evidence [r[steel-agent-turn-wiring.dogfood-evidence]]

The implementation MUST include focused fixture-backed verification proving the real agent-turn wiring path, disabled path, comparison/default selection, fallback behavior, denied host-effect behavior, and repeated-run receipt stability.

#### Scenario: fixture proves real wiring [r[steel-agent-turn-wiring.dogfood-evidence.real-path]]
- GIVEN a fixture agent-turn input and policy enabling Steel comparison mode
- WHEN the real adapter/turn-planning boundary is exercised
- THEN the test MUST prove Steel planning was invoked through the Rust-owned port
- AND Rust-native fallback/oracle behavior remained available

#### Scenario: receipts are stable and redacted [r[steel-agent-turn-wiring.dogfood-evidence.stable-redacted]]
- GIVEN identical fixture inputs, policy, profile, and script hashes
- WHEN the turn-planning boundary is evaluated repeatedly
- THEN the resulting receipt hashes MUST be stable
- AND receipts MUST NOT include credentials, raw provider payloads, raw prompts, raw transcripts, or raw script bodies
