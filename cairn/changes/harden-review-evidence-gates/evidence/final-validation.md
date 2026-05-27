# Final validation evidence

Evidence-ID: final-validation
Artifact-Type: command-output-summary
Task-ID: V2
Covers: openspec-review-gates.spec-stage-omission-prevention.strong-constraint-spec, openspec-review-gates.spec-stage-omission-prevention.strong-constraint-spec-satisfied
Date: 2026-05-27
Status: PASS

## Commands

```text
mdbook build docs
nix run .#cairn -- gate proposal harden-review-evidence-gates --root .
nix run .#cairn -- gate design harden-review-evidence-gates --root .
nix run .#cairn -- gate tasks harden-review-evidence-gates --root .
nix run .#cairn -- validate --root .
git diff --cached --check
git diff --check
```

## Relevant output

```text
INFO HTML book written to `/home/brittonr/git/clankers/docs/book`
proposal gate: valid=true verdict=PASS issues=[]
design gate: valid=true verdict=PASS issues=[]
tasks gate: valid=true verdict=PASS issues=[]
validate: valid=true issues=[] change_issues=[] spec_issues=[] specs_validated=106
git diff --cached --check: pass
git diff --check: pass
```

## Notes

The validation ran after the review-gate checker, paired strong-constraint fixtures, operator guidance, and active Cairn change artifacts were present in the working tree.
