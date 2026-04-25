# clanker-auth

UCAN-inspired capability tokens over iroh Ed25519 identity.

Generic token infrastructure for signing, verifying, and delegating capability
tokens. Define your own capability enum, implement the `Cap` trait, and get
token signing, verification, delegation chains, and revocation for free.

## Usage

```rust
use clanker_auth::{Cap, TokenBuilder, TokenVerifier};
use serde::{Serialize, Deserialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
enum MyCap { Read, Write, Admin, Delegate }

impl Cap for MyCap {
    type Operation = MyOp;
    fn authorizes(&self, op: &MyOp) -> bool { /* ... */ }
    fn contains(&self, child: &MyCap) -> bool { /* ... */ }
    fn is_delegate(&self) -> bool { matches!(self, MyCap::Delegate) }
}

// Create a token
let token = TokenBuilder::<MyCap>::new(secret_key)
    .with_capability(MyCap::Read)
    .with_lifetime(Duration::from_secs(3600))
    .build()?;

// Verify it
let verifier = TokenVerifier::<MyCap>::new();
verifier.verify(&token, None)?;
```

## Features

- **Generic capabilities** — define your own capability and operation types
- **Ed25519 signing** via iroh identity (PublicKey, SecretKey)
- **Delegation chains** with depth limits and escalation prevention
- **Revocation** with bounded in-memory lists
- **Audience enforcement** — bearer or key-scoped tokens
- **Trusted roots** — restrict to specific issuer public keys
- **postcard + BLAKE3** — compact binary encoding and fast hashing

## License

MIT
