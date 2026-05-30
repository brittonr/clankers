# Proposal: Steel Executor Run Receipt

## Why

The runtime smoke now proves default Steel planning selects `executor=SteelScheme`, but production observability still depended on the planning receipt. A non-test execution receipt should prove the Steel-selected adapter itself ran and returned from the Rust-owned host runner.

## What Changes

- Emit a redacted `steel.host.execute_turn` receipt from `turn/steel_execution.rs` after the host runner returns.
- Include only safe execution metadata: executor, session hash, model label, result class, host-runner label, safe counts, and receipt hash.
- Extend unit and controller smoke tests to observe the execution receipt for default Steel execution.
- Keep comparison/disabled Rust-native paths free of the Steel-selected execution receipt.

## Non-Goals

- Do not move provider/tool effects into Steel.
- Do not add raw prompt, provider payload, tool body, credential, UCAN proof, or script source to receipts.
- Do not change fallback or block policy.
