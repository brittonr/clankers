## Context

Clankers desktop behavior intentionally uses `~/.clankers/agent`, project `.clankers`, auth stores, JSONL sessions, cache DBs, and plugin roots. Embedding requires these to be host-owned choices rather than ambient assumptions.

## Decisions

### Host-owned service interfaces

**Choice:** Define injectable service interfaces/config structs for stores and resolvers instead of reading global paths at arbitrary layers.

**Rationale:** Host apps can use in-memory, app-database, sandboxed, or existing Clankers filesystem backends explicitly.

### CLI defaults remain adapters

**Choice:** Preserve current path conventions behind default adapter constructors.

**Rationale:** This avoids breaking terminal users while making dependency ownership explicit for embedders.

### Fail closed on missing injected services

**Choice:** Features that require absent services should be unpublished or return explicit unsupported errors, not silently fall back to global state.

**Rationale:** Hidden fallback to the user's machine state is dangerous in embedded applications.

## Risks / Trade-offs

- **Trait sprawl:** Keep the first slice to services actually needed by runtime construction and prompt execution.
- **Auth complexity:** Provider/router auth already has several stores; start with a credential-source boundary and preserve existing CLI behavior as one adapter.
- **Migration churn:** Use adapter constructors so existing code can move incrementally.
