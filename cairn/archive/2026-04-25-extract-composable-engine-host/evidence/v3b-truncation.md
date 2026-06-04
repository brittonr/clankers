Task-ID: V3b
Covers: embeddable-agent-engine.tool-host-catalog, embeddable-agent-engine.tool-host-outcome-verification
Artifact-Type: validation-evidence

# V3b truncation evidence

## Test paths

- `crates/clankers-tool-host/src/lib.rs` unit tests:
  - `accumulator_truncates_by_utf8_boundary`
  - `accumulator_truncates_by_line_boundary`
  - `accumulator_rejects_zero_byte_limit`
  - `accumulator_rejects_zero_line_limit`
- `crates/clankers-agent/src/turn/mod.rs` unit test:
  - `output_truncation_preserves_existing_clankers_limit_metadata`

## Commands

- `cargo test -p clankers-tool-host --lib && cargo test -p clankers-agent --lib turn::`: PASS (10 tool-host tests, 50 agent turn tests).

## Result

Tool-host truncation covers adapter-supplied byte/line limits, UTF-8 boundaries, newline-boundary line counting, original/truncated metadata, and invalid limits. The Clankers adapter path still preserves existing `clanker_loop` tool-output truncation footer/details behavior.
