Evidence-ID: current-auth-audit
Artifact-Type: code-audit
Task-ID: R1
Covers: r[ucan-basalt-daemon-auth.public-ucan], r[ucan-basalt-daemon-auth.basalt-policy], r[ucan-basalt-daemon-auth.daemon-seams], r[ucan-basalt-daemon-auth.tool-gate]
Created: 2026-05-29
Status: complete

# Current Auth Audit

## Scope

Audited the current code paths relevant to switching daemon/remote auth to OnixResearch `ucan` and Basalt:

- `crates/clanker-auth/`
- `crates/clankers-ucan/`
- `src/modes/daemon/session_store.rs`
- `src/modes/daemon/quic_bridge.rs`
- `src/modes/daemon/handlers.rs`
- `src/modes/daemon/agent_process.rs`
- `src/capability_gate.rs`
- root `Cargo.toml` / `Cargo.lock`
- `crates/clankers-runtime/src/steel_orchestration.rs`
- sibling `../basalt/README.md` and public API summary

## Findings

### Default daemon verifier is custom `clanker-auth`

`src/modes/daemon/session_store.rs::AuthLayer` stores a `clankers_ucan::TokenVerifier` and calls:

```text
self.verifier.verify_with_chain(&cred.token, &cred.proofs, None)
```

`crates/clankers-ucan/src/lib.rs` defines `TokenVerifier`, `TokenBuilder`, `CapabilityToken`, and `Credential` as aliases over `clanker_auth::*` with the Clankers `Capability` enum. That makes the default daemon credential format the workspace-local `clanker-auth` token stack.

### QUIC/chat/RPC entrypoints decode custom credentials

`src/modes/daemon/quic_bridge.rs` and `src/modes/daemon/handlers.rs` parse remote auth material with:

```text
clankers_ucan::Credential::from_base64(token_b64)
```

The resulting custom `Capability` values are passed into session creation and tool authorization.

### Tool gate consumes custom `Capability` snapshots

`src/capability_gate.rs::UcanCapabilityGate` receives `Vec<clankers_ucan::Capability>` and checks operations against the custom enum. It does not currently call the public `ucan` verifier or Basalt at tool-call time.

### Public `ucan` support is optional and sibling-path based

`crates/clankers-ucan/Cargo.toml` currently declares:

```text
ucan = { path = "../../../ucan", optional = true }
external-ucan = ["dep:ucan"]
runtime-admission = ["dep:clankers-runtime", "external-ucan"]
```

`external_adapter.rs` re-exports public `ucan` types and wraps `CompactToken`, `VerificationContext`, proof collections, replay, revocation, and invocation verification. This is the closest existing seam for public UCAN, but it is not default daemon auth.

### Basalt is present but not daemon auth

Root `Cargo.toml` has `basalt = { path = "../basalt", default-features = false }`, and `Cargo.lock` shows Basalt depending on `ucan` from:

```text
git+ssh://git@github.com/OnixResearch/ucan.git?rev=ad61b53e89fa45f9bf7d313ce14c45de645bf53d
```

`crates/clankers-runtime/src/steel_orchestration.rs` already uses Basalt contract envelopes, `CapabilityGrant`, `basalt_enforce`, and Steel receipts for turn-planning authority. That does not cover daemon session creation, attach, chat/RPC, Matrix, or generic tool execution.

### Basalt public API can support policy receipts

The sibling Basalt README documents `CapabilityGrant`, `EnforcementRequest`, `default_policy`, and `enforce`. Basalt checks a selected contract's resource/ability policy and UCAN-like capability grants, returning an `EnforcementReceipt` with allow/deny reason. For daemon auth, Clankers still needs public UCAN token/proof verification plus Basalt policy enforcement for the same normalized request.

## Conclusion

The switch should promote the existing `clankers-ucan::external_adapter` public UCAN seam to the default daemon credential path, align its `ucan` dependency with the remote-pinned OnixResearch source that Basalt uses, and add a shared Basalt-backed authority for remote entrypoints and per-tool gates. The existing `clanker-auth` stack should become migration-only or be removed from default daemon trust.
