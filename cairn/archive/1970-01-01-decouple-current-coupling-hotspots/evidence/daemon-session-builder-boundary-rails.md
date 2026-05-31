# Daemon session-builder boundary rail evidence

Evidence-ID: daemon-session-builder-boundary-rails
Artifact-Type: command-output-summary
Task-ID: V5
Covers: coupling-hotspot-remediation.daemon-session-builder-boundary
Date: 2026-05-31
Status: PASS

## Commands

```text
./scripts/check-daemon-session-builder-boundary.rs
CC=gcc CXX=g++ CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_LINKER=gcc RUSTFLAGS='-C link-arg=-fuse-ld=bfd' TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= cargo test -p clankers --lib session_builder
```

## Relevant output

```text
ok: daemon session-builder boundary rail passed

running 3 tests
test modes::daemon::session_builder::tests::create_plan_for_new_session_has_spawn_and_handle_data_without_socket ... ok
test modes::daemon::session_builder::tests::create_plan_resolves_resume_messages_without_socket ... ok
test modes::daemon::session_builder::tests::keyed_plans_prepare_new_and_recovered_actor_inputs_without_socket ... ok

test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 1027 filtered out
```

## Coverage notes

The static rail requires `src/modes/daemon/session_builder.rs` to own socketless create/resume/keyed-session plans, actor spawn inputs, daemon `SessionHandle` construction, catalog entry construction, and resume-message loading. It also checks that `socket_bridge.rs` delegates create-session construction to `SessionBuilder` instead of opening session files or constructing transport handles/catalog entries inline, and that keyed chat/Matrix recovery in `agent_process.rs` uses the same builder plan helpers.

The focused root tests exercise a fresh create plan, resume-by-session-id seed-message loading from an Automerge session, and new/recovered keyed-session plans without binding a Unix socket or spawning an actor.
