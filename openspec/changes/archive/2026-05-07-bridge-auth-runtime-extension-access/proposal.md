## Why

Plugin and provider execution now route through explicit runtime extension services, but auth-store and credential-pool access still need an injected, default-safe runtime path. Embedded hosts must be able to inspect/select credentials without `clankers-runtime` or desktop adapters implicitly reading auth files, writing OAuth verifier state, or persisting refreshed tokens.

## What Changes

- Add a desktop adapter path that accepts an injected auth store for auth lookup and credential-pool selection.
- Keep default desktop runtime construction fail-closed for extension auth and credential-pool operations unless a host injects auth material.
- Return safe receipts with provider/account/count/status metadata only; never credential values, verifier contents, refresh tokens, headers, env values, or raw auth-file contents.

## Impact

- Files: `crates/clankers-runtime/src/lib.rs`, `src/runtime_services.rs`, OpenSpec specs/tasks.
- APIs: adds explicit desktop runtime service constructor(s) for injected auth-store access.
- Testing: focused runtime service tests plus strict OpenSpec validation and cargo checks.
