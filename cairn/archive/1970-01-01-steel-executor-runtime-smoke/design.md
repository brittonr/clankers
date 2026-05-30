# Design: Steel Executor Runtime Smoke

## Context

`tests/embedded_controller.rs` already drives a real `SessionController` with `SessionCommand::Prompt`, a counting provider, and Steel turn-planning settings. The smoke drains `DaemonEvent` output and checks redacted `steel.host.plan_turn` receipts.

After the Steel-selected execution seam landed, receipt text includes `executor={:?}`. The runtime smoke should assert that field at the controller boundary, not only inside `run_turn_loop` unit tests.

## Design

- Keep the existing deterministic embedded-controller smoke harness.
- In the explicit comparison-profile smoke, assert the daemon-visible receipt contains `executor=RustNative`.
- In the default-settings smoke, assert the daemon-visible receipt contains `executor=SteelScheme`.
- Keep existing provider-call counts to prove provider effects still run through Rust-owned host paths and invalid activation fails before provider calls.
- Update `scripts/check-steel-turn-planning-runtime-smoke.rs` required markers and receipt guidance to include executor evidence.
- Update `docs/src/reference/steel-turn-planning-runtime-smoke.md` to document the daemon-visible executor assertions.

## Safety

The smoke observes receipt text only. It does not grant Steel any new authority, does not expose raw prompt/script/provider data, and does not change runtime execution behavior.
