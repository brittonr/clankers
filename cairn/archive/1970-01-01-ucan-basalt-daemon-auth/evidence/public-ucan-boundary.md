Evidence-ID: public-ucan-boundary
Artifact-Type: test-report
Task-ID: V5
Covers: r[ucan-basalt-daemon-auth.public-ucan.dependency-source], r[ucan-basalt-daemon-auth.verification.dependency-boundary]
Created: 2026-05-29
Status: complete

# Public UCAN Dependency / Boundary Verification

## Scope

`tests/public_ucan_boundary.rs` asserts that default daemon auth surfaces no longer construct or decode legacy `clanker-auth` credentials in daemon/session/token paths, that remote session entrypoints share centralized public UCAN/Basalt request helpers, and that `clankers-ucan` consumes the workspace-pinned OnixResearch `ucan` dependency instead of a sibling `../../../ucan` path.

## Machine Evidence

Command run:

```text
TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= CC=gcc CXX=g++ CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_LINKER=gcc RUSTFLAGS='-C link-arg=-fuse-ld=bfd' cargo test --test public_ucan_boundary
```

Result:

```text
running 3 tests
test daemon_auth_defaults_to_public_ucan_without_legacy_verifier ... ok
test daemon_entrypoints_use_shared_session_admission_request_helpers ... ok
test public_ucan_dependency_uses_remote_workspace_pin ... ok

test result: ok. 3 passed; 0 failed
```
