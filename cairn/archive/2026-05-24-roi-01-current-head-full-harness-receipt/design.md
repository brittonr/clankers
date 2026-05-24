# Design: Current-HEAD full harness receipt

## Context

The latest full harness receipt predates the two Steel eval commits now on `main`, so release/readiness claims need a fresh full-mode receipt bound to the current payload commit.

## Approach

- Preflight checks record clean tracked status, branch/upstream alignment, and the current payload commit before the harness starts.
- The harness run is accepted only when `target/test-harness/results.json` and `summary.md` both report mode `full`, zero failures, and a payload commit equal to the preflight HEAD.
- If the full harness fails, preserve the failing receipt and rank the narrow failure repair before new feature work.

## Verification

- Validate this Cairn package with repo-local/native Cairn validation.
- Run proposal, design, and tasks gates and inspect `valid`/`verdict` receipts.
- Run the implementation-specific verification named in `tasks.md` when draining this package.
