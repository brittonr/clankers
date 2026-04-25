Task-ID: V5b
Covers: embeddable-agent-engine.host-crate-boundary-rails
Artifact-Type: validation-evidence

# V5b host crate boundary rail evidence

## Test paths

- `scripts/check-llm-contract-boundary.sh`:
  - normal-edge cargo-tree checks for `clankers-engine`, `clankers-engine-host`, `clankers-tool-host`, and `clanker-message`
  - direct normal dependency checks for host crates against the finite denylist
- `crates/clankers-controller/tests/fcis_shell_boundaries.rs` tests:
  - `host_crates_reject_shell_runtime_source_leakage`
  - `host_builtin_tool_path_text_inventory_allows_bare_tool_words`
  - `tool_host_rejects_engine_reducer_internal_source_leakage`
  - `engine_host_rejects_reducer_policy_source_leakage`

## Commands

- `./scripts/check-llm-contract-boundary.sh`: PASS.
- `cargo test -p clankers-controller --test fcis_shell_boundaries`: PASS (33 tests).

## Result

Cargo-tree/direct-dependency rails reject forbidden runtime/provider/UI/DB/network crates for host crates. Source rails reject provider/request/response/timestamp/message-ID leakage, shell-native `AgentMessage`, network/runtime handle tokens, built-in tool/system-prompt path references with anchored matching and bare-word false-positive coverage, external truncation helpers in `clankers-tool-host`, and reducer-internal tool-host tokens.
