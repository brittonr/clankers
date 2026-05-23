## Context

The change adds deterministic check coverage for a parser boundary. The review evidence requires fixture-backed verification so regressions can be reproduced without live services.

## Decisions

### 1. Deterministic check coverage

The implementation must add deterministic check coverage for the parser boundary and keep the coverage tied to a reproducible artifact.
