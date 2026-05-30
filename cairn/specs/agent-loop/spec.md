# Agent Loop Specification

## Purpose

Defines agent-loop requirements for honoring authorized Steel turn-planning executor selection in the real core agent turn loop while preserving Rust-owned fallback, blocking, host-effect execution, and deterministic receipts.

## Requirements

### Requirement: Authorized Steel plans select the Steel execution seam [r[steel-core-agent-loop.executor-selection]]

When Steel turn planning returns an authorized default-mode plan, the core agent turn loop MUST execute the turn through the Steel-selected execution seam instead of silently falling through to the Rust-native runner path.

#### Scenario: default planner selects Steel executor [r[steel-core-agent-loop.executor-selection.default]]
- GIVEN reviewed Steel turn planning is enabled in default rollout mode
- AND Rust validates the plan, profile, script, policy, session capability, UCAN ability, disabled action, budget, and receipt destination
- WHEN the real `run_turn_loop` executes a turn
- THEN it MUST route that turn through the Steel-selected execution seam
- AND the emitted Steel planning receipt MUST record `executor=SteelScheme`

#### Scenario: comparison mode preserves Rust executor [r[steel-core-agent-loop.executor-selection.comparison]]
- GIVEN reviewed Steel turn planning is enabled in comparison mode
- WHEN the real `run_turn_loop` executes a turn
- THEN it MUST keep the Rust-native execution path selected
- AND the emitted Steel planning receipt MUST record `executor=RustNative`

### Requirement: Fail-closed behavior stays before host effects [r[steel-core-agent-loop.fail-closed]]

When Steel planning policy selects block-on-failure, malformed or denied Steel planning MUST block the turn before any provider request or tool effect.

#### Scenario: blocked planner returns before provider call [r[steel-core-agent-loop.fail-closed.before-provider]]
- GIVEN Steel planning cannot produce an authorized executable plan
- AND fallback mode is block
- WHEN the real `run_turn_loop` evaluates the planning result
- THEN it MUST return a blocked turn result before provider/tool effects
- AND the receipt MUST identify the blocked planning status without leaking raw prompt, provider payload, credential, UCAN proof, script source, or tool body

### Requirement: Steel-selected execution does not grant ambient authority [r[steel-core-agent-loop.no-ambient-authority]]

The Steel-selected execution seam MUST NOT give Steel Scheme ambient filesystem, shell, git, network, provider, credential, daemon, TUI, native-tool, session, mutation, or direct tool-execution authority. Provider and tool effects MUST continue to execute through Rust-owned typed host adapters.

#### Scenario: execution seam delegates host effects through Rust [r[steel-core-agent-loop.no-ambient-authority.host-effects]]
- GIVEN an authorized Steel-selected default plan
- WHEN the Steel execution seam runs the turn
- THEN provider/tool effects MUST still pass through the existing Rust reducer/host adapter path
- AND interpreter internals MUST remain out of controller, daemon, TUI, provider, and tool-host shells

### Requirement: Deterministic receipt evidence covers selected executor [r[steel-core-agent-loop.receipts]]

The Steel planning receipt MUST include enough deterministic, redacted evidence to distinguish Steel-selected execution from Rust-native comparison/fallback execution.

#### Scenario: receipt records executor without secrets [r[steel-core-agent-loop.receipts.executor]]
- GIVEN Steel planning emits a receipt for a real turn
- WHEN the receipt is converted to the session-visible system message
- THEN it MUST include the selected executor
- AND it MUST preserve existing redaction guarantees for prompts, provider payloads, credentials, UCAN proofs, raw script source, tool bodies, and secret paths
