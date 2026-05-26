# Change: BG-process TUI dogfood docs checker

## Why

The BG-process TUI dogfood rail is now part of `full`; its docs and receipt criteria should not drift from the executable rail. A narrow checker can fail fast when docs omit required fields or describe stale behavior.

## What Changes

- Add or extend a repo-owned checker that validates documented BG-process TUI dogfood command and required receipt fields.
- Cover README and release-readiness docs, or a single canonical docs page plus links.
- Add negative fixture or test coverage for omitted required receipt fields.

## Non-Goals

- No change to the dogfood rail behavior unless the checker exposes a real mismatch.
- No broad docs rewrite.
- No new live model dependency.
