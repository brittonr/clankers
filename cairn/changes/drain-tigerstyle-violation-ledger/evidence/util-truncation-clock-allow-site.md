Artifact-Type: validation
Task-ID: V#util-truncation-clock-allow-site
Covers: tigerstyle-compliance.slice-validation, tigerstyle-compliance.boundary-exceptions

# Validation: util truncation ambient-clock allow drain

Base commit before slice: `2368decb`.

## Changes validated

- Removed the local `tigerstyle::ambient_clock` allow from `crates/clankers-util/src/truncation.rs`.
- Replaced timestamp-derived temporary output filenames with process-id plus an atomic sequence counter.
- The remaining filename uniqueness source is local process state, not wall-clock time.

## Commands

```bash
TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= ./xtask/tigerstyle.sh -p clankers-util -- --keep-going
```

Exit status: 0.

Relevant output:

```text
Checking clankers-util v0.1.0 (/home/brittonr/git/clankers/crates/clankers-util)
Finished `dev` profile [optimized + debuginfo] target(s) in 10.22s
```

```bash
TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= cargo test -p clankers-util --lib
```

Exit status: 0.

Relevant output:

```text
running 87 tests
...
test result: ok. 87 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.04s
```

```bash
TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= ./xtask/tigerstyle.sh -- --keep-going
```

Exit status: 0.

Relevant output:

```text
Checking clankers-matrix v0.1.0 (/home/brittonr/git/clankers/crates/clankers-matrix)
Finished `dev` profile [optimized + debuginfo] target(s) in 1m 14s
```
