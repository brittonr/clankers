Evidence-ID: cairn-validation
Artifact-Type: test-report
Task-ID: V3
Covers: r[steel-core-agent-loop.executor-selection], r[steel-core-agent-loop.fail-closed], r[steel-core-agent-loop.no-ambient-authority], r[steel-core-agent-loop.receipts]
Created: 2026-05-30
Status: complete

# Cairn Validation

## Commands

```text
nix run .#cairn -- gate proposal steel-core-agent-loop --root .
nix run .#cairn -- gate design steel-core-agent-loop --root .
nix run .#cairn -- gate tasks steel-core-agent-loop --root .
nix run .#cairn -- validate --root .
nix run .#cairn -- sync steel-core-agent-loop --root . --execute
nix run .#cairn -- validate --root .
nix run .#cairn -- archive steel-core-agent-loop --root . --execute
nix run .#cairn -- validate --root .
```

## Results

```text
proposal gate: PASS, STATUS 0
design gate: PASS, STATUS 0
tasks gate: PASS, STATUS 0
validate: {"valid": true, "changes": 1, "change_issues": [], "spec_issues": [], "specs_validated": 47}, STATUS 0
sync --execute: mutated true, STATUS 0
post-sync validate: {"valid": true, "changes": 1, "change_issues": [], "spec_issues": [], "specs_validated": 48}, STATUS 0
archive --execute: mutated true, STATUS 0
post-archive validate: {"valid": true, "changes": 0, "change_issues": [], "spec_issues": [], "specs_validated": 47}, STATUS 0
```
