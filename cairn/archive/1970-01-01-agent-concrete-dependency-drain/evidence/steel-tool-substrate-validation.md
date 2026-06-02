# Steel tool substrate adapter validation evidence

Evidence-ID: agent-steel-tool-substrate-validation
Artifact-Type: command-output-summary
Task-ID: V1
Covers: agent-concrete-dependency-drain.neutral-ports,agent-concrete-dependency-drain.verification
Date: 2026-06-02
Status: PASS

## Commands

```text
TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= cargo nextest run -p clankers-agent steel_tool_substrate
TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= cargo check -p clankers-agent --tests
```

## Relevant output

```text
cargo nextest run -p clankers-agent steel_tool_substrate
PASS clankers-agent tests::steel_tool_substrate_settings_adapter_preserves_config_policy_at_agent_edge
PASS clankers-agent turn::execution::tests::steel_tool_substrate_blocks_before_direct_tool_execution
PASS clankers-agent turn::steel_tool_substrate::tests::default_settings_enable_all_executor_kinds
PASS clankers-agent turn::steel_tool_substrate::tests::disabled_executor_is_removed_from_profile
Summary: 4 tests run: 4 passed, 191 skipped

cargo check -p clankers-agent --tests
Finished `dev` profile [optimized + debuginfo]
```

## Coverage notes

`turn/steel_tool_substrate.rs` now owns neutral `AgentToolSteelSubstrateSettings` / rollout / fallback DTOs. `src/lib.rs` is the app-edge adapter that translates `clankers_config::settings::SteelToolSubstrateSettings` into the neutral DTO before activation. Focused tests prove config policy fields survive the adapter and that tool execution still blocks through the Steel substrate seam.
