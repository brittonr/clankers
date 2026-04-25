Task-ID: V1,V2,V3,V4,V5,V6
Covers: embeddable-agent-engine.reusable-stream-accumulator, embeddable-agent-engine.stream-folding-positive, embeddable-agent-engine.stream-folding-negative, embeddable-agent-engine.host-runner-traits, embeddable-agent-engine.tool-host-outcome-verification, embeddable-agent-engine.cancellation-phase-ownership, embeddable-agent-engine.reusable-tool-host, embeddable-agent-engine.tool-host-catalog, embeddable-agent-engine.plugin-tool-adapter, embeddable-agent-engine.agent-default-assembly, embeddable-agent-engine.host-adapter-parity, embeddable-agent-engine.adapter-parity-rails, embeddable-agent-engine.reducer-retry-tests, embeddable-agent-engine.reducer-budget-token-tests, embeddable-agent-engine.invalid-retry-feedback, embeddable-agent-engine.host-extraction-rails, embeddable-agent-engine.no-duplicated-runner-policy, embeddable-agent-engine.host-crate-boundary-rails, embeddable-agent-engine.core-engine-boundary-rails, embeddable-agent-engine.engine-state-active-data, embeddable-agent-engine.host-artifact-refresh, embeddable-agent-engine.composable-host-contract, embeddable-agent-engine.host-artifact-freshness
Artifact-Type: validation-plan

# Validation Plan

## Prerequisite gates

- `./scripts/check-llm-contract-boundary.sh`
- `cargo test -p clankers-controller --test fcis_shell_boundaries`
- `openspec validate extract-composable-engine-host --strict`

## Focused test bundles

- `cargo test -p clankers-engine-host`
- `cargo test -p clankers-tool-host`
- `cargo test -p clankers-agent host_runner`
- `cargo test -p clankers-agent stream_accumulator`
- `cargo test -p clankers-agent tool_host`
- `cargo test -p clankers-controller --test fcis_shell_boundaries`

## Artifact freshness

- `unit2nix --workspace --force --no-check -o build-plan.json`
- `cargo xtask docs`
- Check `Cargo.toml`, `Cargo.lock`, `flake.nix`, `build-plan.json`, and generated docs contain `clankers-engine-host` and `clankers-tool-host`.

## Final acceptance

Record exact command output before marking V6 complete.

## Recorded focused results

### V1 stream accumulator and provider-normalizer seam

- `cargo test -p clankers-agent --lib turn:: && cargo test -p clankers-engine-host --lib`: PASS.
- Coverage: host stream accumulator positive text/thinking/tool JSON/usage/model/stop cases; negative malformed JSON, non-object JSON, missing starts, duplicate indexes, late deltas, provider errors preserving status/retryability, usage-only and empty stops; provider stream normalizer feeds provider-native `StreamEvent` data into `HostStreamEvent` and `StreamAccumulator`.

### V2 host runner diagnostics and tool outcomes

- `cargo test -p clankers-engine-host --lib`: PASS (22 tests).
- Coverage added: retryable model failure sleeps before `RetryReady`, stream malformed-input matrix maps to correlated non-retryable reducer model failures, event-sink and usage-observer failures are report diagnostics only, and all `ToolHostOutcome` variants map through correlated engine feedback/cancellation without reducer rejection.
