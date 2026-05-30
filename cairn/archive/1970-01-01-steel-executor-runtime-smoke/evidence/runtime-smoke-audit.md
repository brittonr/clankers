Evidence-ID: runtime-smoke-audit
Artifact-Type: investigation-note
Task-ID: R1
Covers: r[steel-executor-runtime-smoke.executor-visible]
Created: 2026-05-30
Status: complete

# Runtime Smoke Audit

## Findings

- `tests/embedded_controller.rs` already drives real `SessionController::handle_command(SessionCommand::Prompt { ... })` paths for Steel turn planning.
- The smoke drains `DaemonEvent` output and filters `DaemonEvent::SystemMessage` values containing `steel.host.plan_turn receipt`.
- Existing positive smoke assertions checked `status=Authorized`, `planner=SteelScheme`, fallback state, provider call counts, and redaction.
- They did not check the new `executor=...` receipt field, so the controller boundary could regress even if unit-level turn-loop tests still passed.
- `scripts/check-steel-turn-planning-runtime-smoke.rs` also lacked required markers for `executor=RustNative` and `executor=SteelScheme`.

## Conclusion

The deterministic next rail is to extend the existing embedded controller smoke and checker with executor assertions while preserving the same Rust-owned provider-effect and redaction checks.
