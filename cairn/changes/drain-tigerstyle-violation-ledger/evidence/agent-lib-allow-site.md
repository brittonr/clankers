# Evidence: agent-lib allow-site narrowing

Artifact-Type: validation-evidence
Task-ID: V#agent-lib-allow-site
Covers: tigerstyle-compliance.slice-validation, tigerstyle-compliance.public-api-validation

## Summary

Narrowed the `crates/clankers-agent/src/lib.rs` Tigerstyle allow site by draining `tigerstyle::bool_naming` from the crate-level allow list. Predicate locals now use positive predicate-style names.

Changed sites:

- `crates/clankers-agent/src/lib.rs`: removed `tigerstyle::bool_naming` from the crate-level allow list; renamed `saw_usage`, `pre_turn_fired`, `used_skill_manage`, and `in_latest_turn` locals.
- `crates/clankers-agent/src/error.rs`: renamed retryable projection local to `is_retryable`.
- `crates/clankers-agent/src/turn/execution.rs`: renamed tool-filter decision local to `is_allowed`.

Base commit during validation: `48d5e8b6aad8ade11d87ab964581383b8c21ecac`.
Working tree at validation time contained this slice's modifications.

## Commands

### Discovery

```bash
TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= ./xtask/tigerstyle.sh -p clankers-agent -- --keep-going
```

Exit status: `1` after temporarily removing the broad `clankers-agent` crate allow block.

Relevant `bool_naming` findings:

- `crates/clankers-agent/src/error.rs:54` (`retryable`)
- `crates/clankers-agent/src/turn/execution.rs:293` (`allowed`)
- `crates/clankers-agent/src/lib.rs:235` (`saw_usage`)
- `crates/clankers-agent/src/lib.rs:1107` (`pre_turn_fired`)
- `crates/clankers-agent/src/lib.rs:1320` (`used_skill_manage`)
- `crates/clankers-agent/src/lib.rs:1321` (`in_latest_turn`)

### Focused package Tigerstyle

```bash
TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= ./xtask/tigerstyle.sh -p clankers-agent -- --keep-going
```

Exit status: `0`.

Summary: `clankers-agent` Tigerstyle completed successfully after `bool_naming` was removed from `crates/clankers-agent/src/lib.rs`.

### Focused package tests

```bash
TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= cargo test -p clankers-agent --lib user_tool_filter
TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= cargo test -p clankers-agent --lib skill_creation_nudge
TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= cargo test -p clankers-agent --lib pre_and_post_turn_share_correlation_and_post_usage
```

Exit status: `0` for all three commands.

Summary:

- `user_tool_filter`: `5 passed; 0 failed`
- `skill_creation_nudge`: `4 passed; 0 failed`
- `pre_and_post_turn_share_correlation_and_post_usage`: `1 passed; 0 failed`

### Package compile validation

```bash
TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= cargo test -p clankers-agent --no-run
```

Exit status: `0`.

Summary: `clankers-agent` test target compiled successfully.

### Full Tigerstyle

```bash
TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= ./xtask/tigerstyle.sh -- --keep-going
```

Exit status: `0`.

Summary: workspace Tigerstyle completed successfully.
