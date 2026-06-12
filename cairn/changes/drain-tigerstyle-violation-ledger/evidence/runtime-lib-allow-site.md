# Evidence: runtime-lib allow-site narrowing

Artifact-Type: validation-evidence
Task-ID: V#runtime-lib-allow-site
Covers: tigerstyle-compliance.slice-validation, tigerstyle-compliance.public-api-validation

## Summary

Narrowed the `crates/clankers-runtime/src/lib.rs` Tigerstyle allow site by draining `tigerstyle::no_unwrap` from the crate-level allow list. Production receipt/hash serialization no longer uses `expect(...)`; helper serialization functions provide explicit fallback bytes/strings. Built-in tool catalog constructors now use explicit fallback catalog construction instead of `expect(...)`.

Changed sites:

- `crates/clankers-runtime/src/lib.rs`: removed `tigerstyle::no_unwrap` from the crate-level allow list and added explicit JSON fallback helpers.
- `crates/clankers-runtime/src/{dynamic_runtime.rs,steel_mutation.rs,steel_orchestration.rs,steel_orchestration_mutation.rs,steel_repo_evolution.rs,steel_runtime.rs,steel_tool_substrate.rs}`: replaced production serialization `expect(...)` calls with fallback helper calls.
- `crates/clankers-runtime/src/tools.rs`: replaced built-in catalog `expect(...)` calls with non-panicking fallback construction.

Base commit during validation: `76d621bfff87d572902b2353e3b337a5003009ce`.
Working tree at validation time contained this slice's modifications.

## Commands

### Focused package Tigerstyle

```bash
TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= ./xtask/tigerstyle.sh -p clankers-runtime -- --keep-going
```

Initial exit status after removing `no_unwrap`: `1`.

Relevant production `no_unwrap` findings:

- `crates/clankers-runtime/src/dynamic_runtime.rs:343`
- `crates/clankers-runtime/src/steel_mutation.rs:241`, `:300`, `:358`
- `crates/clankers-runtime/src/steel_orchestration.rs:228`, `:299`, `:606`, `:628`, `:700`, `:1288`, `:1367`, `:1443`
- `crates/clankers-runtime/src/steel_orchestration_mutation.rs:81`
- `crates/clankers-runtime/src/steel_repo_evolution.rs:109`, `:159`
- `crates/clankers-runtime/src/steel_runtime.rs:47`
- `crates/clankers-runtime/src/steel_tool_substrate.rs:338`, `:363`
- `crates/clankers-runtime/src/tools.rs:41`, `:51`

After replacing those panicking calls:

```bash
TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= ./xtask/tigerstyle.sh -p clankers-runtime -- --keep-going
```

Exit status: `0`.

Summary: `clankers-runtime` Tigerstyle completed successfully after `no_unwrap` was removed from `crates/clankers-runtime/src/lib.rs`.

### Focused package tests

```bash
TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= cargo test -p clankers-runtime --lib
```

Exit status: `0`.

Summary: `178 passed; 0 failed; 0 ignored`.

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
