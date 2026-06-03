Task-ID: I1,I2,I3,I4,V1,V2,V3
Covers: sdk-root-brick-extraction.inventory,sdk-root-brick-extraction.brick-owner.selected-cluster,sdk-root-brick-extraction.brick-owner.root-wiring-only,sdk-root-brick-extraction.rails.owner-receipts,sdk-root-brick-extraction.verification.brick-tests,sdk-root-brick-extraction.verification.root-parity,sdk-root-brick-extraction.verification
Artifact-Type: validation-evidence

# Root Brick Extraction Closeout

## Selected cluster

Selected cluster: `process-job-profile`.

The selected process-job profile policy is isolated behind focused root adapter modules:

- `src/tools/process/native.rs::NativeProcessJobService`
- `src/tools/process/pueue.rs::PueueProcessJobService`
- `src/tools/process/systemd.rs::SystemdProcessJobService`

Root `src/tools/process.rs` remains the product-shell tool registration/projection owner for process-job receipts.

## Rails / receipts

`scripts/check-root-brick-extraction.rs` records the root inventory and selected cluster owner receipt. `scripts/check-process-job-profile-kit.rs` verifies typed profile metadata and backend start receipt projection in all three backends.

## Validation

Focused rails/tests:

- `nix develop -c cargo -q -Zscript scripts/check-root-brick-extraction.rs`
- `nix develop -c cargo -q -Zscript scripts/check-process-job-profile-kit.rs`
- `nix develop -c cargo -q -Zscript scripts/check-behavioral-lego-rails.rs`

