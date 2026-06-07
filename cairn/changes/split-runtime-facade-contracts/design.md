# Design: Split Runtime Facade Contracts

## Context

The accepted runtime classification work identifies `clankers-runtime` as yellow-only or mixed rather than a pure green SDK crate. It still exposes useful contracts alongside executable runtime policy. The next coupling risk is docs or downstream code treating every runtime export as a stable green SDK contract.

## Decisions

### 1. Runtime exports are bucketed by authority

**Choice:** Each public export is classified as green contract, yellow host-injection surface, or desktop adapter shell.

**Rationale:** Authority, not convenience, determines SDK status. Serializable DTOs can be green; provider calls, auth stores, plugin runtime, process management, prompt filesystem discovery, and session storage are host/shell behavior.

### 2. Green contracts move or become no-authority modules

**Choice:** DTOs useful to embedded hosts move to neutral crates/modules without filesystem, process, network, provider, auth, plugin, session, clock, or Nickel/Steel execution authority.

**Rationale:** A green contract should compile and validate without ambient desktop services.

### 3. Runtime defaults remain fail-closed

**Choice:** Missing host services must return typed unavailable diagnostics and must not discover `.clankers`, `.pi`, global auth/config, daemon sockets, plugin directories, or session stores.

**Rationale:** Embedders need explicit service injection. Ambient desktop lookup is a coupling leak and a security footgun.

### 4. Generated docs follow the classification

**Choice:** SDK/generated docs list green contracts separately from yellow host-injection surfaces and desktop adapter shells.

**Rationale:** Documentation is part of the API contract. Users should not infer embeddability from a public Rust reexport alone.

## Risks / Trade-offs

- Moving DTOs can create broad import churn; staged reexports may be needed with clear deprecation notes.
- Some Steel orchestration data is reusable while execution is not; split data contracts before moving executable policy.
- Docs and generated API inventories must be refreshed together or reviews will see contradictory status.
