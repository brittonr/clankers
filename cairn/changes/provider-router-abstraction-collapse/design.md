# Design: Provider Router Abstraction Collapse

## Context

Prior convergence delegated some RPC request building to `router_request_bridge`, but `clankers-provider` still carries compatibility abstractions that need explicit convergence conditions.

## Decisions

### 1. One owner per concern

Each concern should identify whether `clanker-router`, a provider backend module, or `clankers-provider` compatibility owns policy.

### 2. Literal fixtures guard adapters

Request/stream/auth adapter tests should compare against explicit fixtures, not helper-generated expectations.

### 3. Compatibility can remain, policy cannot duplicate

A compatibility facade may stay if it only translates DTOs, errors, and stream events.

## Risks / Trade-offs

- Public API churn can break many call sites; use staged compatibility when needed.
- Auth flows are stateful; use provider-scoped fixtures and cleanup guards.
- Live provider behavior can drift; pin deterministic contract tests first.
