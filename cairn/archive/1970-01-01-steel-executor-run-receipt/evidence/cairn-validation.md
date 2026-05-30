Evidence-ID: cairn-validation
Artifact-Type: test-report
Task-ID: V2
Covers: r[steel-executor-run-receipt.execution-receipt], r[steel-executor-run-receipt.redaction]
Created: 2026-05-30
Status: complete

# Cairn and Static Validation

## Commands

```text
git diff --check
rustfmt --check crates/clankers-agent/src/turn/mod.rs crates/clankers-agent/src/turn/steel_execution.rs tests/embedded_controller.rs scripts/check-steel-agent-turn-wiring.rs scripts/check-steel-turn-planning-runtime-smoke.rs
TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= cargo test -p clankers-controller --test fcis_shell_boundaries
nix run .#cairn -- gate proposal steel-executor-run-receipt --root .
nix run .#cairn -- gate design steel-executor-run-receipt --root .
nix run .#cairn -- gate tasks steel-executor-run-receipt --root .
nix run .#cairn -- validate --root .
nix run .#cairn -- gate tasks steel-executor-run-receipt --root .  # after checking V2
nix run .#cairn -- validate --root .                              # after checking V2
nix run .#cairn -- sync steel-executor-run-receipt --root . --execute
nix run .#cairn -- validate --root .
# canonical spec read/repair: retained r[steel-executor-run-receipt.execution-receipt] in cairn/specs/agent-loop/spec.md
nix run .#cairn -- validate --root .
nix run .#cairn -- archive steel-executor-run-receipt --root . --execute
nix run .#cairn -- validate --root .
```

## Results

```text
git diff --check: STATUS 0
rustfmt --check: STATUS 0
fcis_shell_boundaries: 35 passed; 0 failed; STATUS 0
proposal gate: PASS, STATUS 0
design gate: PASS, STATUS 0
tasks gate: PASS, STATUS 0
validate: {"valid": true, "changes": 1, "change_issues": [], "spec_issues": [], "specs_validated": 48}, STATUS 0
post-V2 tasks gate: PASS, STATUS 0
post-V2 validate: {"valid": true, "changes": 1, "change_issues": [], "spec_issues": [], "specs_validated": 48}, STATUS 0
sync --execute: mutated true, STATUS 0
post-sync validate: {"valid": true, "changes": 1, "change_issues": [], "spec_issues": [], "specs_validated": 48}, STATUS 0
canonical spec repair: execution-receipt requirement present under agent-loop
post-repair validate: {"valid": true, "changes": 1, "change_issues": [], "spec_issues": [], "specs_validated": 48}, STATUS 0
archive --execute: mutated true, STATUS 0
post-archive validate: {"valid": true, "changes": 0, "change_issues": [], "spec_issues": [], "specs_validated": 47}, STATUS 0
```
