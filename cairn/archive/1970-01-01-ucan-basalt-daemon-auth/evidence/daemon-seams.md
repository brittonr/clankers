Evidence-ID: daemon-seams
Artifact-Type: test-report
Task-ID: V3
Covers: r[ucan-basalt-daemon-auth.daemon-seams.remote-entrypoints], r[ucan-basalt-daemon-auth.migration.fail-closed], r[ucan-basalt-daemon-auth.verification.daemon-seams]
Created: 2026-05-29
Status: complete

# Daemon Seam Auth Verification

## Scope

Deterministic daemon seam tests cover shared public UCAN + Basalt admission used by remote entrypoints:

- QUIC control create via `session_create_admission_request()`
- QUIC attach via `session_attach_admission_request(session_id)`
- chat/RPC auth frames and Matrix/keyed-session prompt admission via `session_prompt_admission_request(id)`
- `AuthLayer::verify_credential_base64(...)` for shared decode + verification
- `AuthLayer::resolve_credential(...)` for stored Matrix credential lookup without re-consuming a registration replay id

Covered credential cases include valid, malformed, legacy, expired, not-before, revoked, wrong-audience, untrusted-root, and policy-denied credentials.

## Machine Evidence

Commands run:

```text
TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= CC=gcc CXX=g++ CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_LINKER=gcc RUSTFLAGS='-C link-arg=-fuse-ld=bfd' cargo test --test auth_credential
TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= CC=gcc CXX=g++ CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_LINKER=gcc RUSTFLAGS='-C link-arg=-fuse-ld=bfd' cargo test --test public_ucan_boundary
```

Result excerpts:

```text
running 10 tests
test remote_entrypoint_requests_match_public_ucan_vocabulary ... ok
test quic_create_attach_and_chat_rpc_prompt_requests_accept_valid_public_ucan ... ok
test remote_auth_rejects_malformed_and_legacy_base64 ... ok
test remote_auth_rejects_expired_not_before_wrong_audience_and_policy_denied_credentials ... ok
test matrix_stored_credential_resolution_does_not_reconsume_replay_id ... ok
test revoked_public_reference_is_rejected ... ok
test untrusted_public_root_rejected ... ok

test result: ok. 10 passed; 0 failed

running 3 tests
test daemon_entrypoints_use_shared_session_admission_request_helpers ... ok
test daemon_auth_defaults_to_public_ucan_without_legacy_verifier ... ok
test public_ucan_dependency_uses_remote_workspace_pin ... ok

test result: ok. 3 passed; 0 failed
```
