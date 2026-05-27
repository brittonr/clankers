# Final validation evidence

Evidence-ID: final-validation
Artifact-Type: command-output-summary
Task-ID: V5
Covers: steel-default-orchestration.policy-selected-default.default-selected, steel-turn-planning-config-activation.settings-surface.absent-default
Date: 2026-05-27
Status: PASS

## Command bundle

Pueue task 33 ran the archive validation bundle. Pueue task 13 reran focused tests, docs, `cairn validate`, and `git diff --check` after the V3 review follow-up.

Original command bundle:

```text
TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= cargo test -p clankers-config steel_turn_planning --lib
TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= cargo test -p clankers-agent turn::steel_planning --lib
CC=gcc CXX=g++ CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_LINKER=gcc RUSTFLAGS='-C link-arg=-fuse-ld=bfd' RUSTC_WRAPPER= CARGO_TARGET_DIR=target/steel-runtime-smoke-test cargo test -p clankers steel_runtime_smoke --test embedded_controller
./scripts/check-steel-turn-planning-runtime-smoke.rs
./scripts/check-steel-turn-planning-config-activation.rs
./scripts/check-steel-default-orchestration.rs
./scripts/check-steel-agent-turn-wiring.rs
./scripts/check-steel-turn-planning-ucan-authority.rs
mdbook build docs
nix run .#cairn -- gate proposal make-steel-turn-planning-default --root .
nix run .#cairn -- gate design make-steel-turn-planning-default --root .
nix run .#cairn -- gate tasks make-steel-turn-planning-default --root .
nix run .#cairn -- validate --root .
git diff --check
```

## Relevant output

```text
clankers-config steel_turn_planning: 5 passed
clankers-agent turn::steel_planning: 19 passed
embedded_controller steel_runtime_smoke: 5 passed
steel turn planning runtime smoke receipt written to target/steel-turn-planning-runtime-smoke/receipt.json
steel turn planning config activation receipt written to target/steel-turn-planning-config-activation/receipt.json
steel default orchestration receipt written to target/steel-default-orchestration/profile-receipt.json
steel agent turn wiring receipt written to target/steel-agent-turn-wiring/receipt.json
steel turn planning UCAN authority receipt written to target/steel-turn-planning-ucan-authority/receipt.json
INFO HTML book written to `/home/brittonr/git/clankers/docs/book`
proposal gate: valid=true verdict=PASS issues=[]
design gate: valid=true verdict=PASS issues=[]
tasks gate: valid=true verdict=PASS issues=[]
validate: valid=true issues=[] change_issues=[] spec_issues=[] specs_validated=106 after archive/follow-up
git diff --check: pass
```
