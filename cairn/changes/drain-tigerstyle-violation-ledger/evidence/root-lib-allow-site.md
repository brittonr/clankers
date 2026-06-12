# Evidence: root-lib allow-site narrowing

Artifact-Type: validation-evidence
Task-ID: V#root-lib-allow-site
Covers: tigerstyle-compliance.slice-validation, tigerstyle-compliance.public-api-validation

## Summary

Narrowed the `src/lib.rs` root Tigerstyle allow site by draining `tigerstyle::catch_all_on_enum` from the root crate allow list. The remaining allow list still covers existing root CLI/orchestration debt, but new enum variants are no longer allowed to silently hit `_` deny fallbacks in the root crate.

Changed sites:

- `src/lib.rs`: removed `tigerstyle::catch_all_on_enum` from the crate-level root allow list and updated the reason.
- `src/modes/attach/commands.rs`: enumerated all `clanker_message::Content` variants when converting image prompt blocks.
- `src/slash_commands/effects.rs`: enumerated all `AgentCommand` variants that intentionally produce no attach slash effect.
- `src/tools/skill_manage.rs`: replaced the `_` match arm with explicit `Some(empty)` / `None` handling.

Base commit during validation: `d5d87844d8a63fb6ac377d8e3e9b3f6d7fb8924d`.
Working tree at validation time contained this slice's modifications.

## Commands

### Discovery

```bash
TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= ./xtask/tigerstyle.sh -p clankers -- --keep-going
```

Exit status: `1` after temporarily removing the broad root allow block for discovery.

Relevant findings for `catch_all_on_enum`:

- `src/modes/attach/commands.rs:189`
- `src/slash_commands/effects.rs:87`
- `src/tools/skill_manage.rs:202`

### Focused package Tigerstyle

```bash
TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= ./xtask/tigerstyle.sh -p clankers -- --keep-going
```

Exit status: `0`.

Summary: root package Tigerstyle completed successfully after `catch_all_on_enum` was removed from `src/lib.rs`.

### Root compile validation

```bash
TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= cargo test -p clankers --no-run
```

Exit status: `0`.

Summary: root package test targets compiled successfully.

### Root library tests

```bash
TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= \
  CC=gcc CXX=g++ \
  CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_LINKER=gcc \
  RUSTFLAGS='-C link-arg=-fuse-ld=bfd' \
  cargo test -p clankers --lib
```

Exit status: `0`.

Summary: `1058 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out`.

### Full Tigerstyle

```bash
TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= ./xtask/tigerstyle.sh -- --keep-going
```

Exit status: `0`.

Summary: workspace Tigerstyle completed successfully.
