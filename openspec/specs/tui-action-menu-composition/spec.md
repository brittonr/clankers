# TUI Action/Menu Composition Specification

## Purpose

Defines reusable TUI action/menu composition contracts for typed action parsing, leader-menu contribution inventory, deterministic priority conflict resolution, hide rules, and safe app-edge boundaries.

## Requirements

### Requirement: TUI action/menu kit validates contribution inventory [r[tui-action-menu-composition.tui-action-menu-kit]]

The system MUST define `tui-action-menu-kit` as a composable Clankers brick with explicit ownership boundaries, deterministic fixtures, and safe evidence.

#### Scenario: Brick boundary is explicit [r[tui-action-menu-composition.tui-action-menu-kit.boundary]]

- GIVEN a product or contributor adopts the `tui-action-menu-kit` brick
- WHEN the brick is documented, instantiated, or validated
- THEN the contract MUST name typed `Action` / `ExtendedAction` parsing and `MenuContributor` / `MenuContribution` inventory as reusable behavior
- THEN rendering, key event loops, daemon attachment, provider discovery, plugin supervision, and runtime side effects MUST remain product-owned app-edge behavior unless a future design explicitly promotes them
- THEN the brick MUST NOT silently depend on ambient credentials, daemon sessions, TUI state, provider discovery, plugin supervision, Matrix, iroh, or global singleton runtime state unless the design explicitly labels that path as app-edge

#### Scenario: Typed actions and contribution inventory have executable evidence [r[tui-action-menu-composition.tui-action-menu-kit.conflict-resolution]]

- GIVEN typed TUI actions and leader-menu contributions are changed
- WHEN the focused verification for `tui-action-menu-kit` runs
- THEN it MUST exercise at least one positive typed action parse or dispatch path
- THEN it MUST exercise deterministic priority conflict resolution and expose winner/loser source diagnostics
- THEN evidence MUST be safe to commit or summarize without raw prompts, credentials, authorization headers, OAuth tokens, provider payloads, hidden context, raw tool arguments, or secret environment values

#### Scenario: Hidden-menu and unknown-action paths fail closed [r[tui-action-menu-composition.tui-action-menu-kit.hidden-menu]]

- GIVEN a user or product supplies a hidden-menu rule or an unknown action name
- WHEN the leader-menu inventory is built or the action name is parsed
- THEN hidden entries MUST be excluded from the generated menu inventory
- THEN unknown action names MUST return no typed action instead of falling through to stringly dispatch
- THEN the focused fixture MUST include at least one hidden-menu or unknown-action negative path

#### Scenario: Brick drift is diagnosable [r[tui-action-menu-composition.tui-action-menu-kit.drift]]

- GIVEN source code, docs, fixtures, policy, or generated inventories drift apart
- WHEN `scripts/check-tui-action-menu-kit.rs` runs
- THEN it MUST fail with a diagnostic that names the stale artifact and the expected owner of the update
- THEN intentional contract changes MUST require updating tests, docs, and OpenSpec evidence together
