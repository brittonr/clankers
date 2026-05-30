Evidence-ID: cairn-validation
Artifact-Type: test-report
Task-ID: V2
Covers: r[nextest-self-evolution-isolation.verification.focused-rails]
Created: 2026-05-30
Status: complete

# Cairn Validation

## Commands

```text
nix run .#cairn -- gate proposal nextest-self-evolution-isolation --root .
nix run .#cairn -- gate design nextest-self-evolution-isolation --root .
nix run .#cairn -- gate tasks nextest-self-evolution-isolation --root .
nix run .#cairn -- validate --root .
```

## Results

```text
proposal gate: PASS, STATUS 0
design gate: PASS, STATUS 0
tasks gate: PASS, STATUS 0
validate: {"valid": true, "changes": 1, "change_issues": [], "spec_issues": [], "specs_validated": 46}, STATUS 0
```
