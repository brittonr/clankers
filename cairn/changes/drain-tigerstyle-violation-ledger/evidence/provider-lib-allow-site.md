# Evidence: provider-lib allow-site narrowing

Artifact-Type: validation-evidence
Task-ID: V#provider-lib-allow-site
Covers: tigerstyle-compliance.slice-validation, tigerstyle-compliance.public-api-validation

## Summary

Narrowed the `crates/clankers-provider/src/lib.rs` Tigerstyle allow site by draining `tigerstyle::bool_naming` from the crate-level allow list. Predicate locals now use positive predicate-style names.

Changed sites:

- `crates/clankers-provider/src/lib.rs`: removed `tigerstyle::bool_naming` from the crate-level allow list.
- `crates/clankers-provider/src/anthropic/streaming.rs`: renamed the SSE reverse-map boolean to `is_reverse_map_enabled`.
- `crates/clankers-provider/src/anthropic/mod.rs`: renamed refresh-attempt state to `is_refresh_attempted`.
- `crates/clankers-provider/src/router.rs`: renamed downstream channel state to `is_downstream_open`.

Base commit during validation: `793f71e0acb2fa5fa90eb1a3108afb1e783fb999`.
Working tree at validation time contained this slice's modifications.

## Commands

### Focused package Tigerstyle

```bash
TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= ./xtask/tigerstyle.sh -p clankers-provider -- --keep-going
```

Initial exit status after removing `bool_naming`: `1`.

Relevant `bool_naming` findings:

- `crates/clankers-provider/src/anthropic/streaming.rs:106` (`reverse_map`)
- `crates/clankers-provider/src/anthropic/mod.rs:184` (`refresh_attempted`)
- `crates/clankers-provider/src/router.rs:311` (`downstream_open`)

After renaming the predicate locals:

```bash
TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= ./xtask/tigerstyle.sh -p clankers-provider -- --keep-going
```

Exit status: `0`.

Summary: `clankers-provider` Tigerstyle completed successfully after `bool_naming` was removed from `crates/clankers-provider/src/lib.rs`.

### Focused package tests

```bash
TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= cargo test -p clankers-provider --lib
```

Exit status: `0`.

Summary: `180 passed; 0 failed; 0 ignored`.

### Full Tigerstyle

```bash
TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= ./xtask/tigerstyle.sh -- --keep-going
```

Exit status: `0`.

Summary: workspace Tigerstyle completed successfully.
