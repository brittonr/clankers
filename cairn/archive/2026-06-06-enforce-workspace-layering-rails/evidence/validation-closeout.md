# Validation closeout evidence

Evidence-ID: enforce-workspace-layering-rails.validation-closeout
Artifact-Type: command-output-summary
Task-ID: V2
Covers: remaining-coupling-drain.architecture-rail-hardening.workspace-layer-map
Date: 2026-06-06
Status: PASS

## Commands completed

```text
env TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= scripts/check-lego-architecture-boundaries.rs
git diff --check
nix run .#cairn -- validate --root .
nix run .#cairn -- gate proposal enforce-workspace-layering-rails --root .
nix run .#cairn -- gate design enforce-workspace-layering-rails --root .
nix run .#cairn -- gate tasks enforce-workspace-layering-rails --root .
```

## Relevant output

```text
scripts/check-lego-architecture-boundaries.rs
lego architecture dependency ownership inventory written to target/lego-architecture/dependency-ownership-inventory.json
exit=0

git diff --check
exit=0

nix run .#cairn -- validate --root .
valid=true
changes=7
specs_validated=130
exit=0

nix run .#cairn -- gate proposal enforce-workspace-layering-rails --root .
verdict=PASS
exit=0

nix run .#cairn -- gate design enforce-workspace-layering-rails --root .
verdict=PASS
exit=0

nix run .#cairn -- gate tasks enforce-workspace-layering-rails --root .
verdict=PASS
exit=0
```

## Closeout note

After evidence files were added, tasks were updated, and the architecture rail ran, final `git diff --check`, Cairn validation, and Cairn proposal/design/tasks gates were rerun. They are rerun once more after checking V2 before archive so the final evidence packet matches the checked task state.
