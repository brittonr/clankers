Evidence-ID: cairn-validation
Artifact-Type: test-report
Task-ID: V2
Covers: r[steel-executor-runtime-smoke.executor-visible]
Created: 2026-05-30
Status: complete

# Cairn Validation

## Commands

```text
git diff --check
rustfmt --check tests/embedded_controller.rs scripts/check-steel-turn-planning-runtime-smoke.rs
nix run .#cairn -- gate proposal steel-executor-runtime-smoke --root .
nix run .#cairn -- gate design steel-executor-runtime-smoke --root .
nix run .#cairn -- gate tasks steel-executor-runtime-smoke --root .
nix run .#cairn -- validate --root .
nix run .#cairn -- gate tasks steel-executor-runtime-smoke --root .  # after checking V2
nix run .#cairn -- validate --root .                              # after checking V2
nix run .#cairn -- sync steel-executor-runtime-smoke --root . --execute
nix run .#cairn -- validate --root .
# canonical spec read/repair: retained r[steel-executor-runtime-smoke.executor-visible] in cairn/specs/steel-turn-planning-runtime-smoke/spec.md
nix run .#cairn -- validate --root .
nix run .#cairn -- archive steel-executor-runtime-smoke --root . --execute
nix run .#cairn -- validate --root .
```

## Results

```text
git diff --check: STATUS 0
rustfmt --check: STATUS 0
proposal gate: PASS, STATUS 0
design gate: PASS, STATUS 0
tasks gate: PASS, STATUS 0
validate: {"valid": true, "changes": 1, "change_issues": [], "spec_issues": [], "specs_validated": 48}, STATUS 0
post-V2 tasks gate: PASS, STATUS 0
post-V2 validate: {"valid": true, "changes": 1, "change_issues": [], "spec_issues": [], "specs_validated": 48}, STATUS 0
sync --execute: mutated true, STATUS 0
post-sync validate: {"valid": true, "changes": 1, "change_issues": [], "spec_issues": [], "specs_validated": 48}, STATUS 0
canonical spec repair: executor-visible requirement present under steel-turn-planning-runtime-smoke
post-repair validate: {"valid": true, "changes": 1, "change_issues": [], "spec_issues": [], "specs_validated": 48}, STATUS 0
archive --execute: mutated true, STATUS 0
post-archive validate: {"valid": true, "changes": 0, "change_issues": [], "spec_issues": [], "specs_validated": 47}, STATUS 0
```
