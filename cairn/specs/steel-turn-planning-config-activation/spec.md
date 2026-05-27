# Steel Turn Planning Config Activation Specification

## Purpose

Defines the `steel-turn-planning-config-activation` capability for selecting the already-wired Steel Scheme `steel.host.plan_turn` agent-turn planner through reviewed runtime configuration while preserving Rust-owned authority, fallback, and receipts.

## Requirements

### Requirement: Real turn call sites use the same activation helper [r[steel-turn-planning-config-activation.turn-threading]]

Normal agent turns and orchestrated/model-role phase turns MUST use the same Rust-owned activation helper so Steel planning config does not drift across real turn paths.

#### Scenario: normal turns receive activated config [r[steel-turn-planning-config-activation.turn-threading.normal]]
- GIVEN Steel turn planning config is valid and enabled
- WHEN a normal prompt reaches the real turn loop
- THEN the constructed `TurnConfig` MUST carry the activated `AgentTurnSteelPlanningConfig`
- AND the existing Steel planning hook MAY emit the deterministic planning receipt

#### Scenario: orchestrated turns receive activated config [r[steel-turn-planning-config-activation.turn-threading.orchestrated]]
- GIVEN model-role orchestration is active and Steel turn planning config is valid
- WHEN each phase reaches the real turn loop
- THEN the constructed `TurnConfig` MUST use the same activation helper
- AND phase execution MUST preserve the same comparison/default/fallback semantics as normal turns

### Requirement: Configuration cannot grant ambient authority [r[steel-turn-planning-config-activation.fail-closed]]

Steel turn planning configuration MUST NOT grant Steel ambient filesystem, shell, git, network, provider, credential, daemon, TUI, native-tool, session mutation, or code mutation authority.

#### Scenario: unsupported authority is denied before execution [r[steel-turn-planning-config-activation.fail-closed.unsupported-authority]]
- GIVEN config/profile/script data requests unsupported host authority or a broader runtime profile than allowed
- WHEN Rust validates activation
- THEN activation MUST fail closed
- AND the turn MUST continue only as disabled/no-Steel planning or return a stable configuration error according to reviewed policy

#### Scenario: rollout mode still controls fallback [r[steel-turn-planning-config-activation.fail-closed.fallback-policy]]
- GIVEN an enabled profile selects comparison or default mode
- WHEN Steel planning later fails, is denied, or is malformed
- THEN existing runtime fallback/blocking policy MUST determine whether Rust-native planning may continue
- AND activation MUST NOT retry Steel with broader budgets, host functions, session grants, or script authority

### Requirement: Activation has deterministic evidence [r[steel-turn-planning-config-activation.verification]]

The implementation MUST include focused tests, docs, and a deterministic checker receipt proving config-selected activation, fail-closed invalid config behavior, real turn threading, and redacted receipt behavior.

#### Scenario: tests cover activation modes [r[steel-turn-planning-config-activation.verification.tests]]
- GIVEN fixture settings/profile data for disabled, comparison, default, and invalid modes
- WHEN focused tests run
- THEN they MUST prove activation maps to the expected `TurnConfig.steel_turn_planning` behavior
- AND they MUST prove comparison remains Rust-native while default requires Rust authorization

#### Scenario: checker writes redacted receipt [r[steel-turn-planning-config-activation.verification.checker]]
- GIVEN the source tree includes the activation implementation
- WHEN the checker runs
- THEN it MUST write a deterministic receipt under `target/steel-turn-planning-config-activation/`
- AND that receipt MUST NOT include raw prompts, provider payloads, credentials, UCAN proofs, raw script bodies, or absolute secret paths

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
