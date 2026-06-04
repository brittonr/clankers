# Validation closeout evidence

Evidence-ID: classify-runtime-sdk-facade.validation-closeout
Artifact-Type: command-output-summary
Task-ID: V2,V3
Covers: remaining-coupling-drain.runtime-facade-classification, remaining-coupling-drain.runtime-facade-classification.owner-map, remaining-coupling-drain.runtime-facade-classification.promotion-gate, remaining-coupling-drain.runtime-public-api-rail, remaining-coupling-drain.runtime-public-api-rail.leakage, remaining-coupling-drain.runtime-public-api-rail.deterministic, remaining-coupling-drain.runtime-fail-closed-defaults, remaining-coupling-drain.runtime-fail-closed-defaults.no-ambient
Date: 2026-06-04
Status: PASS

## Commands completed

```text
scripts/check-runtime-facade-boundary.rs
cargo test -p clankers-runtime --lib public_api_boundary_rejects_transport_type_leakage
scripts/check-runtime-extension-service-matrix.rs
scripts/check-config-prompt-skill-services.rs
scripts/check-provider-router-boundary.rs
scripts/check-behavioral-lego-rails.rs
scripts/emit-embedded-sdk-release-receipt.rs
env TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= scripts/check-embedded-agent-sdk.rs
git diff --check
nix run .#cairn -- validate --root .
nix run .#cairn -- gate proposal classify-runtime-sdk-facade --root .
nix run .#cairn -- gate design classify-runtime-sdk-facade --root .
nix run .#cairn -- gate tasks classify-runtime-sdk-facade --root .
```

## Relevant output

```text
scripts/check-runtime-facade-boundary.rs
ok: runtime facade boundary inventories clankers-runtime public API and dependency classifications
exit=0

cargo test -p clankers-runtime --lib public_api_boundary_rejects_transport_type_leakage
running 1 test
test tests::public_api_boundary_rejects_transport_type_leakage ... ok
exit=0

scripts/check-runtime-extension-service-matrix.rs
runtime extension service matrix receipt written to target/embedded-sdk-release/runtime-extension-service-matrix-receipt.json
exit=0

scripts/check-config-prompt-skill-services.rs
config-prompt-skill-services receipt written to target/embedded-sdk-release/config-prompt-skill-services-receipt.json
exit=0

scripts/check-provider-router-boundary.rs
ok: provider/router boundary rail passed
exit=0

scripts/check-behavioral-lego-rails.rs
behavioral lego rail inventory receipt written to target/embedded-sdk-release/behavioral-rail-inventory-receipt.json
exit=0

scripts/emit-embedded-sdk-release-receipt.rs
embedded SDK release receipt written to target/embedded-sdk-release/receipt.json
exit=0

pueue task 13: classify-runtime-sdk-facade-acceptance
command: env TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= scripts/check-embedded-agent-sdk.rs
start: Thu, 4 Jun 2026 11:39:00 -0400
end: Thu, 4 Jun 2026 11:46:09 -0400
embedded-agent-sdk acceptance passed
exit=0

git diff --check
exit=0

nix run .#cairn -- validate --root .
valid=true
changes=2
specs_validated=125
exit=0

nix run .#cairn -- gate proposal classify-runtime-sdk-facade --root .
verdict=PASS
exit=0

nix run .#cairn -- gate design classify-runtime-sdk-facade --root .
verdict=PASS
exit=0

nix run .#cairn -- gate tasks classify-runtime-sdk-facade --root .
verdict=PASS
exit=0
```

## Closeout note

After recording evidence and checking tasks complete, final `git diff --check`, Cairn validation, and Cairn proposal/design/tasks gates were rerun so the evidence packet is proven after the last evidence/task edit.
