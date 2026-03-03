# Auth Integration — aspen-auth for clankers

## Summary

Replace the flat allowlist and planned clankers-auth fork with direct use
of aspen-auth's UCAN capability token system.  Clankers-specific capability
types are added to aspen-auth's extensible capability model.

## Capability Types

Aspen-auth's existing `Capability` enum is extended with clankers-specific
variants via a `ClankerCapability` namespace:

```rust
/// Clankers-specific capabilities, stored in UCAN `att` field.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ClankerCapability {
    /// Can send prompts to the agent
    Prompt,

    /// Can use specific tools (empty = all tools)
    ToolUse { tools: Vec<String> },

    /// Can issue bot commands (!status, !restart, !skills, etc.)
    BotCommand,

    /// Can list, delete, resume, and manage sessions
    SessionManage,

    /// Can switch the active LLM model
    ModelSwitch,

    /// Can access files via read/write/edit tools
    /// Paths are optional — empty means unrestricted
    FileAccess { allowed_paths: Vec<String> },

    /// Can create child tokens with subset of own capabilities
    Delegate,

    /// Full access (equivalent to being on the old allowlist)
    Full,
}
```

## Containment Check

Delegation requires child capabilities ⊆ parent capabilities:

```rust
impl ClankerCapability {
    /// Does `self` contain (is a superset of) `other`?
    pub fn contains(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Full, _) => true,

            (Self::ToolUse { tools: parent }, Self::ToolUse { tools: child }) => {
                child.iter().all(|t| parent.contains(t))
            }

            (Self::FileAccess { allowed_paths: parent }, Self::FileAccess { allowed_paths: child }) => {
                if parent.is_empty() { return true; } // unrestricted parent
                child.iter().all(|cp| parent.iter().any(|pp| cp.starts_with(pp)))
            }

            (a, b) => a == b, // exact match for simple variants
        }
    }
}
```

## Token Verification Flow

```
Message arrives (iroh / Matrix / CLI)
  │
  ├─ Extract sender identity
  │   ├─ iroh: NodeId (Ed25519 pubkey from QUIC handshake)
  │   ├─ Matrix: user_id (@user:server)
  │   └─ CLI: local (implicit root)
  │
  ├─ Lookup token
  │   ├─ Cluster mode: aspen KV → clankers:auth:tokens:{sender-hash}
  │   ├─ Standalone mode: redb → auth_tokens table
  │   └─ CLI local: always root capabilities
  │
  ├─ If token found:
  │   ├─ Verify signature (Ed25519, aspen-auth's TokenVerifier)
  │   ├─ Check expiry (token.exp vs current time)
  │   ├─ Check revocation (clankers:auth:revoked:{token-hash})
  │   ├─ Extract ClankerCapability list
  │   └─ Return AuthResult::Authorized(capabilities)
  │
  ├─ If no token:
  │   ├─ Check allowlist (backwards compatibility)
  │   │   ├─ On list → AuthResult::Authorized(vec![Full])
  │   │   └─ Not on list → AuthResult::Denied
  │   └─ Check cluster cookie (aspen bootstrap auth)
  │
  └─ Apply capabilities to agent creation
      ├─ Filter tool set based on ToolUse capabilities
      ├─ Filter file paths based on FileAccess capabilities
      └─ Store capabilities in session for per-call checks
```

## Token Management

### CLI commands

```bash
# Create a token (owner only, uses daemon's secret key)
clankers token create \
  --capabilities "prompt,tool-use:read:grep:find,session-manage" \
  --expire 7d \
  --label "alice-readonly"

# List active tokens
clankers token list

# Revoke a token
clankers token revoke <token-hash>

# Show token details
clankers token inspect <base64-token>
```

### Bot commands (Matrix / iroh chat)

```
!token <base64>        — register a token for this sender
!token status          — show current capabilities
!delegate --tools read,grep --expire 1h  — create child token
```

### KV schema

```
clankers:auth:tokens:{sender-hash}       → Token (JSON, signed)
clankers:auth:revoked:{token-hash}       → revocation timestamp
clankers:auth:allowlist                  → JSON array of allowed identities
```

## Backwards Compatibility

1. **No token, on allowlist** → treated as `Full` capability (same as today)
2. **No token, not on allowlist** → denied (same as today)
3. **Token present** → capabilities from token (new behavior)
4. **Local TUI/CLI** → always `Full` (no auth needed for local user)

The allowlist remains the zero-config path.  Tokens are for teams and
shared daemons that need per-user granularity.
