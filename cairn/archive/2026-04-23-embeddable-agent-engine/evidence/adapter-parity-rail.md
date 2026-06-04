Artifact-Type: verification-note
Evidence-ID: adapter-parity-rail
Task-ID: V2
Covers: embeddable.agent.engine.embeddingfocused.migrationrails.engineparityrailscoverhostadapterseams, embeddable.agent.engine.controllerandagentshells.controllerstopsowningreusableorchestrationpolicy, embeddable.agent.engine.controllerandagentshells.agentruntimestopsowningreusableturnpolicy

## Summary
Define deterministic parity rails proving controller and agent shells act as hosts/adapters around engine-directed work rather than keeping a second authoritative copy of reusable turn policy.

## Evidence
- Existing controller boundary/parity reference: `crates/clankers-controller/tests/fcis_shell_boundaries.rs`
- Existing controller validation bundle reference: `scripts/verify-no-std-functional-core.sh`
- Existing agent parity references: tests in `crates/clankers-agent/src/lib.rs` and `crates/clankers-agent/src/turn/mod.rs`
- Existing embedded-shell parity reference: `cargo nextest run --test embedded_controller`

## Checks
- Controller parity rail: assert controller translation files are the only place that turn shell-native commands/results become engine inputs and engine semantic events become daemon/session-facing outputs.
- Agent parity rail: assert runtime files execute model/tool work requested by engine effects and feed correlated results back without owning reusable continuation, retry, or tool-result-ingestion policy locally.
- Embedded/app-shell parity rail: assert standalone/embedded runtime files route engine-selected next actions and surface rejections through shell-native UI paths rather than synthesizing engine decisions locally.
- Correlation parity: assert model request IDs and tool call IDs originate from engine-owned state/effects and are echoed back by adapters, not re-minted in host shells.
- Anti-fork parity: fail if migrated prompt/model/tool continuation rules exist both in engine-owned reducers and in adapter-local branching for the same slice.

## Planned Commands
- `cargo test -p clankers-controller --test fcis_shell_boundaries -- --nocapture`
- `cargo nextest run -p clankers-controller --tests`
- `cargo test -p clankers-agent --lib turn_retry`
- `cargo nextest run --test embedded_controller`

## Current Reference Result
- `cargo test -p clankers-controller --test fcis_shell_boundaries -- --nocapture` passed locally in this session, with 19/19 tests green.
