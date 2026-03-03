# ucan-auth

## Intent

Replace the flat user allowlist with UCAN-inspired capability tokens so that
different users get different permission levels when talking to the daemon.
Today it's all-or-nothing: you're on the allowlist or you're not.  With
capability tokens, the daemon owner can say "Alice gets full access, Bob can
only use read-only tools, Carol gets 24 hours of access then it expires."

The `aspen-auth` crate (from `../aspen`) already implements this pattern on
top of iroh's Ed25519 identity.  We adapt it for clankers-specific operations
rather than reimplementing from scratch.

## Scope

### In Scope

- `clankers-auth` crate — adapted from aspen-auth with clankers-specific capabilities
- Capability types: Prompt, ToolUse, BotCommand, SessionManage, ModelSwitch, FileAccess
- Token generation CLI (`clankers token create`, `clankers token revoke`, `clankers token list`)
- Daemon integration — verify tokens on incoming messages (iroh and Matrix)
- Delegation — token holders create narrower child tokens
- Token transport over Matrix (send token in DM, bot stores it per-user)
- Revocation via redb

### Out of Scope

- Aspen's KV/Secrets/Transit/PKI capabilities (not relevant to clankers)
- Cluster admin operations (clankers is single-node)
- Changes to the TUI interactive mode (local user is always root)
- E2EE key verification (separate concern)
- OAuth integration (existing OAuth flow is for LLM provider auth, not daemon access)

## Approach

Fork the relevant parts of `aspen-auth` into a new `crates/clankers-auth/`
crate, stripping the KV/secrets/transit/PKI capabilities and adding
clankers-specific ones.  The token format, signing, verification, and
delegation machinery stay the same — they're generic over capability types.

The daemon's allowlist check becomes a token verification + authorization
check.  The flat allowlist remains as a fallback for simple setups (no token
required if you're on the allowlist).
