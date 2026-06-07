# Framed session transport evidence

Evidence-ID: trait-seam-refactor-roadmap.session-transport-trait
Artifact-Type: command-output-summary
Task-ID: V3
Covers: remaining-coupling-drain.trait-seam-refactors.session-transport
Date: 2026-06-06
Status: PASS

## Implementation summary

- Kept `clankers-protocol::frame` as the shared length-prefixed JSON frame policy for both Unix socket and QUIC transports.
- Removed duplicate QUIC-specific JSON frame read/write helpers from `src/modes/attach_remote.rs`.
- Reused `QuicBiStream` as the QUIC `AsyncRead + AsyncWrite` adapter so remote attach/control handshakes call `frame::write_frame` and `frame::read_frame` directly.
- Left wire DTO construction in existing protocol/transport conversion owners; FCIS transport-construction rail stayed green.

## Commands completed

```text
env TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= CC=gcc CXX=g++ CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_LINKER=gcc RUSTFLAGS='-C link-arg=-fuse-ld=bfd' cargo test --lib local_reconnect_resets_parity_tracker_before_new_events_arrive
env TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= CC=gcc CXX=g++ CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_LINKER=gcc RUSTFLAGS='-C link-arg=-fuse-ld=bfd' cargo test --lib remote_reconnect_resets_parity_tracker_before_new_events_arrive
env TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= cargo test -p clankers-controller --test fcis_shell_boundaries
```

## Relevant output

```text
running 1 test
test modes::attach::client_loop::tests::local_reconnect_resets_parity_tracker_before_new_events_arrive ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 1048 filtered out; finished in 0.02s
exit=0

running 1 test
test modes::attach_remote::tests::remote_reconnect_resets_parity_tracker_before_new_events_arrive ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 1048 filtered out; finished in 0.04s
exit=0

running 44 tests
...
test result: ok. 44 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.75s
exit=0
```
