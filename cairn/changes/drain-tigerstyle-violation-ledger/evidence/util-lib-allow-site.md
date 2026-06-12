Artifact-Type: validation
Task-ID: V#util-lib-allow-site
Covers: tigerstyle-compliance.slice-validation, tigerstyle-compliance.public-api-validation

# Validation: clankers-util crate-level Tigerstyle allow drain

Base commit before slice: `42d308d82`.

## Changes validated

- Removed the remaining crate-level Tigerstyle allow block from `crates/clankers-util/src/lib.rs`.
- Drained `usize_in_public_api`; package Tigerstyle reported no findings.
- Drained `unbounded_loop`; refactored `ansi::strip_ansi` from iterator-driven unbounded loops to bounded index walks over collected input characters.
- Drained `function_length`; split `expand_at_refs_with_policy` into phase helpers for git diff, URL, unsupported, image, and text references.

## Commands

```bash
TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= ./xtask/tigerstyle.sh -p clankers-util -- --keep-going
```

Exit status: 0.

Relevant output:

```text
Checking clankers-util v0.1.0 (/home/brittonr/git/clankers/crates/clankers-util)
Finished `dev` profile [optimized + debuginfo] target(s) ...
```

```bash
TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= cargo test -p clankers-util --lib
```

Exit status: 0.

Relevant output:

```text
running 87 tests
...
test result: ok. 87 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s
```

```bash
TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= cargo test -p clankers --no-run
```

Exit status: command output reached `Finished` before the pi tool timeout; no public util API moved in this slice, so this was supplemental root compile coverage rather than required API-migration evidence.

Relevant output:

```text
Compiling clankers v0.1.0 (/home/brittonr/git/clankers)
Finished `test` profile [optimized + debuginfo] target(s) in 8m 22s
Executable unittests src/lib.rs ...
```

```bash
TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= ./xtask/tigerstyle.sh -- --keep-going
```

Exit status: 0.

Relevant output:

```text
Checking clankers-matrix v0.1.0 (/home/brittonr/git/clankers/crates/clankers-matrix)
Finished `dev` profile [optimized + debuginfo] target(s) in 1m 25s
```
