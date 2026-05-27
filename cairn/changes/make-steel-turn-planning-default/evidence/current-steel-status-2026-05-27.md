# Current Steel status snapshot

Evidence-ID: current-steel-status-2026-05-27
Artifact-Type: source-status-snapshot
Task-ID: S1
Covers: steel-turn-planning-config-activation.settings-surface.absent-default, steel-default-orchestration.policy-selected-default.default-selected
Date: 2026-05-27
Status: captured

## Observed source state

- `docs/src/reference/steel-turn-planning-config-activation.md` states the current default remains disabled when settings omit Steel turn planning.
- `crates/clankers-config/src/settings.rs` defines `SteelTurnPlanningSettings::default()` with `enabled: false`.
- `docs/src/reference/steel-agent-turn-wiring.md` states real agent turn wiring can call `steel.host.plan_turn` and that Rust remains provider/tool/effect authority.
- `policy/steel-default-orchestration/orchestration-profile.json` is checked in with `rollout_stage: "default"`, `fallback_mode: "rust_native"`, and one allowed host action: `steel.host.plan_turn`.
- `policy/steel-default-orchestration/scripts/default-plan-turn.scm` is checked in and calls `(host "steel.host.plan_turn")`.

## Boundary

This snapshot contains file-path and behavior summaries only. It does not include prompts, credentials, provider payloads, tokens, or private session transcripts.
