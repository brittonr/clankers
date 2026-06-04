## Why

The current `full` test harness is green, but several readiness surfaces remain outside its default proof: shell-scripted E2E flows, live model smoke, NixOS VM runtime/module checks, and flake/CI checks. Some of those rails are driven by Bash selectors, which makes them harder to compose under `cargo nextest`, harder to shard, and easier to overclaim as tested when they are only script-invoked.

## What Changes

- Add a Rust/`cargo nextest` release-readiness matrix that covers the currently separate E2E, live, VM, and CI readiness gaps.
- Replace Bash-only E2E acceptance with hardened Rust integration tests that invoke the compiled binary through isolated fixtures and assert artifacts/output directly.
- Wrap VM/flake/live readiness checks in Rust integration tests with explicit opt-in gates, bounded timeouts, structured skip reasons, and redacted diagnostics.
- Keep shell scripts as optional convenience wrappers only; they must not be the source of truth for release-readiness claims.

## Capabilities

### New Capabilities
- `release-readiness.nextest-matrix`: credential-safe readiness coverage runnable through `cargo nextest`.
- `release-readiness.rust-e2e`: Rust-owned binary E2E tests replacing ad-hoc Bash assertions.
- `release-readiness.vm-ci-live-adapters`: Rust test adapters for VM, flake, and live-provider checks with explicit gating.

## Impact

- **Files**: new or updated Rust integration tests under `tests/`, optional nextest configuration, release-readiness docs, and removal/demotion of Bash-only readiness claims.
- **APIs**: no product API changes required; test helper APIs may be factored for binary invocation, temp homes, fake providers, and redacted command receipts.
- **Dependencies**: prefer existing dev dependencies; add small test-only crates only if needed for timeouts/temp fixtures/assertions.
- **Testing**: `cargo nextest run --workspace --no-fail-fast` must include credential-free readiness tests; opt-in VM/live/flake tests must be runnable with documented nextest filters/env and must produce deterministic skip/fail behavior.
