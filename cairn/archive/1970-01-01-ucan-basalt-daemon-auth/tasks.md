# Tasks: UCAN + Basalt Daemon Auth

## Phase 0: Audit

- [x] [serial] R1. Audit current daemon auth, public UCAN adapter, Basalt usage, and dependency sources before implementation. [covers=r[ucan-basalt-daemon-auth.public-ucan], r[ucan-basalt-daemon-auth.basalt-policy], r[ucan-basalt-daemon-auth.daemon-seams], r[ucan-basalt-daemon-auth.tool-gate]] [evidence=evidence/current-auth-audit.md]

## Phase 1: Public UCAN credential substrate

- [x] [serial] I1. Replace the default daemon credential type with a versioned public UCAN envelope carrying OnixResearch `ucan::CompactToken` proofs, audience/root metadata, replay identifiers, and safe decode errors. [covers=r[ucan-basalt-daemon-auth.public-ucan.credential-envelope], r[ucan-basalt-daemon-auth.public-ucan.reject-legacy]]
- [x] [serial] I2. Move `clankers-ucan` onto the same remote-pinned OnixResearch `ucan` source used by Basalt and remove the sibling-path `../../../ucan` requirement from default builds. [covers=r[ucan-basalt-daemon-auth.public-ucan.dependency-source]]
- [x] [serial] I3. Add a public-UCAN signer/issuer adapter for the daemon owner iroh identity, including root issuance, delegation, import, revoke, and stored credential encode/decode helpers. [covers=r[ucan-basalt-daemon-auth.public-ucan.delegation-chain], r[ucan-basalt-daemon-auth.storage.versioned-records]]

## Phase 2: Basalt daemon authority

- [x] [serial] I4. Implement a shared `BasaltUcanAuthority` that verifies public UCAN tokens/proofs, checks replay/revocation state, runs Basalt policy for the same resource/ability, and emits redacted receipts. [covers=r[ucan-basalt-daemon-auth.basalt-policy.mandatory], r[ucan-basalt-daemon-auth.receipts.redacted]]
- [x] [serial] I5. Thread the authority through QUIC control create, QUIC attach, chat/RPC auth frames, Matrix stored credentials, keyed session recovery, and local allow-all bypasses without granting ambient authority when auth is configured. [covers=r[ucan-basalt-daemon-auth.daemon-seams.remote-entrypoints], r[ucan-basalt-daemon-auth.daemon-seams.allow-all-boundary]]
- [x] [serial] I6. Replace or wrap `UcanCapabilityGate` with call-time public UCAN + Basalt invocation checks for prompt/session, generic tool use, file read/write, shell execution, process actions, and model use. [covers=r[ucan-basalt-daemon-auth.tool-gate.call-time], r[ucan-basalt-daemon-auth.vocabulary.operation-matrix]]
- [x] [serial] I7. Add explicit legacy-token migration/import handling if needed, and keep legacy `clanker-auth` verification disabled by default. [covers=r[ucan-basalt-daemon-auth.migration.fail-closed]]

## Phase 3: Verification

- [x] [serial] V1. Add deterministic public UCAN fixtures for valid root/child/grandchild delegation plus malformed, expired, not-before, wrong-audience, missing-proof, replayed, and revoked credentials. [covers=r[ucan-basalt-daemon-auth.public-ucan.delegation-chain], r[ucan-basalt-daemon-auth.storage.replay-revocation], r[ucan-basalt-daemon-auth.verification.ucan-fixtures]] [evidence=evidence/ucan-fixtures.md]
- [x] [serial] V2. Add Basalt policy fixtures proving recognized Clankers resource/ability contracts allow and unknown contract/resource/ability or missing UCAN grants deny with redacted receipts. [covers=r[ucan-basalt-daemon-auth.basalt-policy.mandatory], r[ucan-basalt-daemon-auth.receipts.redacted], r[ucan-basalt-daemon-auth.verification.basalt-fixtures]] [evidence=evidence/basalt-fixtures.md]
- [x] [serial] V3. Add daemon seam tests for QUIC create, QUIC attach, chat/RPC auth frames, and Matrix/keyed-session auth using valid, missing, malformed, legacy, expired, revoked, wrong-audience, and policy-denied credentials. [covers=r[ucan-basalt-daemon-auth.daemon-seams.remote-entrypoints], r[ucan-basalt-daemon-auth.migration.fail-closed], r[ucan-basalt-daemon-auth.verification.daemon-seams]] [evidence=evidence/daemon-seams.md]
- [x] [serial] V4. Add tool-gate tests for read/write/edit/bash/process/model/session operations that assert concrete invocation mapping, Basalt receipt contents, and no human-confirmation bypass of UCAN/Basalt denial. [covers=r[ucan-basalt-daemon-auth.tool-gate.call-time], r[ucan-basalt-daemon-auth.vocabulary.operation-matrix], r[ucan-basalt-daemon-auth.verification.tool-gate]] [evidence=evidence/tool-gate-fixtures.md]
- [x] [serial] V5. Add dependency/source and boundary checks proving default daemon auth no longer constructs `clanker-auth::TokenVerifier` credentials and `clankers-ucan` no longer depends on a local sibling `../../../ucan` path. [covers=r[ucan-basalt-daemon-auth.public-ucan.dependency-source], r[ucan-basalt-daemon-auth.verification.dependency-boundary]] [evidence=evidence/public-ucan-boundary.md]
- [x] [serial] V6. Run focused Rust tests, `cargo check --tests` for touched crates, Cairn validation/gates, `git diff --check`, and sync/archive after implementation. [covers=r[ucan-basalt-daemon-auth.verification.closeout]] [evidence=evidence/closeout-validation.md]
