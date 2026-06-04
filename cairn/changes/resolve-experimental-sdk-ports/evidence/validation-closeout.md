# Validation closeout evidence

Evidence-ID: resolve-experimental-sdk-ports.validation-closeout
Artifact-Type: command-output-summary
Task-ID: V2,V3
Covers: embedded-composition-kits.experimental-port-budget, embedded-composition-kits.experimental-port-budget.actionable, embedded-composition-kits.experimental-port-budget.hide-unused, neutral-tool-context.supported-service-ports, neutral-tool-context.supported-service-ports.fixtures, neutral-tool-context.supported-service-ports.docs
Date: 2026-06-04
Status: PASS

## Commands completed

```text
scripts/check-embedded-sdk-api.rs
scripts/check-experimental-sdk-port-budget.rs
scripts/check-brick-inventory-stability.rs
scripts/check-engine-host-feature-matrix.rs
scripts/check-tool-catalog-matrix.rs
env TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= scripts/check-embedded-agent-sdk.rs
git diff --check
nix run .#cairn -- validate --root .
nix run .#cairn -- gate proposal resolve-experimental-sdk-ports --root .
nix run .#cairn -- gate design resolve-experimental-sdk-ports --root .
nix run .#cairn -- gate tasks resolve-experimental-sdk-ports --root .
```

## Relevant output

```text
scripts/check-embedded-sdk-api.rs
ok: embedded SDK API inventory covers 586 public items (591 rows)
exit=0

scripts/check-experimental-sdk-port-budget.rs
ok: experimental SDK port budget covers 23 experimental rows; 137 promoted rows
exit=0

scripts/check-brick-inventory-stability.rs
brick-inventory-stability receipt written to target/embedded-sdk-release/brick-inventory-stability-receipt.json
exit=0

scripts/check-engine-host-feature-matrix.rs
exit=0

scripts/check-tool-catalog-matrix.rs
exit=0

pueue task 12: resolve-experimental-sdk-ports-acceptance-rerun
command: env TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= scripts/check-embedded-agent-sdk.rs
start: Thu, 4 Jun 2026 10:56:50 -0400
end: Thu, 4 Jun 2026 11:12:26 -0400
embedded-agent-sdk acceptance passed
exit=0

git diff --check
exit=0

nix run .#cairn -- validate --root .
valid=true
changes=3
specs_validated=127
exit=0

nix run .#cairn -- gate proposal resolve-experimental-sdk-ports --root .
verdict=PASS
exit=0

nix run .#cairn -- gate design resolve-experimental-sdk-ports --root .
verdict=PASS
exit=0

nix run .#cairn -- gate tasks resolve-experimental-sdk-ports --root .
verdict=PASS
exit=0
```

## Closeout note

After recording this evidence and checking V2/V3 complete, final Cairn validation and the tasks gate were rerun so the task evidence packet is proven after the last evidence/task edit.
