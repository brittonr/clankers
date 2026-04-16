## Context

Clankers has credential pool failover in `clankers-provider/src/anthropic/mod.rs` that rotates accounts on 429 responses. Beyond that, error handling is ad-hoc: raw `reqwest` errors bubble up, status codes are checked in scattered `match` arms, and error messages are string-matched locally at each call site. The `clanker-router` external crate handles some routing-level errors but doesn't classify them structurally.

Hermes has `error_classifier.py` with a `FailoverReason` enum (auth, billing, rate_limit, overloaded, timeout, context_overflow, model_not_found, format_error, etc.) and a `ClassifiedError` struct carrying recovery hints (`retryable`, `should_compress`, `should_rotate_credential`, `should_fallback`). The retry loop consults the classifier instead of checking raw errors.

## Goals / Non-Goals

**Goals:**
- `FailoverReason` enum covering all common API failure modes
- `ClassifiedError` type with the original error, classified reason, and recovery hints
- Provider-specific pattern matching for Anthropic, OpenAI, OpenRouter, and generic OpenAI-compatible endpoints
- Recovery hints that the retry loop and router can act on without re-classifying
- Wire into existing credential pool rotation and the agent turn loop

**Non-Goals:**
- Automatic provider failover across different providers (that's the router's job — this just classifies errors to help the router decide)
- Retry budget management (existing retry logic handles backoff timing)
- User-facing error reporting improvements (separate concern)

## Decisions

**Taxonomy:**
```rust
enum FailoverReason {
    Auth,              // 401/403 — refresh token or rotate credential
    AuthPermanent,     // auth failed after refresh — abort
    Billing,           // 402 or "insufficient credits" — rotate immediately
    RateLimit,         // 429 — backoff then rotate
    Overloaded,        // 503/529 — provider overloaded, backoff
    ServerError,       // 500/502 — internal, retry
    Timeout,           // connection/read timeout — retry
    ContextOverflow,   // "context length exceeded" — compress, don't failover
    ModelNotFound,     // 404 or "invalid model" — fallback to different model
    FormatError,       // 400 bad request — abort or strip problematic content
    Unknown,           // unclassifiable — retry with backoff
}
```

**Classification pipeline (priority-ordered):**
1. Check HTTP status code first (429, 402, 401, 403, 404, 413, 500, 502, 503, 529)
2. Then pattern-match error message body against provider-specific string sets
3. Disambiguate overlapping patterns (e.g., "quota" could be billing or rate limit — check for transient signals like "try again" or "resets at")

**Integration points:**
- `clankers-provider` exports `classify_api_error(status, body, provider) -> ClassifiedError`
- The Anthropic credential pool uses `should_rotate_credential` instead of raw 429 checks
- The agent turn loop uses `retryable` and `should_compress` to decide next action
- `clanker-router` can consume `ClassifiedError` from provider errors to make routing decisions

**Module location:** `crates/clankers-provider/src/error_classifier.rs` — co-located with provider code since classification patterns are provider-specific.

## Risks / Trade-offs

- **Pattern maintenance:** API error messages change without notice. Mitigate with broad patterns ("rate limit" matches multiple phrasings) and a catch-all `Unknown` that defaults to retry.
- **Over-classification:** Risk of misclassifying a billing error as a rate limit (or vice versa). Hermes handles this with disambiguation heuristics; we should adopt the same approach.
- **Cross-crate dependency:** `clanker-router` would benefit from seeing `ClassifiedError` but adding it to the router's error type requires coordinating the external crate. Start with classification in `clankers-provider` and expose it to the router via a trait or re-export.
