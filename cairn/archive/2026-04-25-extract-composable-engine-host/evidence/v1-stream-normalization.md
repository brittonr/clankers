Task-ID: V1
Covers: embeddable-agent-engine.reusable-stream-accumulator, embeddable-agent-engine.stream-folding-positive, embeddable-agent-engine.stream-folding-negative
Artifact-Type: validation-evidence

# V1 stream normalization evidence

## Test paths

- `crates/clankers-engine-host/src/stream.rs` unit tests:
  - `folds_text_thinking_tool_usage_model_and_stop`
  - `rejects_malformed_tool_json`
  - `rejects_non_object_tool_json`
  - `rejects_delta_before_start`
  - `rejects_duplicate_index`
  - `rejects_late_delta_after_stop`
  - `preserves_provider_error_status_and_retryability`
  - `usage_only_and_empty_stop_normalize`
- `crates/clankers-agent/src/turn/execution.rs` unit test:
  - `provider_stream_normalizer_feeds_host_accumulator`

## Commands

- `cargo test -p clankers-agent --lib turn:: && cargo test -p clankers-engine-host --lib`: PASS (49 agent turn tests, 18 host tests when recorded).

## Result

Provider-native `StreamEvent` data is normalized to provider-neutral `HostStreamEvent` data and folded by `StreamAccumulator`; malformed stream inputs fail deterministically and provider errors preserve retryability/status.
