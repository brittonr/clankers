## MODIFIED Requirements

### Requirement: Steel orchestration is policy selected [r[steel-default-orchestration.policy-selected-default]]
Clankers MUST enable Steel default orchestration only through a reviewed orchestration profile/policy that names the planning seam, script source, script hash requirement, runtime budget profile, fallback mode, allowed host actions, receipt requirements, and rollout stage.

#### Scenario: default profile names the seam [r[steel-default-orchestration.policy-selected-default.named-seam]]
- GIVEN Clankers uses the bundled default Steel orchestration profile
- WHEN Clankers starts a supported real turn-planning decision
- THEN the selected profile MUST name `steel.host.plan_turn` as the exact planning seam
- AND it MUST NOT apply Steel orchestration globally to unrelated decisions

#### Scenario: default profile can select Steel as planner [r[steel-default-orchestration.policy-selected-default.default-selected]]
- GIVEN the bundled reviewed profile selects default rollout for `steel.host.plan_turn`
- WHEN Rust validates the profile, script, policy, session capability, UCAN ability, disabled actions, budget, and receipt destination
- THEN Clankers MAY use the typed Steel plan as the selected planner output for that turn-planning decision
- AND Rust MUST remain the authority for every provider call, tool call, daemon/session update, mutation, fallback, block decision, and receipt

#### Scenario: explicit disabled profile uses Rust-native planner [r[steel-default-orchestration.policy-selected-default.disabled]]
- GIVEN operator settings explicitly disable Steel turn planning
- WHEN the same planning decision occurs
- THEN Clankers MUST use the Rust-native planner
- AND it MUST emit no claim that Steel authored the decision

### Requirement: Rollout evidence precedes default expansion [r[steel-default-orchestration.rollout-evidence]]
Before Steel becomes default for additional planning seams, Clankers MUST collect comparison evidence between Steel planner output and Rust-native planner output for the reviewed seam, including plan hashes, decision class, authorized effect summary, denial summary, and fallback status.

#### Scenario: defaulting current seam requires current evidence [r[steel-default-orchestration.rollout-evidence.default-current-seam]]
- GIVEN `steel.host.plan_turn` is already wired, documented, and supported by comparison/default tests
- WHEN Clankers changes missing config from disabled to bundled default
- THEN validation MUST include a real-turn smoke proving default activation reaches the existing adapter and emits redacted Steel planning receipt evidence
- AND validation MUST include an explicit-disabled smoke proving the opt-out remains Rust-native

#### Scenario: expansion still requires reviewed profile update [r[steel-default-orchestration.rollout-evidence.reviewed-expansion]]
- GIVEN a new planning seam is proposed for Steel default orchestration
- WHEN the profile is updated
- THEN the update MUST include reviewed policy, fixtures, fallback behavior, and receipt evidence for that seam
- AND it MUST NOT inherit authority from `steel.host.plan_turn` implicitly
