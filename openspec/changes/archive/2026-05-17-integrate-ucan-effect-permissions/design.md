## Context

Clankers has Unison-inspired specs for immutable content-addressed agent artifacts, typed effect ability classes, host-owned handlers, remote hash dependency preflight, and typed durable session facts. UCAN provides a complementary authorization model: signed, attenuable grants with proof chains, caveats, invocation checks, replay admission hooks, and revocation hooks. The sibling `../ucan/` repo's public surface is explicitly designed for downstream consumers.

## Goals / Non-Goals

**Goals:**
- Map Clankers effect classes to UCAN ability strings and resource URIs.
- Verify UCAN proof chains and caveats before side-effect handlers execute.
- Support attenuation for subagents, remote daemons, scheduled jobs, and replay handlers.
- Record safe authorization receipts in content-addressed artifacts and typed ledger facts.
- Depend on `../ucan/` public APIs through a thin adapter seam.

**Non-Goals:**
- No Clankers-local reimplementation of token parsing, proof traversal, caveat evaluation, replay admission, or revocation logic.
- No ambient global authority loaded from process state without explicit session/host configuration.
- No raw compact tokens, secrets, headers, or provider payloads in receipts or ledger rows.
- No claim that UCAN replaces human confirmation for destructive actions.

## Decisions

### 1. UCAN is the authorization layer over effect abilities

**Choice:** Treat each protected `EffectRequest` as a concrete UCAN invocation: `(invoker, audience, resource, ability, caveat context, replay context)`.

**Rationale:** This preserves the Unison-style split: typed abilities describe effects; handlers interpret them; UCAN decides whether this caller has an attenuated grant for this resource and operation.

**Alternative:** Keep tool-enable booleans and ad hoc permission checks. Rejected because they do not compose across subagents, remote peers, proof chains, or replay.

### 2. Clankers maps effects into a narrow capability vocabulary

**Choice:** Define stable ability identifiers such as `clankers/file.read`, `clankers/file.write`, `clankers/shell.exec`, `clankers/network.fetch`, `clankers/secret.read`, `clankers/browser.act`, `clankers/scheduler.enqueue`, `clankers/remote.exec`, `clankers/provider.request`, `clankers/delivery.send`, `clankers/artifact.read`, `clankers/artifact.write`, `clankers/plugin.invoke`, and `clankers/mcp.invoke`.

**Rationale:** Stable names are required for grants, attenuation, fixtures, and audit. They also avoid exposing internal Rust type names as authorization contracts.

**Implementation:** The adapter translates internal effect classes to UCAN ability strings and resource URIs. Unknown effect classes fail closed. The first handler migration may route one low-risk effect through the adapter, but tests and docs must avoid describing other legacy paths as UCAN-protected until they are wired.

### 3. Caveats carry Clankers-specific policy facts

**Choice:** Use UCAN caveat payloads for bounded permissions: path prefixes, command allowlists, network host allowlists, timeout ceilings, artifact hashes, redaction classes, replay nonce/freshness, model/provider scope, delivery targets, and max bytes.

**Rationale:** The `../ucan/` library intentionally leaves application caveat semantics to callers while preserving fail-closed policy hooks. Clankers owns the meaning of its resources.

**Implementation:** Clankers supplies deterministic caveat policy hooks to `ucan` invocation verification. Unknown caveats deny authorization.

### 4. Delegation is attenuation, not cloning authority

**Choice:** Session-to-subagent, session-to-scheduled-job, local-to-remote-daemon, and replay-handler grants are delegated UCANs that must be no broader than their parents.

**Rationale:** UCAN's proof-chain attenuation matches the desired Clankers delegation model and prevents accidental permission expansion.

**Implementation:** Delegation helpers mint or select narrowed grants for each child execution context. The child context receives only safe grant/proof references and never ambient parent secrets.

### 5. Receipts store proof references and decisions, not raw tokens

**Choice:** Effect receipts and ledger facts record authorization status, effect/resource, caveat IDs/classes, proof-chain hash/reference, token issuer/audience DIDs, denial class, and replay/revocation status where safe. Raw compact tokens and secrets are excluded or redacted.

**Rationale:** Receipts must be useful for replay/review while staying safe to persist and sync.

**Implementation:** Content-addressed artifact envelopes may store redacted UCAN grant metadata by hash. Secret-bearing token material remains in the host authority store and is resolved by proof reference when policy allows.

### 6. Use `../ucan/` as a public-library dependency behind an adapter

**Choice:** Add a Clankers `ucan_authorization` adapter that depends only on public `ucan` / `ucan-core` APIs from the sibling checkout during development.

**Rationale:** Keeps Clankers from depending on UCAN internals and makes later source pinning or vendoring mechanical.

**Implementation:** The first implementation slice should add a small adapter crate/module with fake/in-memory fixtures. Packaging work must decide whether release builds use a git-pinned UCAN revision, workspace overlay, or vendored source; until then release docs must not claim the integration is distributable without the sibling checkout.

## Risks / Trade-offs

**Dependency source drift** → Mitigate with a pinned revision/source-plan task and CI checks that fail when `../ucan/` APIs drift.

**Receipt leaks** → Mitigate with negative tests for raw compact-token, header, env, and secret substrings in receipt/ledger output.

**Overbroad grants** → Mitigate with attenuation tests proving child grants cannot add abilities, widen resources, relax caveats, or extend expiry beyond parent authority.

**Confirmation bypass** → Mitigate by ordering gates explicitly: UCAN admission is necessary but not sufficient where human confirmation policy applies. A UCAN allow result must flow into the existing confirmation/admission chain rather than calling handlers directly.

## Validation Summary

- Vocabulary fixtures: every protected class maps to a stable `clankers/...` ability and normalized resource URI, while unknown classes deny.
- Caveat fixtures: path/command, network/provider, artifact/redaction, freshness/replay, malformed, and unknown caveats are deterministic and fail closed.
- Admission fixtures: the migrated low-risk handler allows with matching UCAN, denies before handler contact without authority, and preserves existing human confirmation ordering.
- Delegation fixtures: child grants cannot add abilities, widen resources, relax caveats, extend expiry, or mutate parent authority.
- Receipt/ledger fixtures: allowed/denied receipts and typed facts include safe proof metadata and omit raw tokens/secrets.
- Dependency rail: Clankers targeted check plus `../ucan` workspace tests or a pinned compatibility fixture.
