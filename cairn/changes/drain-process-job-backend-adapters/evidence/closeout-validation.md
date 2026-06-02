# Closeout validation evidence

Evidence-ID: process-backend-adapter-closeout
Artifact-Type: command-output-summary
Task-ID: V3
Covers: process-job-backend-adapters.verification.closeout
Date: 2026-06-02
Status: PASS

## Commands

```text
TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= cargo test -p clankers --no-run
TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= cargo nextest run -p clankers tools::process::
TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= cargo check -p clankers --tests
./scripts/check-lego-architecture-boundaries.rs
./scripts/check-process-job-boundary.rs
git diff --check
nix run .#cairn -- gate proposal drain-process-job-backend-adapters --root .
nix run .#cairn -- gate design drain-process-job-backend-adapters --root .
nix run .#cairn -- gate tasks drain-process-job-backend-adapters --root .
nix run .#cairn -- validate --root .
```

## Relevant output before Cairn re-gate

```text
Finished `test` profile [optimized + debuginfo] target(s) in 1m 01s
Summary: 48 tests run: 48 passed, 1488 skipped
Finished `dev` profile [optimized + debuginfo] target(s) in 22.49s
lego architecture dependency ownership inventory written to target/lego-architecture/dependency-ownership-inventory.json
ok: process-job boundary rail passed
git diff --check: exit 0
```

## Cairn output

```text
nix run .#cairn -- gate proposal drain-process-job-backend-adapters --root .
{
  "change": "drain-process-job-backend-adapters",
  "issues": [],
  "stage": "proposal",
  "valid": true,
  "verdict": "PASS"
}

nix run .#cairn -- gate design drain-process-job-backend-adapters --root .
{
  "change": "drain-process-job-backend-adapters",
  "issues": [],
  "stage": "design",
  "valid": true,
  "verdict": "PASS"
}

nix run .#cairn -- gate tasks drain-process-job-backend-adapters --root .
{
  "change": "drain-process-job-backend-adapters",
  "issues": [],
  "stage": "tasks",
  "valid": true,
  "verdict": "PASS"
}

nix run .#cairn -- validate --root .
{
  "change_issues": [],
  "changes": 1,
  "issues": [],
  "spec_issues": [],
  "specs_validated": 52,
  "valid": true
}
```
