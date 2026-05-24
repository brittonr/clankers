# Design: Current-HEAD release evidence index

## Context

After a fresh current-HEAD full harness receipt exists, operators need a small checked-in index that points at the verified payload, receipt paths, readiness history, and tag boundary without moving readiness tags or committing raw logs.

## Approach

- Generate evidence only after the full harness payload is clean and synced.
- The docs page records the evaluated payload commit and later evidence-recording commit separately or omits self-referential final hashes.
- Verification checks that the index references an existing receipt whose payload matches the stated commit.

## Verification

- Validate this Cairn package with repo-local/native Cairn validation.
- Run proposal, design, and tasks gates and inspect `valid`/`verdict` receipts.
- Run the implementation-specific verification named in `tasks.md` when draining this package.
