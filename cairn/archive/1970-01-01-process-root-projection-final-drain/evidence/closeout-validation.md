# Closeout validation evidence

Evidence-ID: process-root-closeout-validation
Artifact-Type: command-output-summary
Task-ID: V2
Covers: process-root-projection-final-drain.verification
Date: 2026-06-02
Status: PASS

## Commands

```text
TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= cargo check -p clankers --tests
TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= ./scripts/check-process-job-boundary.rs
TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= ./scripts/check-lego-architecture-boundaries.rs
nix run .#cairn -- gate tasks process-root-projection-final-drain --root .
nix run .#cairn -- validate --root .
git diff --check
```

## Relevant output

```text
cargo check -p clankers --tests
Finished `dev` profile [optimized + debuginfo] target(s) in 16.71s

./scripts/check-process-job-boundary.rs
ok: process-job boundary rail passed

./scripts/check-lego-architecture-boundaries.rs
lego architecture dependency ownership inventory written to target/lego-architecture/dependency-ownership-inventory.json

nix run .#cairn -- gate tasks process-root-projection-final-drain --root .
"valid": true,
"verdict": "PASS"

nix run .#cairn -- validate --root .
"changes": 7,
"specs_validated": 58,
"valid": true

git diff --check
exit 0
```

## Coverage notes

The process boundary rail now expects `NativeProcessJobService` and `ProcessEntry` in `src/tools/process/native.rs`. The lego architecture rail also requires those native owners and rejects native service/status definitions from `src/tools/process.rs`.
