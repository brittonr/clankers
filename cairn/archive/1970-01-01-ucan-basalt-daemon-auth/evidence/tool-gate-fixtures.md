Evidence-ID: tool-gate-fixtures
Artifact-Type: test-report
Task-ID: V4
Covers: r[ucan-basalt-daemon-auth.tool-gate.call-time], r[ucan-basalt-daemon-auth.vocabulary.operation-matrix], r[ucan-basalt-daemon-auth.verification.tool-gate]
Created: 2026-05-29
Status: complete

# Tool Gate Fixture Verification

## Scope

Deterministic tests cover the call-time public UCAN + Basalt gate for protected operations.

Covered classes:

- prompt admission via `session/prompt`, including transport-identity-bound resources for Matrix/chat keyed sessions
- session management via `session/manage`
- generic `tool/use` requests for each tool name
- file read and write paths, including write denial under read-only grants
- bash/shell execution by concrete working directory
- process observe/log/start/mutate/stdin/backend-selection abilities
- model switching via `switch_model` and protocol model switch requiring `model/use`
- concrete request construction for write/edit-style file, prompt, session, and model operations
- UCAN/Basalt denial before bash human confirmation can run or bypass it
- controller denial for prompt, model, and session-management commands before state/history mutation

## Machine Evidence

Commands run:

```text
TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= CC=gcc CXX=g++ CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_LINKER=gcc RUSTFLAGS='-C link-arg=-fuse-ld=bfd' cargo test --lib capability_gate
TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= CC=gcc CXX=g++ CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_LINKER=gcc RUSTFLAGS='-C link-arg=-fuse-ld=bfd' cargo test -p clankers-controller capability_gate --lib
```

Result excerpts:

```text
running 29 tests
test capability_gate::tests::public_session_and_model_requests_are_concrete ... ok
test capability_gate::tests::public_ucan_gate_can_bind_prompt_checks_to_transport_identity ... ok
test capability_gate::tests::public_ucan_gate_requires_session_prompt_capability ... ok
test capability_gate::tests::public_ucan_gate_requires_session_manage_capability ... ok
test capability_gate::tests::public_ucan_gate_requires_model_use_for_protocol_model_switch ... ok
test capability_gate::tests::public_ucan_gate_maps_model_switch_to_model_use ... ok
test capability_gate::tests::public_ucan_denial_happens_before_bash_confirmation_can_bypass_it ... ok

test result: ok. 29 passed; 0 failed

running 3 tests
test command::tests::prompt_is_denied_by_capability_gate_before_history_mutation ... ok
test command::tests::session_manage_command_is_denied_by_capability_gate_before_mutation ... ok
test command::tests::set_model_is_denied_by_capability_gate ... ok

test result: ok. 3 passed; 0 failed
```
