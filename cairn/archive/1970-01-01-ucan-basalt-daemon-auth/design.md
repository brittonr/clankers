# Design: UCAN + Basalt Daemon Auth

## Context

Current audit findings are recorded in `evidence/current-auth-audit.md`.

The daemon auth path currently decodes `clankers_ucan::Credential`, which is an alias for `clanker_auth::Credential<Capability>`, then calls `TokenVerifier::verify_with_chain(...)` in `src/modes/daemon/session_store.rs`. QUIC control, QUIC attach, chat/RPC handlers, Matrix handlers, and `src/capability_gate.rs` all consume the resulting custom `Capability` values. Public `ucan` support exists in `crates/clankers-ucan/src/external_adapter.rs` and `external_caveats.rs`, but it is feature-gated and not the default daemon verifier. Basalt is already a workspace dependency and is used by Steel orchestration, but not by daemon credential admission.

## Decisions

### Decision: public UCAN is the canonical remote credential

**Choice:** `ControlCommand::CreateSession.token`, QUIC attach handshakes, chat/RPC auth frames, Matrix stored credentials, and token CLI output will carry a versioned Clankers public-UCAN credential envelope. The envelope wraps an OnixResearch `ucan::CompactToken`, proof compact tokens, audience/root metadata, and replay/revocation identifiers needed to build a deterministic `VerificationContext`.

**Rationale:** The protocol can keep its existing optional token string field while making the payload explicit and forward-compatible. The daemon must reject unknown envelope versions and legacy `clanker-auth` base64 credentials by default so remote access cannot silently fall back to the old verifier.

### Decision: Basalt is mandatory for remote admission

**Choice:** Remote admissions run through a shared `BasaltUcanAuthority` that performs public UCAN verification and Basalt policy enforcement for the same `(resource, ability)` request. The authority returns a redacted receipt used by daemon logs, session events, and capability-gate denials.

**Rationale:** UCAN proves delegation and capability possession; Basalt constrains which Clankers contracts/resources/abilities the daemon recognizes. Both are needed: a valid token outside policy must deny, and an allowed policy without a matching verified UCAN grant must deny.

### Decision: Clankers uses a stable resource/ability vocabulary

**Choice:** `crates/clankers-ucan` owns normalized resource and ability constructors such as:

- `clankers:daemon/<peer>/session` + `session/create`
- `clankers:session/<session-id>` + `session/attach`, `session/prompt`, `session/manage`
- `clankers:tool/<tool-name>` + `tool/use`
- `clankers:file://<normalized-path>` + `file/read` or `file/write`
- `clankers:shell://<working-dir>` + `shell/execute`
- `clankers:process/<backend>` + `process/observe`, `process/start`, `process/mutate`, `process/stdin`, `process/logs`
- `clankers:model/<provider-or-model>` + `model/use`

Invocation requests must be concrete. Wildcards and prefixes are grant/caveat material, not invocation material.

**Rationale:** A single vocabulary lets the daemon, Matrix/chat entrypoints, runtime admission helpers, and tool gate ask the same UCAN/Basalt question. Keeping wildcard semantics out of invocation construction matches the existing public UCAN adapter guardrails.

### Decision: daemon storage is public-UCAN aware

**Choice:** `AuthLayer` becomes a public-UCAN/Basalt authority store. Redb tables store credential envelopes by peer/user id, revoked proof references, replay admissions/nonces, policy metadata, and migration records. The daemon owner iroh key is adapted to a public UCAN signer/DID for root issuance; trusted roots are configured explicitly from owner identity and operator config.

**Rationale:** The current redb storage can remain the persistence boundary, but stored bytes must become versioned public-UCAN records so stale custom credentials are detected and removed rather than decoded as authority.

### Decision: per-tool gates ask the same authority at call time

**Choice:** `UcanCapabilityGate` is replaced or wrapped by a gate that maps each tool call into one or more concrete public UCAN invocations and Basalt requests at execution time. File, shell, process, model, session, and generic tool checks must all be covered. Human confirmation remains an additional shell requirement for operations that already need it; it cannot turn a UCAN/Basalt deny into allow.

**Rationale:** Static custom `Capability` snapshots cannot express caveats, replay, or policy receipts. Runtime admission must evaluate the exact path/command/tool/model being requested.

### Decision: migration is explicit and fail-closed

**Choice:** Legacy `clanker-auth` credentials are not trusted by default. If compatibility is needed, add an explicit operator command or feature flag that imports a legacy credential, verifies it with the old verifier, issues an equivalent public UCAN credential, records a migration receipt, and then stores only the public UCAN envelope.

**Rationale:** A silent dual-verifier default weakens the switch. Explicit migration gives operators a path forward without making old tokens ambient authority.

### Decision: receipts are safe by construction

**Choice:** Admission receipts include schema version, policy hash/id, resource, ability, token proof reference, audience/root ids, replay/revocation status, decision, and deny reason. Receipts must not include raw compact tokens, proof token bodies, signing keys, prompts, provider payloads, full command strings, raw file contents, or unredacted tool JSON.

**Rationale:** UCAN/Basalt decisions need to be debuggable and auditable, but remote auth receipts can easily become credential leaks if they carry raw token material.

## Risks / Trade-offs

- Existing custom Clankers tokens will stop working unless explicitly migrated.
- Basalt is AGPL-licensed and already in the workspace; any direct daemon dependency should keep that license boundary visible to operators.
- The OnixResearch `ucan` source should be remote-pinned consistently with Basalt rather than requiring a sibling `../../../ucan` checkout.
- Per-tool public UCAN invocation mapping is more precise than the current custom enum, but it requires deterministic fixtures for every high-risk operation class.
- Fail-closed behavior may initially reject remote flows that were previously admitted by allowlists; diagnostics and migration receipts must make that rejection actionable without leaking secrets.

## Verification Plan

- Fixture-issued public UCAN root, child, and grandchild tokens covering proof chains, expiry, not-before, audience, replay, revocation, caveats, and deny paths.
- Basalt policy fixtures that allow recognized Clankers contracts and deny unknown contracts/resources/abilities even when UCAN is otherwise valid.
- Daemon seam tests for QUIC create, QUIC attach, chat/RPC auth frames, and Matrix keyed session recovery using valid, missing, malformed, legacy, expired, revoked, wrong-audience, and policy-denied credentials.
- Tool-gate tests for read/write/edit/bash/process/model/session operations that assert both UCAN invocation construction and Basalt receipts.
- Redaction tests over every receipt/event/log surface that could carry token, proof, prompt, command, or provider data.
- Dependency/source tests proving daemon auth no longer uses `clanker-auth::TokenVerifier` by default and `clankers-ucan` no longer requires a sibling `../../../ucan` path.
