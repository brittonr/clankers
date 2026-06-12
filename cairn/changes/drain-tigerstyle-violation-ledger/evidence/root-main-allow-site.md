# Evidence: root-main allow-site narrowing

Artifact-Type: validation-evidence
Task-ID: V#root-main-allow-site
Covers: tigerstyle-compliance.slice-validation, tigerstyle-compliance.public-api-validation

## Summary

Narrowed the `src/main.rs` binary Tigerstyle allow site by removing `tigerstyle::catch_all_on_enum`. Temporary discovery with the broad allow removed showed no remaining `catch_all_on_enum` findings in the binary; the remaining findings are existing binary entrypoint/orchestration debt still covered by the narrowed allow list.

Changed sites:

- `src/main.rs`: removed `tigerstyle::catch_all_on_enum` from the binary crate-level allow list and updated the reason.

Base commit during validation: `a2f3427e80b3efd7bfa6621cf9b4276b77682d1c`.
Working tree at validation time contained this slice's modifications.

## Commands

### Discovery

```bash
TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= ./xtask/tigerstyle.sh -p clankers -- --keep-going
```

Exit status: `1` after temporarily removing the broad `src/main.rs` allow block for discovery.

Relevant result: no `catch_all_on_enum` findings were reported for `src/main.rs`. Remaining binary findings were assertion density, function length, no-unwrap, too-many-parameters, and explicit-defaults.

### Focused package Tigerstyle

```bash
TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= ./xtask/tigerstyle.sh -p clankers -- --keep-going
```

Exit status: `0`.

Summary: root package Tigerstyle completed successfully after `catch_all_on_enum` was removed from `src/main.rs`.

### Root compile validation

```bash
TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= cargo test -p clankers --no-run
```

Exit status: `0`.

Summary: root package test targets compiled successfully.

### Full Tigerstyle

```bash
TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= ./xtask/tigerstyle.sh -- --keep-going
```

Exit status: `0`.

Summary: workspace Tigerstyle completed successfully.
