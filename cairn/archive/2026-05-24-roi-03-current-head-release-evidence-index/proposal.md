# Change: Current-HEAD release evidence index

## Why

After a fresh current-HEAD full harness receipt exists, operators need a small checked-in index that points at the verified payload, receipt paths, readiness history, and tag boundary without moving readiness tags or committing raw logs.

## What Changes

- Create or refresh a docs evidence index for the current verified payload.
- Summarize harness run id, payload commit, status, and local receipt paths.
- State the readiness tag boundary explicitly and avoid self-referential docs-commit claims.

## Non-Goals

- No readiness tag movement unless separately requested.
- No raw generated receipt/log publication.
- No claim that docs commit itself was the harness payload when the payload is the prior clean commit.
