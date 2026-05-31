# Slash effect boundary rail evidence

Evidence-ID: slash-effect-boundary-rails
Artifact-Type: command-output-summary
Task-ID: V6
Covers: coupling-hotspot-remediation.slash-effect-boundary
Date: 2026-05-31
Status: PASS

## Commands

```text
./scripts/check-slash-effect-boundary.rs
CC=gcc CXX=g++ CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_LINKER=gcc RUSTFLAGS='-C link-arg=-fuse-ld=bfd' TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= cargo test -p clankers --lib slash_commands::effects
CC=gcc CXX=g++ CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_LINKER=gcc RUSTFLAGS='-C link-arg=-fuse-ld=bfd' TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= cargo test -p clankers --lib attach_think
CC=gcc CXX=g++ CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_LINKER=gcc RUSTFLAGS='-C link-arg=-fuse-ld=bfd' TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= cargo test -p clankers --lib attach_plugin_route_requests_plugin_inventory
```

## Relevant output

```text
ok: slash effect boundary rail passed

running 2 tests
test slash_commands::effects::tests::attach_effects_cover_ui_plugin_session_forward_and_noop_shapes ... ok
test slash_commands::effects::tests::standalone_interpreter_applies_ui_session_and_noop_effects ... ok

test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 1030 filtered out

running 4 tests
test modes::attach::tests::mcp_thinking_level_command_matches_attach_think_command ... ok
test modes::attach::tests::attach_think_cycle_bridge_updates_local_state_and_emits_cycle_command ... ok
test modes::attach::tests::attach_think_matches_standalone_after_daemon_roundtrip ... ok
test modes::attach::tests::attach_think_cycle_matches_standalone_after_daemon_roundtrip ... ok

test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 1028 filtered out

running 1 test
test modes::attach::tests::attach_plugin_route_requests_plugin_inventory ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 1031 filtered out
```

## Coverage notes

The static rail requires `src/slash_commands/effects.rs` to define declarative UI, session, plugin, forward, and no-op effects plus a standalone interpreter. It also requires attach slash dispatch to route client-local commands, plugin inventory fetches, daemon-forwarded slash commands, and registry-emitted `AgentCommand`s through the effect constructors/interpreter instead of open-coded transport policy. Existing shared session command policy remains the source of thinking, disabled-tool, and manual-compaction local effects plus daemon ack expectations.

The effect tests cover a UI-only help effect, session-command thinking effect, plugin inventory effect, daemon-forward effect, and deterministic no-op/fail-closed shape; the standalone interpreter test applies UI/session/no-op effects to an `App` and command channel. The attach parity tests prove the effect path still preserves `/think` standalone/daemon behavior and `/plugin` sends `GetPlugins`.
