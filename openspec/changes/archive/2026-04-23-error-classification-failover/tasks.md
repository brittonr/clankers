## 1. Error taxonomy types

- [x] 1.1 Create `crates/clankers-provider/src/error_classifier.rs` [covers=structured-error-taxonomy]
- [x] 1.2 Define `FailoverReason` enum: Auth, AuthPermanent, Billing, RateLimit, Overloaded, ServerError, Timeout, ContextOverflow, ModelNotFound, FormatError, Unknown [covers=structured-error-taxonomy]
- [x] 1.3 Define `ClassifiedError` struct with fields: reason, status_code, provider, message, retryable, should_compress, should_rotate_credential, should_fallback [covers=recovery-hints,classified-payload-propagation]
- [x] 1.4 Implement `recovery_hints(reason) -> (retryable, should_compress, should_rotate, should_fallback)` mapping [covers=recovery-hints]

## 2. Classification pipeline

- [x] 2.1 Define pattern sets: `BILLING_PATTERNS`, `RATE_LIMIT_PATTERNS`, `CONTEXT_OVERFLOW_PATTERNS`, `MODEL_NOT_FOUND_PATTERNS`, `AUTH_PATTERNS` [covers=provider-specific-pattern-matching,disambiguation-of-ambiguous-patterns]
- [x] 2.2 Implement `classify_api_error(status_code: Option<u16>, body: &str, provider: &str) -> ClassifiedError` [covers=structured-error-taxonomy,provider-specific-pattern-matching]
- [x] 2.3 Classification priority: status code first, then body pattern matching [covers=provider-specific-pattern-matching,disambiguation-of-ambiguous-patterns]
- [x] 2.4 Implement disambiguation for ambiguous patterns: check for transient signals ("try again", "resets at") to distinguish RateLimit from Billing [covers=disambiguation-of-ambiguous-patterns]
- [x] 2.5 Implement timeout-aware classification helper for transport/provider errors without an HTTP response [covers=timeout-classification]

## 3. Provider-specific patterns

- [x] 3.1 Add Anthropic-specific patterns (529 → Overloaded, "thinking block" → FormatError) [covers=provider-specific-pattern-matching]
- [x] 3.2 Add OpenAI-specific patterns (model_not_found, insufficient_quota) [covers=provider-specific-pattern-matching]
- [x] 3.3 Add OpenRouter-specific patterns (routing errors, upstream provider errors) [covers=provider-specific-pattern-matching]
- [x] 3.4 Add generic OpenAI-compatible patterns for local endpoints (vLLM, ollama, llama.cpp context errors) [covers=provider-specific-pattern-matching]

## 4. Integration with existing code

- [x] 4.1 Wire classifier into Anthropic credential pool: replace raw 429 checks with `should_rotate_credential` [covers=classified-payload-propagation]
- [x] 4.2 Store the full `ClassifiedError` (including original error text/message and hints) on `ProviderError`, then drive retryability/fallback decisions from that structured payload [covers=classified-payload-propagation]
- [x] 4.3 Wire classifier-derived retry/compress handling into `crates/clankers-agent/src/turn.rs` without regressing current retry behavior [covers=classified-payload-propagation]
- [x] 4.4 Export `ClassifiedError` from `clankers-provider` public API for router/local consumers inside this repo [covers=classified-payload-propagation]
- [x] 4.5 Add `classify_api_error` calls in `crates/clankers-provider/src/anthropic/api.rs` [covers=classified-payload-propagation]
- [x] 4.6 Add `classify_api_error` calls in `crates/clankers-provider/src/anthropic/mod.rs` [covers=classified-payload-propagation]
- [x] 4.7 Add `classify_api_error` calls in `crates/clankers-provider/src/router.rs` summary/error paths [covers=classified-payload-propagation]
- [x] 4.8 Add `classify_api_error` calls in `crates/clankers-provider/src/openai_codex.rs` error/probe handling [covers=classified-payload-propagation,provider-specific-pattern-matching]
- [x] 4.9 Distinguish `AuthPermanent` from `Auth` when refresh/fallback has already been attempted and auth still fails [covers=classified-payload-propagation,recovery-hints]
- [x] 4.10 Add `openspec/changes/error-classification-failover/router-follow-up.md` documenting the deferred external `clanker-router` coordination and why it is out of scope here [covers=classified-payload-propagation]

## 5. Tests

- [x] 5.1 Verification test: `Billing` recovery hints set `should_rotate_credential: true`, `retryable: false`, and `should_fallback: true` [covers=recovery-hints]
- [x] 5.2 Verification test: `ContextOverflow` recovery hints set `should_compress: true`, `retryable: true`, and `should_rotate_credential: false` [covers=recovery-hints]
- [x] 5.3 Verification test: `AuthPermanent` recovery hints set `retryable: false` and `should_fallback: true` [covers=recovery-hints]
- [x] 5.4 Verification test: status-code classification covers `429`, `402`, `401`, `403`, `404`, `413`, `500`, `502`, `503`, and `529` [covers=structured-error-taxonomy,provider-specific-pattern-matching]
- [x] 5.5 Verification test: OpenAI `model_not_found` body classification sets `ModelNotFound` with `should_fallback: true` [covers=provider-specific-pattern-matching]
- [x] 5.6 Verification test: body-based `Billing` classification maps `insufficient credits` to `Billing` [covers=structured-error-taxonomy]
- [x] 5.7 Verification test: body-based `ContextOverflow` classification maps `context length` or `too many tokens` to `ContextOverflow` [covers=structured-error-taxonomy]
- [x] 5.8 Verification test: quota disambiguation maps `quota exceeded` + transient signal to `RateLimit`, but `quota exceeded` alone to `Billing` [covers=disambiguation-of-ambiguous-patterns]
- [x] 5.9 Verification test: unknown errors default to retryable `Unknown` [covers=structured-error-taxonomy]
- [x] 5.10 Verification test: Anthropic `thinking block` body classification maps to `FormatError` [covers=provider-specific-pattern-matching]
- [x] 5.11 Verification test: OpenRouter upstream overload classifies to `Overloaded`, and missing routed endpoint/model paths classify to `ModelNotFound` [covers=provider-specific-pattern-matching]
- [x] 5.12 Verification test: generic OpenAI-compatible local endpoint patterns classify context/token overflow correctly [covers=provider-specific-pattern-matching]
- [x] 5.13 Verification test: Anthropic credential pool rotates only when classified hints set `should_rotate_credential` [covers=classified-payload-propagation]
- [x] 5.14 Verification test: `ProviderError` retains the full classified payload and exposes classifier-derived retry/compress/fallback hints, including `AuthPermanent` after refresh/fallback-style escalation conditions [covers=classified-payload-propagation]
- [x] 5.15 Verification test: `crates/clankers-provider/src/anthropic/api.rs` preserves classified reasons/hints on HTTP errors [covers=classified-payload-propagation]
- [x] 5.16 Verification test: `crates/clankers-provider/src/anthropic/mod.rs` pool/wrapper behavior respects classified hints [covers=classified-payload-propagation]
- [x] 5.17 Verification test: `crates/clankers-provider/src/router.rs` summary/error paths preserve classified reasons/hints [covers=classified-payload-propagation]
- [x] 5.18 Verification test: `crates/clankers-provider/src/openai_codex.rs` error/probe handling preserves classified reasons/hints using deterministic fixtures (for example generated invalid JWTs or fake providers, not opaque copied tokens) [covers=classified-payload-propagation,provider-specific-pattern-matching]
- [x] 5.19 Verification test: agent/runtime consumers use classifier-derived retryability and compressability without regressing existing retry behavior [covers=classified-payload-propagation]
- [x] 5.20 Verification test: timeout helper classifies transport/provider timeout failures as `Timeout` [covers=timeout-classification]
- [x] 5.21 Verification test: conflicting status/body evidence still classifies by status first [covers=disambiguation-of-ambiguous-patterns,provider-specific-pattern-matching]
- [x] 5.22 Verification test: `ClassifiedError` remains publicly re-exported from `clankers-provider` [covers=classified-payload-propagation]
- [x] 5.23 Verification test: `openspec/changes/error-classification-failover/router-follow-up.md` exists and documents the deferred external router work [covers=classified-payload-propagation]
