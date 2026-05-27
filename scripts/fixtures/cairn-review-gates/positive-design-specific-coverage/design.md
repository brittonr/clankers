# Design

## Decisions

- Reasoning signature retention stores provider signatures with assistant reasoning metadata and reuses them on each later turn that resumes the same provider session.
- Retry policy bounds are concrete: use 3 retries for retryable 429/5xx failures with 1s/2s/4s backoff, exactly one 401 refresh retry, and one refresh cycle per top-level request.
- The scenario-complete verification plan includes deterministic fixtures for proactive refresh, 401 refresh retry, 429 retry exhaustion, provider-scoped status, and discovery hiding when credentials are absent.
