# Metrics snapshot: deterministic-check task omissions

- Artifact-Type: sanitized-review-metrics-snapshot
- Date: 2026-05-23
- Selected category: `omission|tasks|deterministic-check`
- Count: 151
- Sources: `cairn-gate=151`
- Stages: `tasks=151`

## Safe representative examples

1. Deterministic request-fixture coverage is incomplete for entitlement probes on retry/refresh paths. The task ledger covers initial/transient/401 paths for normal requests and covers entitlement-probe contract once, but does not require transient-retry or 401 refresh-retry probe fixtures.
2. A probe task requires headers/body checks but does not explicitly assert absence of normal-request-only transport headers, leaving a deterministic transport difference unguarded.
3. A stream parser task names a raw event family but omits a task requiring fixture coverage for the raw delta event path.

## Selected prevention rail

Future task ledgers that invoke deterministic fixture/check coverage must name a concrete fixture, helper, command, golden file, script, evidence path, or oracle checkpoint on a task line with `[covers=...]`. Vague text such as "add deterministic tests" or "verify fixture coverage" is insufficient.

## Sanitization boundary

This snapshot contains counts, classes, sanitized path-style examples, and category summaries only. It does not include credentials, tokens, account identifiers, raw hidden prompts, provider payload bodies, or private transcript dumps.
