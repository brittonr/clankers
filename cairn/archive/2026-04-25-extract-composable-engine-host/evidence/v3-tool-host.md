Task-ID: V3a
Covers: embeddable-agent-engine.reusable-tool-host, embeddable-agent-engine.tool-host-catalog, embeddable-agent-engine.tool-host-outcome-verification
Artifact-Type: validation-evidence

# V3 tool-host contract evidence

## Test paths

- `crates/clankers-tool-host/src/lib.rs` unit tests:
  - `catalog_lists_metadata_and_checks_lookup`
  - `capability_checker_allows_and_denies`
  - `hook_ordering_is_explicit`
  - `outcome_variants_are_explicit`
  - `accumulator_keeps_short_output`
  - `accumulator_truncates_by_utf8_boundary`
  - `accumulator_truncates_by_line_boundary`
  - `accumulator_rejects_zero_byte_limit`
  - `accumulator_rejects_zero_line_limit`

## Commands

- `cargo test -p clankers-tool-host --lib`: PASS (10 tests).

## Result

Tool catalog metadata/listing, capability decisions, hook ordering, explicit outcome variants, output accumulation, UTF-8-safe byte truncation, line truncation, and invalid limit rejection are covered.
