# Proposal: Steel Execute Turn Authority

## Why

The default Steel planner can now select `executor=SteelScheme`, and the adapter emits an execution receipt after the Rust host runner returns. That receipt proves the adapter ran, but the execution branch itself still needs an explicit pre-run authority contract so default Steel execution cannot rely only on the planning grant.

## What Changes

- Add a reviewed `steel.host.execute_turn` host-action entry to the default Steel orchestration profile with its own `turn-execution` session capability and `clankers/steel/orchestrate.execute_turn` UCAN ability.
- Add a runtime-owned `SteelTurnExecutionInput` DTO plus `SteelTurnExecutionReceipt` authorization result for the selected execution adapter.
- Require the Steel-selected execution adapter to authorize `steel.host.execute_turn` before calling the Rust host runner; denial emits a redacted daemon-visible receipt and blocks before provider/tool effects.
- Extend focused runtime, turn-loop, embedded-controller smoke, docs, and checker evidence for allowed and denied execution authority.

## Impact

- **Files**: `crates/clankers-runtime/src/steel_orchestration.rs`, `crates/clankers-agent/src/turn/{steel_planning.rs,steel_execution.rs,mod.rs}`, Steel default profile JSON/Nickel artifacts, config defaults, smoke tests, docs, and a deterministic checker.
- **Testing**: focused runtime authority tests, real turn-loop selected-executor test, embedded controller Steel smoke, static checker receipts, and Cairn gates/validation.
