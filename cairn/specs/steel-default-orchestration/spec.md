# Steel Default Orchestration Specification

## Purpose

Defines the `steel-default-orchestration` capability for making Steel Scheme the reviewed default orchestration/planning seam for selected Clankers agent decisions while preserving Rust-owned enforcement and host-effect seams.

## Requirements

### Requirement: Steel orchestration is policy selected [r[steel-default-orchestration.policy-selected-default]]

Clankers MUST enable Steel default orchestration only through a reviewed orchestration profile/policy that names the planning seam, script source, script hash requirement, runtime budget profile, fallback mode, allowed host actions, receipt requirements, and rollout stage.

#### Scenario: default profile names the seam [r[steel-default-orchestration.policy-selected-default.named-seam]]
- GIVEN a profile enables Steel orchestration by default
- WHEN Clankers starts a supported planning decision
- THEN the selected profile MUST name the exact planning seam
- AND it MUST NOT apply Steel orchestration globally to unrelated decisions

#### Scenario: disabled profile uses Rust-native planner [r[steel-default-orchestration.policy-selected-default.disabled]]
- GIVEN policy disables Steel orchestration
- WHEN the same planning decision occurs
- THEN Clankers MUST use the Rust-native planner
- AND it MUST emit no claim that Steel authored the decision

### Requirement: Rust owns the orchestration adapter seam [r[steel-default-orchestration.rust-adapter-seam]]

Clankers MUST route Steel orchestration through a Rust-owned adapter that implements the same planner interface as the Rust-native planner. Agent, controller, daemon, TUI, attach, provider, and tool-host shells MUST NOT construct Steel interpreter internals directly.

#### Scenario: callers depend on planner interface [r[steel-default-orchestration.rust-adapter-seam.interface]]
- GIVEN a caller needs an orchestration decision
- WHEN Steel orchestration is available
- THEN the caller MUST invoke the Rust planner interface or adapter
- AND it MUST NOT branch on interpreter-specific APIs

#### Scenario: wrapper owns evaluation [r[steel-default-orchestration.rust-adapter-seam.wrapper-only]]
- GIVEN the Steel planner evaluates a script
- WHEN evaluation occurs
- THEN it MUST call the existing Clankers Steel runtime wrapper
- AND it MUST preserve wrapper-owned profile, budget, host-function, and receipt behavior

### Requirement: Steel returns typed plans only [r[steel-default-orchestration.typed-plan-output]]

Steel orchestration MUST return typed versioned plans or dynamic-runtime action envelopes. Free-form textual script output MUST NOT be executable authority.

#### Scenario: typed plan is accepted for review [r[steel-default-orchestration.typed-plan-output.accepted]]
- GIVEN a Steel script returns a supported plan schema
- WHEN Rust parses the plan
- THEN Rust MAY continue to policy and authority checks
- AND the receipt MUST include the plan schema, script hash, profile, and redaction class

#### Scenario: malformed plan falls back or blocks [r[steel-default-orchestration.typed-plan-output.malformed]]
- GIVEN a Steel script returns malformed or unsupported output
- WHEN Rust parses the plan
- THEN Rust MUST reject the plan with a stable issue code
- AND Rust MAY use the Rust-native fallback only when policy allows fallback

### Requirement: Host effects remain Rust authorized [r[steel-default-orchestration.rust-authorized-effects]]

Every effectful action requested by a Steel plan MUST cross Rust authorization before execution, including Nickel policy checks, UCAN/session capability checks, disabled-tool checks, runtime profile checks, provider/router ownership, and mutation preflight/apply/rollback seams where applicable.

#### Scenario: allowed action envelope crosses existing seam [r[steel-default-orchestration.rust-authorized-effects.allowed-envelope]]
- GIVEN Steel returns a dynamic-runtime action envelope allowed by policy
- WHEN Rust evaluates the plan
- THEN Rust MUST authorize it through the existing dynamic-runtime authorization seam
- AND the Steel script MUST NOT execute the effect directly

#### Scenario: denied host action performs no fallback effect [r[steel-default-orchestration.rust-authorized-effects.denied-envelope]]
- GIVEN Steel returns an unknown, disabled, unauthorized, or over-budget host action
- WHEN Rust evaluates the plan
- THEN Rust MUST deny the action before any host effect
- AND it MUST NOT retry through filesystem, process, git, network, provider, credential, daemon, TUI, or native-tool fallback authority

### Requirement: Fallback is explicit and receipt-backed [r[steel-default-orchestration.fallback-and-receipts]]

Steel orchestration failure MUST produce deterministic receipts and use Rust-native fallback only when policy explicitly allows it. Fallback MUST NOT loosen the Steel runtime profile or silently hide repeated Steel failures.

#### Scenario: script failure emits fallback receipt [r[steel-default-orchestration.fallback-and-receipts.script-failure]]
- GIVEN Steel script load, evaluation, or typed-plan parsing fails
- WHEN fallback is allowed
- THEN Clankers MUST emit a receipt recording the Steel failure class, script/profile identity, fallback policy, and Rust-native fallback decision class
- AND it MUST NOT include raw secrets, credentials, provider payloads, or unbounded script output

#### Scenario: fallback disabled blocks safely [r[steel-default-orchestration.fallback-and-receipts.fallback-disabled]]
- GIVEN Steel script load, evaluation, or typed-plan parsing fails
- WHEN fallback is disabled
- THEN Clankers MUST block the orchestration decision with a stable issue code
- AND no host effect may execute from the failed Steel plan

### Requirement: Rollout evidence precedes default expansion [r[steel-default-orchestration.rollout-evidence]]

Before Steel becomes default for additional planning seams, Clankers MUST collect comparison evidence between Steel planner output and Rust-native planner output for the reviewed seam, including plan hashes, decision class, authorized effect summary, denial summary, and fallback status.

#### Scenario: comparison receipt is stable [r[steel-default-orchestration.rollout-evidence.comparison-receipt]]
- GIVEN Steel orchestration is enabled in comparison mode
- WHEN a planning decision is evaluated
- THEN Clankers MUST emit a deterministic comparison receipt
- AND the receipt MUST be stable across repeated runs with identical inputs and policies

#### Scenario: expansion requires reviewed profile update [r[steel-default-orchestration.rollout-evidence.reviewed-expansion]]
- GIVEN a new planning seam is proposed for Steel default orchestration
- WHEN the profile is updated
- THEN the update MUST include reviewed policy, fixtures, fallback behavior, and receipt evidence for that seam
- AND it MUST NOT inherit authority from another seam implicitly
