# Change: Dogfood full-readiness evidence page

## Why

The current HEAD is clean, synced, tagged as `internal-readiness-2026-05-26-dogfood-full`, and has a full harness receipt that includes BG-process TUI dogfood. A checked-in evidence page would make the checkpoint discoverable without committing raw `target/` receipts or moving tags.

## What Changes

- Add a docs reference page indexing the tag, target commit, full harness run, dogfood receipt facts, and scope boundaries.
- Link the page from the docs table of contents.
- Keep generated receipts under `target/` untracked and cite only stable paths/facts.

## Non-Goals

- No readiness tag movement.
- No raw harness log or generated receipt commit.
- No claim of public unattended production readiness.
