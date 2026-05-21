# Proposal

## Why

The provider change adds Codex response handling where reasoning signature retention, retry policy bounds, and a scenario-complete verification plan are release-critical.

## Verification

Acceptance evidence must cover reasoning signature retention across later turns, bounded 429/5xx retries, exactly one 401 refresh retry, proactive refresh, provider-scoped status, and discovery hiding when credentials are absent.
