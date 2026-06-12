# Evidence: router-lib allow-site narrowing

Artifact-Type: validation-evidence
Task-ID: V#router-lib-allow-site
Covers: tigerstyle-compliance.slice-validation, tigerstyle-compliance.public-api-validation

## Summary

Narrowed the `crates/clanker-router/src/lib.rs` Tigerstyle allow site by draining `tigerstyle::bool_naming` from the crate-level allow list. Predicate locals now use positive predicate-style names.

Changed sites:

- `crates/clanker-router/src/lib.rs`: removed `tigerstyle::bool_naming` from the crate-level allow list.
- `crates/clanker-router/src/auth.rs`: renamed account-removal and active-account sort booleans.
- `crates/clanker-router/src/backends/anthropic.rs`: renamed reactive OAuth refresh state.
- `crates/clanker-router/src/backends/openai_codex/{attempt.rs,entitlement.rs}`: renamed Codex refresh state.
- `crates/clanker-router/src/credential.rs`: renamed auth-file lock acquisition state.
- `crates/clanker-router/src/quorum/mod.rs`: renamed assignment, similarity, and agreement booleans.
- `crates/clanker-router/src/router/mod.rs`: renamed retryability and quorum-result booleans.
- `crates/clanker-router/src/rpc/server.rs`: renamed RPC completion-drain state found by full Tigerstyle.

Base commit during validation: `545c149c880dc17bcbceed82733c6123d61430c1`.
Working tree at validation time contained this slice's modifications.

## Commands

### Focused package Tigerstyle

```bash
TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= ./xtask/tigerstyle.sh -p clanker-router -- --keep-going
```

Initial exit status after removing `bool_naming`: `1`.

Relevant package-scope `bool_naming` findings:

- `crates/clanker-router/src/auth.rs:290` (`removed`)
- `crates/clanker-router/src/auth.rs:342` (`a_active`)
- `crates/clanker-router/src/auth.rs:343` (`b_active`)
- `crates/clanker-router/src/backends/anthropic.rs:324` (`did_reactive_refresh`)
- `crates/clanker-router/src/backends/openai_codex/attempt.rs:46` (`did_refresh`)
- `crates/clanker-router/src/backends/openai_codex/entitlement.rs:225` (`did_refresh`)
- `crates/clanker-router/src/credential.rs:307` (`locked`)
- `crates/clanker-router/src/quorum/mod.rs:289` (`assigned`)
- `crates/clanker-router/src/quorum/mod.rs:292` (`similar`)
- `crates/clanker-router/src/quorum/mod.rs:348` (`all_agree`)
- `crates/clanker-router/src/router/mod.rs:305` (`retryable`)
- `crates/clanker-router/src/router/mod.rs:593` (`quorum_met`)

After renaming the predicate locals:

```bash
TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= ./xtask/tigerstyle.sh -p clanker-router -- --keep-going
```

Exit status: `0`.

Summary: default-feature `clanker-router` Tigerstyle completed successfully.

### Focused package tests

```bash
TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= cargo test -p clanker-router --lib
```

Exit status: `0`.

Summary: `238 passed; 0 failed; 0 ignored`.

### Full Tigerstyle

```bash
TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= ./xtask/tigerstyle.sh -- --keep-going
```

Initial full-workspace exit status after package-scope cleanup: `1`.

Additional feature-enabled finding:

- `crates/clanker-router/src/rpc/server.rs:241` (`complete_done`)

After renaming the RPC completion state:

```bash
TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= ./xtask/tigerstyle.sh -- --keep-going
```

Exit status: `0`.

Summary: workspace Tigerstyle completed successfully.
