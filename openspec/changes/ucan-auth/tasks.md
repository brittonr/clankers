# ucan-auth — Tasks

Depends on: `matrix-daemon-v2` phase 1 (allowlist + bot commands must exist first) ✓

## Phase 1: clankers-auth crate

- [x] Create `crates/clankers-auth/Cargo.toml` with workspace deps
- [x] Port `token.rs` from aspen-auth (unchanged)
- [x] Port `builder.rs` from aspen-auth (rewrite `generate_root_token()` for clankers caps)
- [x] Port `verifier.rs` from aspen-auth (unchanged)
- [x] Port `constants.rs` from aspen-auth (unchanged)
- [x] Port `utils.rs` from aspen-auth (`current_time_secs()`)
- [x] Write `capability.rs` — Prompt, ToolUse, ShellExecute, FileAccess, BotCommand, SessionManage, ModelSwitch, Delegate
- [x] Write `capability.rs` — `authorizes()` for each capability×operation pair
- [x] Write `capability.rs` — `contains()` for delegation containment checks
- [x] Write `capability.rs` — `glob_match()` helper for ShellExecute/ToolUse patterns
- [x] Write `error.rs` — AuthError with thiserror (matches aspen-auth style)
- [x] Write `revocation.rs` — `RevocationStore` trait + `RedbRevocationStore` (sync, not async)
- [x] Write `revocation.rs` — `AUTH_TOKENS_TABLE` + `REVOKED_TOKENS_TABLE` redb tables
- [x] Tests: token create/verify round-trip
- [x] Tests: capability authorization matrix (each capability vs each operation)
- [x] Tests: delegation containment (escalation prevention)
- [x] Tests: expired/revoked/future token rejection
- [x] Tests: delegation chain verification (depth 0 → 1 → 2)
- [x] Tests: `verify_with_chain()` stateless chain verification
- [x] Tests: glob_match patterns
- [x] Tests: pattern_contains for delegation

## Phase 2: CLI

- [x] `clankers token create` — interactive and flag-based token creation
- [x] `clankers token create --tools <pattern>` — ToolUse scoping
- [x] `clankers token create --read-only` — shorthand for read/grep/find/ls
- [x] `clankers token create --expire <duration>` — time-bounded
- [x] `clankers token create --for <pubkey>` — audience-locked
- [x] `clankers token create --from <base64>` — delegated from parent
- [x] `clankers token list` — list issued tokens from redb
- [x] `clankers token revoke <hash>` — add to revocation table
- [x] `clankers token info <base64>` — decode and print human-readable
- [x] Store issued tokens in redb `auth_tokens` table for tracking

## Phase 3: Daemon integration

- [x] Initialize `TokenVerifier` at daemon startup with owner's pubkey as trusted root
- [x] Load revoked tokens from redb into verifier on startup
- [x] Add `auth_tokens` redb table — user_id → encoded token
- [x] `AuthLayer` struct: verifier + revocation store + redb + owner key
- [x] `resolve_capabilities()` — lookup token → verify → return capabilities
- [x] Matrix: handle `!token <base64>` bot command — verify + store
- [x] Matrix: lookup sender's token on each message, verify, extract capabilities
- [x] Matrix: fall back to allowlist if no token (backwards compatible)
- [x] Iroh/QUIC: require auth token in handshake for remote connections
- [x] `capability.rs` in clankers-controller — `is_tool_allowed()`, `clamp_capabilities()`
- [x] `tool_blocked_event()` — return permission denied as DaemonEvent
- [x] Expired token: reply with error message

## Phase 4: Delegation via Matrix

- [x] `!delegate` bot command — create child token from sender's token
- [x] `!delegate --tools <pattern> --expire <duration>` — attenuated
- [x] Validate Delegate capability on sender's token
- [x] Validate containment (no escalation)
- [x] Reply with base64 child token

## Phase 5: Missing from original spec (from aspen-auth review)

- [ ] Port `Credential` type (token + proof chain bundle) from aspen-auth
- [ ] `Credential::from_root()`, `delegate()`, `verify()`, `encode()`/`decode()`, `to_base64()`/`from_base64()`
- [ ] Update `!token` to accept Credential (base64-encoded chain, not bare token)
- [ ] Update `!delegate` to return Credential (child token + parent chain)
- [ ] Update iroh QUIC handshake to send/verify Credential instead of bare token
- [ ] Update `AuthLayer` to store + verify Credentials
- [ ] Add `facts` field to token (metadata: model preferences, audit context)
- [ ] `TokenBuilder::with_fact()` / `with_facts()` methods
- [ ] Tests: Credential encode/decode roundtrip
- [ ] Tests: Credential delegation chain roundtrip (2-level, 3-level)
- [ ] Tests: broken chain rejected
- [ ] Tests: max delegation depth enforced through Credential
- [ ] Tests: capability escalation rejected through Credential
