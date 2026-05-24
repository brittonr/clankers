# Change: Current-HEAD full harness receipt

## Why

The latest full harness receipt predates the two Steel eval commits now on `main`, so release/readiness claims need a fresh full-mode receipt bound to the current payload commit.

## What Changes

- Generate a fresh `./scripts/test-harness.sh full` receipt from a clean, aligned `main`.
- Require the receipt to bind `payload.commit` to the intended current HEAD and report no failed steps.
- Keep the generated raw harness artifacts under `target/` unless a later evidence-index slice explicitly publishes selected metadata.

## Non-Goals

- No readiness tag movement.
- No checked-in generated harness logs in this slice.
- No broad feature work while refreshing the receipt.
