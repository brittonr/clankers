# Design: Process Root Projection Final Drain

## Context

This is the follow-up to `drain-process-job-backend-adapters`: backend adapters now exist, but the final root-shell drain has not happened because `NativeProcessJobService` and `ProcessEntry` still live in the root file.

## Decisions

### 1. Move native service ownership before cosmetic test reshuffling

Move `NativeProcessJobService`, `ProcessEntry`, native receipt helpers, and native service tests as one ownership slice so the root file loses policy, not just lines.

### 2. Keep root projection explicit

Root may still construct service adapters and call `ProcessToolJsonAdapter`, but it should not define native lifecycle state or backend-specific receipt helpers.

### 3. Enforce with source and behavior rails

The boundary rail should name the allowed root responsibilities and fail with owner diagnostics when native service policy returns.

## Risks / Trade-offs

- Moving tests can hide behavior changes; keep focused native service and root projection tests green after each step.
- Native process globals are sensitive; avoid changing registry semantics while moving ownership.
- Root file size alone is not a correctness signal; use owner checks and behavior fixtures.
