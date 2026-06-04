## Why

The Unison-inspired Clankers specs already define content-addressed artifacts, typed effect abilities, handler-mediated execution, and a typed durable session ledger. Those specs still need a concrete authorization contract. UCAN is the right fit: effect abilities describe what execution wants to do, while UCAN grants prove who may do that effect on which resource under which caveats.

The sibling `../ucan/` crate now exposes issuance, verification, proof chains, caveats, invocation authorization, replay admission, revocation hooks, and verified-logic-backed finite admission bridges. Clankers should integrate that public library instead of inventing a parallel permission model.

## What Changes

- **UCAN-backed effect admission**: every protected effect request can be checked against a verified UCAN invocation before the handler performs side effects.
- **Delegated subagent/session permissions**: parent sessions attenuate UCAN grants for subagents, remote peers, scheduled jobs, and replay handlers.
- **Caveat vocabulary for Clankers resources**: resource/effect caveats cover path prefixes, command allowlists, network hosts, provider/model scope, time/expiry, replay nonce, artifact hashes, max-bytes, and redaction class.
- **Content-addressed proof receipts**: model/tool/session receipts record safe UCAN proof identifiers and authorization decisions alongside artifact hashes.
- **Sibling UCAN integration**: Clankers consumes `../ucan/` public APIs at an adapter seam and does not reimplement token/proof/caveat/attenuation logic.

## Capabilities

### New Capabilities
- `ucan-effect-permissions`: UCAN verification and invocation admission for Clankers effect requests.

### Modified Capabilities
- `effect-ability-runtime`: effect handlers gain UCAN admission before side effects.
- `content-addressed-agent-artifacts`: receipts include proof-chain and grant artifact identities.
- `typed-durable-session-ledger`: typed facts include safe authorization decision metadata.

## Impact

- **Files**: future work will touch Clankers effect/tool dispatch, session persistence, remote/subagent delegation, dependency metadata, docs, and Cargo/Nix wiring for `../ucan/`.
- **APIs**: add an internal UCAN authorization adapter that maps `EffectRequest` into `ucan` invocation facts and maps authorization results into Clankers denials/receipts.
- **Dependencies**: introduce a controlled dependency on sibling `../ucan/` public crates for local development, with a pinned/reproducible source plan before release packaging.
- **Testing**: fixture-backed allow/deny tests for each protected effect class named by the vocabulary, delegated subagent attenuation, caveat failures, replay admission failures, missing proof/revocation failures, confirmation ordering, safe remote proof-reference sync, and redacted receipt/ledger output.

## Non-Goals

- Do not replace existing user confirmations with silent UCAN approval; confirmations remain separate policy gates where required.
- Do not transmit secrets or raw tokens in artifact sync, replay receipts, logs, or typed ledger facts.
- Do not implement a new UCAN library inside Clankers.
- Do not claim unmigrated legacy operations are UCAN-protected; the first handler slice may migrate one low-risk effect while the vocabulary, receipts, and tests define the broader protected set.
