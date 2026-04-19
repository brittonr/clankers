# Auth Crate Extraction (ucan-cap)

## Purpose

Extract the generic UCAN token machinery from `clankers-auth` into a
standalone capability token library. The token format, signing,
verification, delegation, and revocation logic (~600 lines) is generic
over capability types. The clankers-specific capability variants
(Prompt, ToolUse, FileAccess, etc.) stay in the workspace.

This extraction requires the most refactoring because `Capability` is
currently a concrete enum. It needs to become a trait or generic parameter.

## Requirements

### Crate identity

r[auth.identity.name]
The extracted crate MUST be named `ucan-cap` (or chosen alternative).

r[auth.identity.repo]
The crate MUST live in its own GitHub repository.

### Generalization

r[auth.generic.capability-trait]
The crate MUST define a trait that capability types implement:

```rust
pub trait Capability: Serialize + DeserializeOwned + Clone + Debug {
    /// Does this capability authorize the given operation?
    fn authorizes(&self, op: &Self::Operation) -> bool;

    /// Can this capability be delegated to produce `child`?
    /// Returns true if `child` is a subset of `self`.
    fn contains(&self, child: &Self) -> bool;
}
```

The associated `Operation` type lets each domain define its own
operation enum.

r[auth.generic.token]
`CapabilityToken<C: Capability>` MUST be generic over the capability type.
Fields: version, issuer, audience, capabilities, issued_at, expires_at,
nonce, proof, delegation_depth, signature.

r[auth.generic.builder]
`TokenBuilder<C: Capability>` MUST be generic. Usage:

```rust
let token = TokenBuilder::<MyCap>::new(secret_key)
    .with_capability(MyCap::Admin)
    .with_lifetime(Duration::from_secs(3600))
    .build()?;
```

r[auth.generic.verifier]
`TokenVerifier<C: Capability>` MUST be generic. It verifies signatures,
expiry, revocation, and delegation chains without knowing what the
capabilities mean.

r[auth.generic.revocation]
The `RevocationStore` trait MUST be part of the extracted crate:

```rust
pub trait RevocationStore: Send + Sync {
    fn is_revoked(&self, hash: &[u8; 32]) -> bool;
    fn revoke(&self, hash: [u8; 32], timestamp: u64);
    fn load_all(&self) -> Vec<[u8; 32]>;
}
```

Concrete implementations (redb, in-memory) can be provided as optional
features or left to consumers.

### Source migration

r[auth.source.generic-modules]
The following modules MUST move to the new crate:

- `token.rs` — `CapabilityToken<C>` (was concrete, now generic)
- `builder.rs` — `TokenBuilder<C>`
- `verifier.rs` — `TokenVerifier<C>`
- `constants.rs` — `MAX_DELEGATION_DEPTH`, `MAX_TOKEN_SIZE`, etc.
- `utils.rs` — `current_time_secs()`
- `error.rs` — `AuthError` (stripped of clankers-specific variants)

r[auth.source.stays-in-workspace]
The following MUST stay in `crates/clankers-auth/`:

- `capability.rs` — `ClankerCapability` enum (Prompt, ToolUse, etc.)
  implementing the new `Capability` trait
- `revocation.rs` — `RedbRevocationStore` (redb-specific impl)
- `tests.rs` — clankers-specific authorization matrix tests

r[auth.source.no-clankers-refs]
The source MUST NOT reference "clankers".

### Dependencies

r[auth.deps.iroh]
The crate MUST depend on `iroh` for Ed25519 identity (`PublicKey`,
`SecretKey`, `Signature`).

r[auth.deps.minimal]
Beyond iroh, the crate SHOULD depend only on: serde, postcard, blake3,
base64, thiserror. No tokio, no redb, no domain-specific crates.

### Delegation

r[auth.delegation.chain]
Token delegation MUST work by:
1. Child token's `proof` = BLAKE3 hash of parent token's bytes
2. `delegation_depth` increments (max: `MAX_DELEGATION_DEPTH`)
3. Each child capability MUST satisfy `parent_cap.contains(&child_cap)`

r[auth.delegation.no-escalation]
The builder MUST reject delegation attempts where any child capability
is not contained by a parent capability.

GIVEN a parent token with `ToolUse { pattern: "read,grep" }`
WHEN a child token requests `ToolUse { pattern: "*" }`
THEN the builder MUST return an error

### Tests

r[auth.tests.generic]
The extracted crate MUST include tests using a trivial test capability
type (e.g., `enum TestCap { Read, Write, Admin }`) proving:
- Token create/verify roundtrip
- Expired token rejection
- Revoked token rejection
- Delegation chain verification (2-level, 3-level)
- Delegation depth limit enforcement
- Escalation prevention

r[auth.tests.existing]
Clankers-specific tests (authorization matrix, glob patterns, etc.)
MUST continue to pass in `crates/clankers-auth/` using the extracted
crate's generic types with `ClankerCapability`.

### Workspace migration

r[auth.migration.workspace-crate]
After extraction, `crates/clankers-auth/` MUST depend on `ucan-cap`
via git dep and provide:
- `ClankerCapability` implementing `Capability`
- `ClankerOperation` implementing the associated `Operation` type
- `RedbRevocationStore` implementing `RevocationStore`
- Type aliases: `type ClankerToken = CapabilityToken<ClankerCapability>`

r[auth.migration.callers-unchanged]
All 7 call sites MUST compile. Callers that reference `CapabilityToken`
directly will need to use the type alias or add the generic parameter.

r[auth.migration.workspace-builds]
`cargo check` and `cargo nextest run` MUST pass on the full workspace.
