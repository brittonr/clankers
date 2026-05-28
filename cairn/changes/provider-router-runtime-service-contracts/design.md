# Design: Provider Router Runtime Service Contracts

## Summary

The runtime provider service should be an SDK boundary: it accepts neutral model requests and yields neutral stream/response events. Provider-native JSON/body shaping, OAuth, router daemon fallback, refresh, and cooldown policy remain owned by provider/router implementations and desktop adapters.

## Decisions

### Decision: neutral request mirrors engine needs, not provider internals

The runtime request should carry model label, session id, system prompt, neutral messages/content, tools, thinking/cache options, and safe extra metadata. It should not expose `CompletionRequest`, daemon RPC frames, OAuth stores, or provider-native body JSON.

### Decision: streaming is first-class

Provider services must return semantic stream events or a completed response that can feed engine-host accumulators. Returning only receipt stats is insufficient for runtime execution.

### Decision: auth and credential policy are host-owned services

Auth lookup, refresh persistence, pending login verifiers, account selection, and credential-pool strategy must be explicit service calls with safe receipts. Disabled/default embedded mode must fail closed.

## Verification Plan

- Pin literal neutral-provider fixtures and desktop adapter conversions instead of building expected JSON with implementation helpers.
- Add redaction tests for provider/auth receipts.
- Add parity tests proving existing provider discovery/fail-closed behavior remains through desktop adapters.
