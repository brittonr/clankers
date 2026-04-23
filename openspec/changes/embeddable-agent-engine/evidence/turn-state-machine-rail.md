Artifact-Type: verification-note
Evidence-ID: turn-state-machine-rail
Task-ID: V3
Covers: embeddable.agent.engine.embeddingfocused.migrationrails.engineturnstatemachinerailscoverpositiveandnegativepaths, embeddable.agent.engine.turnorchestration.prompttomodeltotooltocontinuationflowisengineowned, embeddable.agent.engine.turnorchestration.stopreasonsandcontinuationdecisionsareengineowned, embeddable.agent.engine.turnorchestration.toolresultingestionisengineowned

## Summary
Define positive and negative deterministic state-machine rails for the reusable turn slice that will migrate from `clankers-agent::turn` into `clankers-engine`.

## Evidence
- Existing turn-policy reference implementation: `crates/clankers-agent/src/turn/mod.rs`
- Existing retry/cancellation positive+negative rails already present in `crates/clankers-agent/src/turn/mod.rs`
- Existing execution entrypoint from host shell: `turn::run_turn_loop(...)`

## Positive Cases
- prompt submission schedules the first model-request effect
- model completion with plain assistant output terminates the turn cleanly
- model completion with `tool_use` schedules one or more tool-execution effects
- successful tool results trigger continuation model-request planning
- retryable model failure retries until success within configured retry budget
- stable session/correlation metadata survives later turns and resume paths

## Negative Cases
- non-retryable model failure stops immediately without retry
- cancellation during retry backoff terminates the turn as cancelled
- tool calls present with a non-`tool_use` stop reason do not execute tools implicitly
- wrong or stale correlation IDs must be rejected once correlation moves into explicit engine inputs
- duplicate or out-of-order tool/model completion feedback must leave prior valid state unchanged
- token-limit stop behavior must not be re-derived differently in host shells

## Existing Session Evidence
- `crates/clankers-agent/src/turn/mod.rs` already includes deterministic tests for retry recovery, non-retryable failure, and cancellation during backoff.
- Those tests demonstrate the shape of the engine rail: positive and negative state-machine coverage anchored on one reusable turn loop.

## Planned Commands
- `cargo test -p clankers-agent turn_retry_recovers_on_second_attempt --lib`
- `cargo test -p clankers-agent turn_retry_non_retryable_error_skips_retry --lib`
- `cargo test -p clankers-agent turn_retry_cancellation_during_backoff --lib`
- future engine crate rail: `cargo test -p clankers-engine --test turn_state_machine`
