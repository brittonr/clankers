# Crate Structure

## Purpose

Define what goes into `crates/clankers-auth/` and how it relates to
aspen-auth.

## Requirements

### New crate: clankers-auth

The system MUST create `crates/clankers-auth/` with the following modules:

```
crates/clankers-auth/
├── Cargo.toml
└── src/
    ├── lib.rs           # Re-exports
    ├── capability.rs    # Capability + Operation enums (clankers-specific)
    ├── token.rs         # CapabilityToken struct (from aspen-auth, unchanged)
    ├── builder.rs       # TokenBuilder (from aspen-auth, unchanged)
    ├── verifier.rs      # TokenVerifier (from aspen-auth, unchanged)
    ├── error.rs         # AuthError enum
    ├── constants.rs     # MAX_DELEGATION_DEPTH, MAX_TOKEN_SIZE, etc.
    ├── revocation.rs    # RevocationStore trait + redb impl
    └── tests.rs
```

### Dependencies

```toml
[dependencies]
iroh            = { workspace = true }    # PublicKey, SecretKey, Signature
serde           = { workspace = true }
postcard        = { workspace = true }
blake3          = { workspace = true }
base64          = { workspace = true }
rand            = { workspace = true }

[dev-dependencies]
proptest        = { workspace = true }
```

### What to fork from aspen-auth

The following modules MUST be adapted from aspen-auth with minimal changes:

| aspen-auth module | clankers-auth | Changes |
|---|---|---|
| `token.rs` | `token.rs` | None — generic over capabilities |
| `builder.rs` | `builder.rs` | None — generic signing logic |
| `verifier.rs` | `verifier.rs` | None — generic verification logic |
| `error.rs` | `error.rs` | Remove KV/secrets/transit error variants |
| `constants.rs` | `constants.rs` | Same constants |
| `capability.rs` | `capability.rs` | Replace entirely with clankers capabilities |
| `revocation.rs` | `revocation.rs` | Replace KV store impl with redb impl |

### What NOT to include

The following aspen-auth modules MUST NOT be included:

- `hmac_auth.rs` — HMAC auth for internal cluster communication
- `verified_auth.rs` — Cluster-specific verified auth wrapper
- All `Secrets*`, `Transit*`, `Pki*` capability/operation variants
- `ClusterAdmin` capability
- `KeyValueRevocationStore` (replaced by redb)

### Redb revocation store

The revocation store MUST use clankers' existing redb database
(`~/.clankers/agent/clankers.db`):

```rust
impl RevocationStore for RedbRevocationStore {
    fn is_revoked(&self, hash: &[u8; 32]) -> bool;
    fn revoke(&self, hash: [u8; 32], timestamp: u64);
    fn load_all(&self) -> Vec<[u8; 32]>;
}
```

New redb table: `revoked_tokens` — `[u8; 32] -> u64` (hash -> timestamp)

### Token storage table

New redb table: `auth_tokens` — `String -> Vec<u8>` (user_id -> encoded token)

Used by the daemon to persist user→token mappings across restarts.

### Integration with clankers main crate

The daemon (`src/modes/daemon.rs`) MUST depend on `clankers-auth` and use
`TokenVerifier` in the message handling path.  The verifier is initialized
once at daemon startup with the owner's public key as the trusted root.

```rust
let verifier = TokenVerifier::new()
    .with_trusted_root(identity.public_key());
```
