## MODIFIED Requirements

### Requirement: Optional settings surface selects Steel turn planning [r[steel-turn-planning-config-activation.settings-surface]]
Clankers MUST provide a stable typed settings/configuration surface that enables the reviewed Steel turn planner by default for supported real agent turns, while preserving an explicit operator opt-out.

#### Scenario: absent config uses bundled default Steel planner [r[steel-turn-planning-config-activation.settings-surface.absent-default]]
- GIVEN no `steelTurnPlanning` block is present in settings
- WHEN Clankers builds a real agent `TurnConfig` for a supported turn-planning decision
- THEN `steel_turn_planning` MUST be derived from the bundled reviewed `steel.host.plan_turn` profile and script
- AND the derived config MUST remain limited to the named turn-planning seam
- AND Rust MUST still validate profile, script, budget, session capability, UCAN ability, disabled action, fallback policy, and receipt destination before Steel can influence the decision

#### Scenario: explicit opt-out keeps Rust-native planning [r[steel-turn-planning-config-activation.settings-surface.explicit-disabled]]
- GIVEN settings set `steelTurnPlanning.enabled = false`
- WHEN Clankers builds a real agent `TurnConfig`
- THEN `steel_turn_planning` MUST remain `None`
- AND no receipt may claim Steel authored the turn planning decision

#### Scenario: explicit config overrides bundled default [r[steel-turn-planning-config-activation.settings-surface.explicit-profile]]
- GIVEN settings name an explicit reviewed Steel turn-planning profile and script
- WHEN Clankers builds a real agent `TurnConfig`
- THEN Rust MUST attempt to load that explicit profile through the same typed activation helper used for the bundled default
- AND the resulting `AgentTurnSteelPlanningConfig` MUST be derived from validated profile data rather than from Steel script self-selection

### Requirement: Rust validates profile and script bindings [r[steel-turn-planning-config-activation.profile-loader]]
Rust MUST validate the Nickel-exported orchestration profile and script binding before any real turn can use Steel planning.

#### Scenario: bundled default is hash-bound and source-controlled [r[steel-turn-planning-config-activation.profile-loader.bundled-default]]
- GIVEN Clankers uses the bundled default Steel planner because settings omit `steelTurnPlanning`
- WHEN Rust activates the planner
- THEN Rust MUST load the checked-in default profile and script from repo policy paths
- AND it MUST compute or verify BLAKE3 hashes for both artifacts before constructing `AgentTurnSteelPlanningConfig`
- AND receipts MUST include redacted profile/script hash evidence without raw script source or prompt material

#### Scenario: invalid bundled default fails closed [r[steel-turn-planning-config-activation.profile-loader.invalid-bundled-default]]
- GIVEN the bundled default profile or script is missing, malformed, hash-mismatched, over budget, unsupported, or points outside allowed policy/script roots
- WHEN Rust activates the default planner
- THEN activation MUST fail before Steel runs
- AND the turn MUST continue only through reviewed Rust-native fallback or return a stable block error according to fallback policy
- AND activation MUST NOT retry Steel with broader budgets, host functions, session grants, script authority, filesystem, shell, git, network, provider, credential, daemon, TUI, native-tool, session mutation, or code mutation authority
