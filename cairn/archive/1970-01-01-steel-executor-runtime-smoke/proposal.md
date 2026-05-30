# Proposal: Steel Executor Runtime Smoke

## Why

The core turn-loop unit tests prove `AgentTurnExecutionPlanner::SteelScheme` calls the Steel-selected execution adapter, but the controller/runtime smoke rail only asserted an authorized Steel planning receipt. It did not prove that a real `SessionCommand::Prompt` path exposes the selected executor in daemon-visible events.

## What Changes

- Extend the embedded controller Steel runtime smoke to assert executor selection in receipt events.
- Require comparison mode to surface `executor=RustNative`.
- Require default settings to surface `executor=SteelScheme`.
- Update the runtime-smoke checker and docs so future drift cannot drop the executor evidence.

## Non-Goals

- Do not move provider/tool effects into the Steel interpreter.
- Do not change Steel policy, UCAN, or fallback semantics.
- Do not add live daemon networking coverage; this is a deterministic embedded controller smoke.
