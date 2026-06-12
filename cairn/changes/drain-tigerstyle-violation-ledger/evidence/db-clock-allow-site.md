# Evidence: db-clock allow-site review

Artifact-Type: validation-evidence
Task-ID: V#db-clock-allow-site
Covers: tigerstyle-compliance.slice-validation, tigerstyle-compliance.boundary-exceptions

## Summary

Reviewed the local `tigerstyle::ambient_clock` allow in `crates/clankers-db/src/lib.rs` and kept it as a narrow shell-boundary exception.

The allow is scoped to one helper:

```rust
pub(crate) fn db_clock_now() -> DateTime<Utc> {
    Utc::now()
}
```

Rationale:

- The ambient clock read is centralized in `db_clock_now()` rather than duplicated across table modules.
- The helper is `pub(crate)`, so it does not expose a public API clock dependency.
- The helper has a specific allow reason: `database shell-boundary timestamp source`.
- Prior database drain work already routed direct database timestamp reads through this helper; this task confirms the remaining local allow is an intentional boundary rather than broad lint debt.

Base commit during validation: `130fa8199`.
Working tree at validation time contained this evidence/tasks/design update only.

## Commands

### Focused package Tigerstyle

```bash
TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= ./xtask/tigerstyle.sh -p clankers-db -- --keep-going
```

Exit status: `0`.

Summary: `clankers-db` Tigerstyle completed successfully with the reviewed narrow local `ambient_clock` boundary.

### Focused package tests

```bash
TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= cargo test -p clankers-db --lib
```

Exit status: `0`.

Summary: `193 passed; 0 failed; 0 ignored`.

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
