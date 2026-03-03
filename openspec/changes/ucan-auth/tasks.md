# ucan-auth — Tasks

Depends on: `matrix-daemon-v2` phase 1 (allowlist + bot commands must exist first)

## Phase 1: clankers-auth crate

- [ ] Create `crates/clankers-auth/Cargo.toml` with workspace deps
- [ ] Port `token.rs` from aspen-auth (unchanged)
- [ ] Port `builder.rs` from aspen-auth (unchanged)
- [ ] Port `verifier.rs` from aspen-auth (unchanged)
- [ ] Port `constants.rs` from aspen-auth (unchanged)
- [ ] Write `capability.rs` — Prompt, ToolUse, ShellExecute, FileAccess, BotCommand, SessionManage, ModelSwitch, Delegate
- [ ] Write `capability.rs` — `authorizes()` for each capability×operation pair
- [ ] Write `capability.rs` — `contains()` for delegation containment checks
- [ ] Write `error.rs` — strip aspen's KV/secrets variants, keep generic auth errors
- [ ] Write `revocation.rs` — `RevocationStore` trait + redb implementation
- [ ] Tests: token create/verify round-trip
- [ ] Tests: capability authorization matrix (each capability vs each operation)
- [ ] Tests: delegation containment (escalation prevention)
- [ ] Tests: expired/revoked/future token rejection
- [ ] Tests: delegation chain verification (depth 0 → 1 → 2)

## Phase 2: CLI

- [ ] `clankers token create` — interactive and flag-based token creation
- [ ] `clankers token create --tools <pattern>` — ToolUse scoping
- [ ] `clankers token create --read-only` — shorthand for read/grep/find/ls
- [ ] `clankers token create --expire <duration>` — time-bounded
- [ ] `clankers token create --for <pubkey>` — audience-locked
- [ ] `clankers token create --from <base64>` — delegated from parent
- [ ] `clankers token list` — list issued tokens from redb
- [ ] `clankers token revoke <hash>` — add to revocation table
- [ ] `clankers token info <base64>` — decode and print human-readable
- [ ] Store issued tokens in redb `auth_tokens` table for tracking

## Phase 3: Daemon integration

- [ ] Initialize `TokenVerifier` at daemon startup with owner's pubkey as trusted root
- [ ] Load revoked tokens from redb into verifier on startup
- [ ] Add `auth_tokens` redb table — user_id → encoded token
- [ ] Matrix: handle `!token <base64>` bot command — verify + store
- [ ] Matrix: lookup sender's token on each message, verify, extract capabilities
- [ ] Matrix: fall back to allowlist if no token (backwards compatible)
- [ ] Iroh: accept optional `{ "type": "auth", "token": "..." }` first frame
- [ ] Iroh: fall back to allowlist if no auth frame (backwards compatible)
- [ ] Filter tool set when creating agent for capability-restricted sessions
- [ ] Per-tool-call authorization check in the agent turn loop
- [ ] Return "permission denied" as tool error (agent can adapt)
- [ ] Expired token: reply with "Token expired, request a new one"

## Phase 4: Delegation via Matrix

- [ ] `!delegate` bot command — create child token from sender's token
- [ ] `!delegate --tools <pattern> --expire <duration>` — attenuated
- [ ] Validate Delegate capability on sender's token
- [ ] Validate containment (no escalation)
- [ ] Reply with base64 child token
- [ ] Register child token's parent hash in verifier cache
