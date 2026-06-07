# Change: Remote Auth UX Docs

## Why

Public UCAN + Basalt daemon auth is implemented and archived, but the operator-facing docs still describe capability tokens at a high level. Users need an authoritative workflow for discovering the daemon audience, minting root and delegated credentials, installing credentials for QUIC/chat/Matrix entrypoints, rotating or revoking them, and understanding how Basalt participates in admission.

## What Changes

- Add a remote-auth guide that explains the public UCAN + Basalt model in user terms without exposing token bodies or secret material.
- Document concrete command flows for creating scoped credentials, delegating from a parent credential, attaching to remote daemons, and revoking/rotating credentials.
- Clarify the source boundary: local Cargo uses `../basalt`, while Nix maps that path to the pinned `OnixResearch/basalt` flake input through `externalSources`.
- Add a deterministic docs/help drift rail so examples stay aligned with clap command shapes and existing public UCAN boundary tests.

## Impact

- **Files**: `docs/src/getting-started/auth.md`, `docs/src/reference/daemon.md`, a new remote-auth reference page, `README.md`, docs summary/navigation, and a focused docs/source contract test.
- **Testing**: docs contract test for token/remote-auth examples, existing `tests/public_ucan_boundary.rs`, Cairn validation and gates, and `git diff --check`.
