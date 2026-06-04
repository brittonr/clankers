Task-ID: V3c
Covers: embeddable-agent-engine.plugin-tool-adapter, embeddable-agent-engine.host-crate-boundary-rails
Artifact-Type: validation-evidence

# V3c mixed tool adapter evidence

## Test paths

- `crates/clankers-agent/src/turn/execution.rs` unit test:
  - `built_in_wasm_and_stdio_tools_share_executor_seam`
- `crates/clankers-controller/tests/fcis_shell_boundaries.rs` tests:
  - `host_crates_reject_shell_runtime_source_leakage`
  - `tool_host_rejects_engine_reducer_internal_source_leakage`

## Commands

- `cargo test -p clankers-agent --lib turn::execution`: PASS (8 execution tests).
- `cargo test -p clankers-controller --test fcis_shell_boundaries`: PASS (32 tests, recorded in V5 host rail evidence).

## Result

Built-in, WASM-plugin-labeled, and stdio-plugin-labeled tools flow through the same agent `ToolExecutor` adapter seam. Generic host crates do not import plugin supervision/runtime crates.
