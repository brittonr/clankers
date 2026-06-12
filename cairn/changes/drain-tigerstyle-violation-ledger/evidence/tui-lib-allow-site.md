# Evidence: tui-lib allow-site narrowing

Artifact-Type: validation-evidence
Task-ID: V#tui-lib-allow-site
Covers: tigerstyle-compliance.slice-validation, tigerstyle-compliance.public-api-validation

## Summary

Narrowed the `crates/clankers-tui/src/lib.rs` Tigerstyle allow site by draining `tigerstyle::bool_naming` from the crate-level allow list.

Changed sites:

- `crates/clankers-tui/src/lib.rs`: removed `tigerstyle::bool_naming` from the crate-level allow list.
- `crates/clankers-tui/src/components/experiment_dashboard.rs`: renamed local predicate binding `minimize` to `is_minimize`.

Base commit during validation: `d73a83c7cb21da40efb7571b0b601ad3c3d78116`.
Working tree at validation time contained this slice's modifications.

## Commands

### Focused package Tigerstyle

```bash
TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= ./xtask/tigerstyle.sh -p clankers-tui -- --keep-going
```

Initial exit status after removing `bool_naming`: `1`.

Relevant finding:

- `crates/clankers-tui/src/components/experiment_dashboard.rs:36`: boolean binding `minimize` should have a predicate prefix.

After renaming the predicate binding:

```bash
TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= ./xtask/tigerstyle.sh -p clankers-tui -- --keep-going
```

Exit status: `0`.

Summary: `clankers-tui` Tigerstyle completed successfully after `bool_naming` was removed from `crates/clankers-tui/src/lib.rs`.

### Focused package tests

```bash
TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= cargo test -p clankers-tui --lib
```

Exit status: `0`.

Summary: `286 passed; 0 failed; 0 ignored`.

### Full Tigerstyle

```bash
TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= ./xtask/tigerstyle.sh -- --keep-going
```

Exit status: `0`.

Summary: workspace Tigerstyle completed successfully.

### Cairn and whitespace gates

```bash
nix run .#cairn -- validate --root .
nix run .#cairn -- gate tasks drain-tigerstyle-violation-ledger --root .
git diff --check
```

Exit status: `0`.

Summary: Cairn validate reported `valid: true`, tasks gate reported `verdict: PASS`, and `git diff --check` reported no whitespace errors.
