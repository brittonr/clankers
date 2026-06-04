## Context

`RuntimeServices::in_memory()` already disables extension services, and desktop plugin/provider execution require explicit injected runtimes. The remaining auth surfaces should follow the same policy.

## Goals

- Auth-store lookup and credential-pool selection are host-owned runtime extension services.
- Default construction must not touch auth files, pending verifier paths, OAuth refresh flows, or credential persistence.
- Injected desktop adapters should prove normal provider/account credential selection can be represented by safe receipts.

## Non-Goals

- Starting OAuth login flows.
- Persisting refreshed tokens or pending login verifiers through the runtime seam.
- Exposing credential values to receipts.

## Decisions

### Inject in-memory desktop auth state

**Choice:** Add an explicit constructor that accepts an injected `AuthStore` snapshot.
**Rationale:** Tests and embedders can prove behavior without filesystem reads/writes or live OAuth. Desktop CLI auth flows remain owned by existing command/provider code.

### Receipts over secrets

**Choice:** Return provider/account/kind/count/strategy/status metadata only.
**Rationale:** Runtime receipts are replay/debug evidence, not credential transport.

### Fail closed for mutation-oriented operations

**Choice:** `RefreshPersist` and `PendingLoginVerifier` remain unavailable in the read-only injected adapter.
**Rationale:** Those operations imply token writes or verifier-file state and need a later explicitly writable host contract.

## Risks / Trade-offs

- This is a read-only/auth-selection slice; live OAuth refresh persistence remains in existing desktop paths until a later writable contract is specified.
