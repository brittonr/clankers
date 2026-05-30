# Proposal: Steel Execute Turn Host Call

## Summary

Make `steel.host.execute_turn` an explicit Steel runtime host-call contract before the Rust host runner executes provider/tool effects. The previous slice added separate execution authority; this slice proves the Steel-selected execution path is requested through the constrained Steel runtime wrapper, produces a redacted host-call receipt, and still requires Rust dynamic-runtime authorization before effects run.

## Motivation

Default Steel planning can now select the Steel execution adapter, and Rust authorizes `steel.host.execute_turn` before calling the host runner. The remaining gap is that execution is still represented as a Rust-selected adapter receipt rather than a Steel runtime host-call request/response. A dedicated host-call contract gives daemon observers and tests a stable proof that Steel requested execution through the reviewed runtime wrapper and that Rust approved or denied it before provider work.

## Scope

- Add a metadata-only Steel execution host-call payload and receipt under runtime-owned types.
- Evaluate `(host "steel.host.execute_turn")` through the constrained Steel runtime wrapper before dynamic-runtime execution authorization.
- Include host-call status/reason/hash in daemon-visible execution receipts while preserving redaction.
- Add deterministic runtime, turn-loop, embedded smoke, docs, and checker evidence.

## Non-Goals

- Replacing the fixture Steel evaluator with a full upstream interpreter.
- Moving provider/tool execution into Steel.
- Expanding the reviewed default profile beyond `plan_turn` and `execute_turn`.
