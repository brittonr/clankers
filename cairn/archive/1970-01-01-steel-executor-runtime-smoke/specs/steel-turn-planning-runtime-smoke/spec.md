# Steel Turn Planning Runtime Smoke Delta

## Purpose

Extends the Steel turn-planning runtime smoke so a real controller prompt path proves daemon-visible executor selection, not only planner authorization.

## Requirements

### Requirement: Runtime smoke exposes selected executor [r[steel-executor-runtime-smoke.executor-visible]]

Clankers MUST prove that real `SessionCommand::Prompt` handling emits daemon/session-visible Steel receipt text that distinguishes Rust-native comparison execution from Steel-selected default execution.

#### Scenario: comparison smoke records Rust-native executor [r[steel-executor-runtime-smoke.executor-visible.comparison]]
- GIVEN a session controller is built with reviewed Steel turn planning in comparison rollout mode
- WHEN a prompt command is handled
- THEN the daemon-visible `steel.host.plan_turn` receipt MUST include `executor=RustNative`
- AND the provider call MUST still run through the Rust-owned provider path

#### Scenario: default smoke records Steel executor [r[steel-executor-runtime-smoke.executor-visible.default]]
- GIVEN a session controller is built with default reviewed Steel turn-planning settings
- WHEN a prompt command is handled
- THEN the daemon-visible `steel.host.plan_turn` receipt MUST include `executor=SteelScheme`
- AND the receipt MUST omit raw prompts, raw scripts, credentials, UCAN proofs, and provider payloads
