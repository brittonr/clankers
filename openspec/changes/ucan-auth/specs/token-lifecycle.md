# Token Lifecycle

## Purpose

Define how tokens are created, distributed, verified, and revoked.
Adapts aspen-auth's token machinery for clankers' transports (Matrix and iroh).

## Requirements

### Token format

Tokens MUST use the same structure as aspen-auth's `CapabilityToken`:

- `version: u8` — protocol version
- `issuer: PublicKey` — Ed25519 key that signed the token (iroh identity)
- `audience: Audience` — `Key(PublicKey)` or `Bearer`
- `capabilities: Vec<Capability>` — clankers-specific capabilities
- `issued_at: u64` — Unix timestamp
- `expires_at: u64` — Unix timestamp
- `nonce: Option<[u8; 16]>` — for revocation
- `proof: Option<[u8; 32]>` — BLAKE3 hash of parent token (delegation chain)
- `delegation_depth: u8` — max 8 levels
- `signature: [u8; 64]` — Ed25519 signature

### Token encoding

Tokens MUST support:
- Binary: postcard encoding (compact, for iroh transport)
- Base64: URL-safe no-pad (for Matrix messages, CLI output, config files)

### Token creation via CLI

The daemon MUST provide CLI commands for token management:

```
clankers token create                          # interactive, full access
clankers token create --tools "read,grep,find" # read-only tools
clankers token create --tools "*" --expire 24h # full tools, 24hr expiry
clankers token create --read-only              # shorthand: read/grep/find/ls, no bash/write/edit
clankers token create --for <pubkey>           # audience-locked to specific identity
clankers token create --from <parent-token>    # delegated from parent
clankers token list                            # list issued tokens (from redb)
clankers token revoke <token-hash>             # revoke by hash
clankers token info <base64-token>             # decode and print token details
```

GIVEN the user runs `clankers token create --tools "read,grep" --expire 1h`
WHEN the token is created
THEN it contains `Prompt` + `ToolUse { "read,grep" }` capabilities
AND expires in 1 hour
AND is printed as base64 to stdout
AND is stored in redb for tracking

### Token distribution over Matrix

Users MUST be able to send a token to the bot via Matrix DM.  The bot
recognizes and stores it.

#### Registration message

The bot MUST recognize messages matching `!token <base64-token>` as
token registration attempts.

GIVEN a user sends `!token eyJ...` in a Matrix room
WHEN the bot receives the message
THEN it decodes and verifies the token
AND if valid, stores it mapped to the sender's Matrix user ID
AND replies with "Token accepted — capabilities: Prompt, ToolUse(read,grep)"

GIVEN a user sends `!token <invalid-base64>`
WHEN the bot attempts to decode
THEN it replies with "Invalid token: <reason>" (expired, bad signature, etc.)

#### Token storage

The daemon MUST persist user→token mappings in redb so they survive restarts.

Table: `matrix_tokens` — key: Matrix user ID, value: encoded token bytes

#### Token precedence

When a user has both an allowlist entry AND a token:
- Token capabilities are used for authorization (more granular)
- Allowlist entry is treated as "implicit full-access token"

When a user has neither:
- Message is silently rejected (same as current allowlist behavior)

### Token verification on message receipt

The daemon MUST verify the sender's token on every incoming message:

1. Look up token by sender identity (Matrix user ID or iroh pubkey)
2. If no token found, fall back to allowlist check
3. If token found, verify: signature, expiry, revocation
4. If token invalid (expired, revoked), reject with error message
5. If token valid, extract capabilities for the session

GIVEN a user with an expired token sends a message
WHEN the daemon checks the token
THEN it replies "Your access token has expired. Request a new one from the daemon owner."
AND the message is not forwarded to the agent

### Per-operation authorization

The daemon MUST check capabilities before executing operations:

- Before prompting the agent: check `Prompt`
- Before each tool call: check `ToolUse` against tool name
- Before bash execution: check `ShellExecute` against command
- Before file read/write: check `FileAccess` against path
- Before bot commands: check `BotCommand` against command name
- Before session management: check `SessionManage`
- Before model switch: check `ModelSwitch`

GIVEN a user with `ToolUse { "read,grep,find" }` sends a prompt
WHEN the agent attempts to call the `bash` tool
THEN the tool call is rejected with "Permission denied: tool 'bash' not authorized"
AND the agent receives the denial as a tool error (so it can adapt)

### Tool filtering in agent context

When a user has a `ToolUse` capability that doesn't include all tools,
the daemon SHOULD create the agent with only the authorized tools.
This prevents the agent from even attempting unauthorized tool calls.

GIVEN a token with `ToolUse { "read,grep,find,ls" }`
WHEN the daemon creates an agent for this session
THEN only `read`, `grep`, `find`, `ls` tools are registered with the agent

### Delegation

Token holders with the `Delegate` capability MUST be able to create
child tokens via Matrix:

```
!delegate --tools "read,grep" --expire 1h
```

The bot creates a child token attenuated from the sender's token and
replies with the base64-encoded child token.

GIVEN Alice has a token with `[Prompt, ToolUse("*"), Delegate]`
WHEN Alice sends `!delegate --tools "read,grep" --expire 1h`
THEN a child token is created with `[Prompt, ToolUse("read,grep")]`
AND `delegation_depth` is incremented
AND the child token's `proof` field contains the hash of Alice's token
AND the base64 token is sent back to Alice

GIVEN Bob has a token WITHOUT `Delegate`
WHEN Bob sends `!delegate --tools "read"`
THEN the bot replies "Delegation not authorized"

### Revocation

Revoked token hashes MUST be persisted in redb and checked during
verification.

Table: `revoked_tokens` — key: token hash ([u8; 32]), value: revocation timestamp

The daemon owner can revoke via CLI:
```
clankers token revoke <hash>
```

Revoking a parent token SHOULD implicitly invalidate all child tokens
(since the delegation chain is broken).

### Iroh transport

For iroh connections, the token MUST be sent as the first frame on a
new `clankers/chat/1` stream:

```json
{ "type": "auth", "token": "<base64-token>" }
```

The daemon verifies before accepting any prompts on that stream.

GIVEN an iroh peer connects without sending an auth frame
WHEN the daemon receives a prompt frame
THEN it falls back to the allowlist check (backwards compatible)
