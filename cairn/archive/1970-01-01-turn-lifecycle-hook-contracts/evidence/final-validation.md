Evidence-ID: final-validation
Task-ID: V5
Artifact-Type: command-log
Covers: turn-lifecycle-hooks.docs-config, turn-lifecycle-hooks.validation
Status: complete

# Final Validation

Closeout validation was run from `/home/brittonr/git/clankers` after the hook ordering and payload redaction rails were implemented.

## Hook-focused tests

```text
TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= \
  cargo nextest run -p clankers-hooks
```

Result: 50 tests run, 50 passed, 0 skipped.

```text
TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= \
  cargo nextest run -p clankers-plugin hooks
```

Result: 1 test run, 1 passed, 41 skipped.

```text
TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= \
  cargo nextest run -p clankers-agent pre_
```

Result: 5 tests run, 5 passed, 184 skipped.

```text
TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= \
  cargo nextest run -p clankers-controller controller_owned_prompt
```

Result: 2 tests run, 2 passed, 229 skipped.

## Repository validation rails

```text
TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= ./scripts/verify.sh
```

Result: exit status 0. The script reported:

- Verus: 71 verified, 0 errors.
- No-std functional core validation bundle passed.
- Controller reducer, shell-boundary, input/output-translation, transport/client-boundary, and parity suites: 229 tests run, 229 passed, 2 skipped.
- Agent turn allowlist/filter/thinking/tool-inventory parity rails passed.
- Embedded runtime prompt-lifecycle parity suite: 7 passed.
- Embedded controller parity suite: 38 tests run, 38 passed.
- Tracey: 47 of 47 requirements covered; 47 of 47 have a verification reference.
- Final script line: `=== All checks passed ===`.

## Cairn gates

```text
TMPDIR=/home/brittonr/.cargo-target/tmp \
  nix run .#cairn -- gate proposal turn-lifecycle-hook-contracts --root .
```

Result: PASS.

```text
TMPDIR=/home/brittonr/.cargo-target/tmp \
  nix run .#cairn -- gate design turn-lifecycle-hook-contracts --root .
```

Result: PASS.

```text
TMPDIR=/home/brittonr/.cargo-target/tmp \
  nix run .#cairn -- gate tasks turn-lifecycle-hook-contracts --root .
```

Result: PASS.

```text
TMPDIR=/home/brittonr/.cargo-target/tmp \
  nix run .#cairn -- validate --root .
```

Result: valid true, changes 2, specs_validated 52, no issues.

## Diff hygiene

```text
git diff --check
```

Result: exit status 0.
