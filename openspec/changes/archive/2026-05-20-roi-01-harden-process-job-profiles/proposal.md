## Why

Clankers now has a reusable process-job profile kit and a durable `process` surface that can target native, pueue, and systemd backends. The current requirements cover the broad durable-job model and the initial profile-kit boundary, but the next implementation work needs tighter rails before behavior changes: profile discovery precedence, manifest schema/versioning, safe identity inputs, profile execution receipts, and negative-policy evidence should be explicit enough that fallback backend behavior, ambient config, or redaction drift cannot hide mistakes.

## What Changes

- **Profile manifest hardening**: Define a versioned profile manifest contract with deterministic discovery precedence and no ambient backend dispatch during resolution.
- **Execution receipt hardening**: Require profile-start receipts to report profile identity, resolved backend, policy source, and validation evidence without raw secrets.
- **Policy and redaction guardrails**: Require negative fixtures for malformed commands, disallowed backends, secret-like environment keys, resource ceilings, cwd/writable-path policy, and unsupported backend fallbacks.
- **Operator/docs evidence**: Keep docs, fixtures, and `scripts/check-process-job-profile-kit.rs` aligned as the deterministic drift rail.

## Capabilities

### Modified Capabilities
- `durable-process-jobs`: hardens project profile resolution, execution receipts, and profile-kit drift evidence.
- `process-job-tool-api`: profile-start requests continue to flow through backend-neutral typed DTOs before backend dispatch.
- `process-job-security-redaction`: profile metadata and receipts must stay safe under shared redaction rules.

## Impact

- **Files**: likely `crates/clankers-runtime/src/process_jobs.rs`, process tool request parsing/service code, docs under `docs/src/reference/process-jobs.md`, fixtures/policy under examples or policy dirs, and `scripts/check-process-job-profile-kit.rs`.
- **APIs**: may extend typed process/job DTOs and receipts for profile source/version/policy metadata; should preserve existing direct `process` actions.
- **Dependencies**: no new runtime dependencies expected; prefer Rust/Nix-owned fixtures and checks.
- **Testing**: focused runtime tests for positive and negative profile resolution, receipt redaction, no-backend-dispatch fakes, Nix/module policy if touched, plus `scripts/check-process-job-profile-kit.rs`, `cargo nextest run -p clankers-runtime`, and targeted `cargo nextest run -p clankers` filters for process tool wiring.

## Out of Scope

- Replacing native/pueue/systemd backend implementations wholesale.
- Public unattended production readiness claims.
- Live provider/network tests or credential-dependent profile examples.
- Shell/Python-owned guard rails; durable checks should remain Rust/Nix owned.
