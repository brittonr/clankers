# Evidence: controller-lib allow-site narrowing

Artifact-Type: validation-evidence
Task-ID: V#controller-lib-allow-site
Covers: tigerstyle-compliance.slice-validation, tigerstyle-compliance.public-api-validation

## Summary

Narrowed the `crates/clankers-controller/src/lib.rs` Tigerstyle allow site by draining `tigerstyle::bool_naming` from the crate-level allow list. Predicate locals now use positive predicate-style names.

Changed sites:

- `crates/clankers-controller/src/lib.rs`: removed `tigerstyle::bool_naming` from the crate-level allow list.
- `crates/clankers-controller/src/auto_test.rs`: renamed prompt-completion booleans to `is_applied` / `is_prompt_applied`.
- `crates/clankers-controller/src/command.rs`: renamed prompt-completion booleans to `is_applied`.
- `crates/clankers-controller/src/core_effects.rs`: renamed replay, feedback, loop-state, and loop-finished booleans to predicate names.
- `crates/clankers-controller/src/event_processing.rs`: renamed embedded prompt start boolean to `is_started`.

Base commit during validation: `984323720d83e0b08c57bbee36241e474cd5bf6e`.
Working tree at validation time contained this slice's modifications.

## Commands

### Focused package Tigerstyle

```bash
TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= ./xtask/tigerstyle.sh -p clankers-controller -- --keep-going
```

Initial exit status after removing `bool_naming`: `1`.

Relevant `bool_naming` findings:

- `crates/clankers-controller/src/auto_test.rs:69` (`applied`)
- `crates/clankers-controller/src/auto_test.rs:204` (`prompt_applied`)
- `crates/clankers-controller/src/command.rs:536` (`applied`)
- `crates/clankers-controller/src/command.rs:732` (`applied`)
- `crates/clankers-controller/src/core_effects.rs:56` (`replay_queued_prompt`)
- `crates/clankers-controller/src/core_effects.rs:151` (`all_feedback_applied`)
- `crates/clankers-controller/src/core_effects.rs:164` (`applied`)
- `crates/clankers-controller/src/core_effects.rs:185` (`saw_loop_state_change`)
- `crates/clankers-controller/src/core_effects.rs:209` (`saw_loop_state_change`)
- `crates/clankers-controller/src/core_effects.rs:287` (`loop_finished`)
- `crates/clankers-controller/src/event_processing.rs:90` (`started`)

After renaming the predicate locals:

```bash
TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= ./xtask/tigerstyle.sh -p clankers-controller -- --keep-going
```

Exit status: `0`.

Summary: `clankers-controller` Tigerstyle completed successfully after `bool_naming` was removed from `crates/clankers-controller/src/lib.rs`.

### Focused package tests

```bash
TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= cargo test -p clankers-controller --lib
```

Exit status: `0`.

Summary: `192 passed; 0 failed; 2 ignored`.

### Full Tigerstyle

```bash
TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= ./xtask/tigerstyle.sh -- --keep-going
```

Exit status: `0`.

Summary: workspace Tigerstyle completed successfully.
