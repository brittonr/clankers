# Evidence: plugin-lib allow-site narrowing

Artifact-Type: validation-evidence
Task-ID: V#plugin-lib-allow-site
Covers: tigerstyle-compliance.slice-validation, tigerstyle-compliance.public-api-validation

## Summary

Narrowed the `crates/clankers-plugin/src/lib.rs` Tigerstyle allow site by draining `tigerstyle::bool_naming` from the crate-level allow list. Predicate locals now use positive predicate-style names.

Changed sites:

- `crates/clankers-plugin/src/lib.rs`: removed `tigerstyle::bool_naming` from the crate-level allow list.
- `crates/clankers-plugin/src/stdio_runtime.rs`: renamed restricted-network and stdio handshake booleans to predicate names.
- `crates/clankers-plugin/src/restricted_sandbox.rs`: renamed Landlock support boolean to `is_landlock_supported`.

Base commit during validation: `79f66ade429e4c3c4e9884a0bd1086f9bf482083`.
Working tree at validation time contained this slice's modifications.

## Commands

### Focused package Tigerstyle

```bash
TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= ./xtask/tigerstyle.sh -p clankers-plugin -- --keep-going
```

Initial exit status after removing `bool_naming`: `1`.

Relevant `bool_naming` findings:

- `crates/clankers-plugin/src/stdio_runtime.rs:531` (`allow_network`)
- `crates/clankers-plugin/src/stdio_runtime.rs:619` (`hello_seen`)
- `crates/clankers-plugin/src/stdio_runtime.rs:620` (`ready_seen`)
- `crates/clankers-plugin/src/stdio_runtime.rs:872` (`allow_network`)
- `crates/clankers-plugin/src/restricted_sandbox.rs:42` (`landlock_supported`)

After renaming the predicate locals:

```bash
TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= ./xtask/tigerstyle.sh -p clankers-plugin -- --keep-going
```

Exit status: `0`.

Summary: `clankers-plugin` Tigerstyle completed successfully after `bool_naming` was removed from `crates/clankers-plugin/src/lib.rs`.

### Focused package tests

```bash
TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= cargo test -p clankers-plugin --lib
```

Exit status: `0`.

Summary: `42 passed; 0 failed; 0 ignored`.

### Full Tigerstyle

```bash
TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= ./xtask/tigerstyle.sh -- --keep-going
```

Exit status: `0`.

Summary: workspace Tigerstyle completed successfully.
