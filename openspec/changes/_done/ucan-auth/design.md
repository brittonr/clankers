# ucan-auth — Design

## Status

All phases complete. UCAN auth is fully implemented.

## Decisions

### Fork aspen-auth rather than depend on it

**Choice:** Copy and adapt the relevant modules into `crates/clankers-auth/`
**Rationale:** aspen-auth has ~400 lines of capability/operation types specific
to a KV store, secrets engine, transit engine, and PKI — none of which apply
to clankers.  The generic machinery (token, builder, verifier) is ~600 lines
and stable.  A fork avoids pulling in aspen's full dependency tree and lets
the capability types diverge freely.
**Alternatives considered:** Depend on aspen-auth with feature flags.  Would
require making aspen-auth's `Capability` enum generic, which changes its API
for aspen's benefit with no gain.

### Credential type needed for delegation chains

**Choice:** Port `Credential` from aspen-auth (token + proof chain bundle)
**Rationale:** The current implementation stores bare `CapabilityToken` per
user. Delegated tokens need their parent chain to be verifiable — without
`Credential`, the daemon must maintain a parent token cache (stateful, fragile).
`Credential` is self-contained: the verifier walks the chain offline using
only the credential contents. This also makes `!delegate` work properly —
the child receives the full chain, not just a leaf token that can't be
verified without the daemon already having the parent cached.
**Source:** `aspen-auth/src/credential.rs` — 133 lines + 200 lines tests.
**Impact:** `AuthLayer.store_token()` and `lookup_token()` change from bare
token to credential. `!token` and `!delegate` commands switch to credential
encoding. QUIC handshake sends credential instead of bare token.

### Allowlist as implicit full-access token

**Choice:** Users on the flat allowlist are treated as having a root token
**Rationale:** Backwards compatible.  Simple setups (single user, no tokens)
keep working exactly as before.  The allowlist is the "zero-config" path;
tokens are the "I need more control" path.
**Alternatives considered:** Require tokens for everyone (breaking change),
or make allowlist and tokens completely separate systems (confusing).

### Tool filtering at agent creation, not tool execution

**Choice:** When a user has restricted `ToolUse`, create the agent with only
those tools registered — don't register all tools and then deny at call time.
**Rationale:** The agent never sees unauthorized tools in its tool list, so
it won't attempt to call them.  This is more efficient (no wasted API tokens
on denied calls) and produces better UX (agent plans around available tools).
**Alternatives considered:** Register all tools, deny at execution.  Wastes
model tokens and produces confusing "permission denied" errors mid-turn.

### Token registration via `!token` bot command

**Choice:** Users send their token as `!token <base64>` in the Matrix room
**Rationale:** No separate registration API needed.  Works from any Matrix
client.  The token is verified immediately and the user gets feedback.
**Alternatives considered:** Config file per user (requires daemon restart),
HTTP endpoint (requires a web server), DM-only (limits where tokens work).

### Iroh auth frame is optional (backwards compatible)

**Choice:** If an iroh peer connects and sends a prompt without an auth frame,
fall back to the allowlist.  Auth frame is opt-in.
**Rationale:** Existing iroh clients (CLI, other clankers instances) don't
send auth frames.  Breaking them would be bad.  New clients that want
capability-scoped access send the auth frame first.
**Alternatives considered:** Require auth frame always (breaking), negotiate
via ALPN (new `clankers/chat/2` — too much ceremony for now).

## Architecture

```
┌──────────────────────────────────────────────────────────┐
│                      Daemon                              │
│                                                          │
│  ┌────────────────────────────────────────────────────┐  │
│  │              TokenVerifier                          │  │
│  │  trusted_root: owner's public key                   │  │
│  │  revoked: HashSet<[u8;32]> (from redb on startup)  │  │
│  └────────────┬───────────────────────────────────────┘  │
│               │                                          │
│  ┌────────────▼───────────────────────────────────────┐  │
│  │           Auth Layer (per message)                  │  │
│  │                                                     │  │
│  │  1. Lookup token by sender ID (redb: auth_tokens)   │  │
│  │  2. If no token → allowlist fallback                │  │
│  │  3. If token → verify(signature, expiry, revoked)   │  │
│  │  4. Extract capabilities                            │  │
│  │  5. Filter tools for this session                   │  │
│  └────────────┬───────────────────────────────────────┘  │
│               │                                          │
│  ┌────────────▼───────────────────────────────────────┐  │
│  │           SessionStore                              │  │
│  │  sessions now include: capabilities: Vec<Capability>│  │
│  │  agent created with filtered tool set               │  │
│  └────────────────────────────────────────────────────┘  │
└──────────────────────────────────────────────────────────┘

Token flow:

  Owner                         User                        Daemon
    │                             │                           │
    │  clankers token create      │                           │
    │  --tools "read,grep"        │                           │
    │  --expire 24h               │                           │
    │  → prints base64 token      │                           │
    │                             │                           │
    │  (out of band: email/DM)    │                           │
    │  ─── token ──────────────>  │                           │
    │                             │                           │
    │                             │  !token eyJ...            │
    │                             │  ─────────────────────>   │
    │                             │                           │  verify()
    │                             │                           │  store in redb
    │                             │  <─── "Token accepted"    │
    │                             │                           │
    │                             │  "explain this codebase"  │
    │                             │  ─────────────────────>   │
    │                             │                           │  lookup token
    │                             │                           │  filter tools
    │                             │                           │  create agent
    │                             │                           │  (read,grep only)
    │                             │  <─── response            │
```

## Data Flow

### Authorization check (per message)

```
Message arrives from Matrix/iroh
  │
  ├─ Lookup sender in redb auth_tokens table
  │   ├─ Token found → verify(signature, expiry, revocation)
  │   │   ├─ Valid → extract capabilities, continue
  │   │   └─ Invalid → reply with error, stop
  │   │
  │   └─ No token → check allowlist
  │       ├─ On allowlist → full capabilities, continue
  │       └─ Not on allowlist → silent reject, stop
  │
  ├─ Bot command? → check BotCommand capability
  │
  ├─ Prompt? → check Prompt capability
  │   │
  │   └─ Create/reuse agent with filtered tool set
  │       │
  │       ├─ On each tool call: verify ToolUse + ShellExecute + FileAccess
  │       │   ├─ Authorized → execute
  │       │   └─ Denied → return error to agent ("permission denied: tool X")
  │       │
  │       └─ Return response
```

### Delegation check

```
User sends !delegate --tools "read" --expire 1h
  │
  ├─ Lookup sender's token
  ├─ Verify sender has Delegate capability
  ├─ Verify requested capabilities ⊆ sender's capabilities
  │   └─ contains() check on each requested capability
  ├─ Build child token with:
  │   - issuer = daemon's key (not the user's)
  │   - proof = hash of sender's token
  │   - delegation_depth = sender.depth + 1
  │   - attenuated capabilities
  ├─ Sign with daemon's secret key
  └─ Reply with base64 child token
```
