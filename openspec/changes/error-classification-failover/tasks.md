## 1. Error taxonomy types

- [ ] 1.1 Create `crates/clankers-provider/src/error_classifier.rs`
- [ ] 1.2 Define `FailoverReason` enum: Auth, AuthPermanent, Billing, RateLimit, Overloaded, ServerError, Timeout, ContextOverflow, ModelNotFound, FormatError, Unknown
- [ ] 1.3 Define `ClassifiedError` struct with fields: reason, status_code, provider, message, retryable, should_compress, should_rotate_credential, should_fallback
- [ ] 1.4 Implement `recovery_hints(reason) -> (retryable, should_compress, should_rotate, should_fallback)` mapping

## 2. Classification pipeline

- [ ] 2.1 Define pattern sets: `BILLING_PATTERNS`, `RATE_LIMIT_PATTERNS`, `CONTEXT_OVERFLOW_PATTERNS`, `MODEL_NOT_FOUND_PATTERNS`, `AUTH_PATTERNS`
- [ ] 2.2 Implement `classify_api_error(status_code: Option<u16>, body: &str, provider: &str) -> ClassifiedError`
- [ ] 2.3 Classification priority: status code first, then body pattern matching
- [ ] 2.4 Implement disambiguation for ambiguous patterns: check for transient signals ("try again", "resets at") to distinguish RateLimit from Billing

## 3. Provider-specific patterns

- [ ] 3.1 Add Anthropic-specific patterns (529 → Overloaded, "thinking block" → ThinkingSignature)
- [ ] 3.2 Add OpenAI-specific patterns (model_not_found, insufficient_quota)
- [ ] 3.3 Add OpenRouter-specific patterns (routing errors, upstream provider errors)
- [ ] 3.4 Add generic OpenAI-compatible patterns for local endpoints (vLLM, ollama, llama.cpp context errors)

## 4. Integration with existing code

- [ ] 4.1 Wire classifier into Anthropic credential pool: replace raw 429 checks with `should_rotate_credential`
- [ ] 4.2 Wire classifier into the agent turn loop retry logic: check `retryable` and `should_compress` before deciding next action
- [ ] 4.3 Export `ClassifiedError` from `clankers-provider` public API for consumption by `clanker-router`
- [ ] 4.4 Add `classify_api_error` calls at each provider's HTTP error handling path

## 5. Tests

- [ ] 5.1 Unit test: each FailoverReason produces correct recovery hints
- [ ] 5.2 Unit test: status code classification (429, 402, 401, 404, 500, 502, 503, 529)
- [ ] 5.3 Unit test: body pattern matching for each pattern set
- [ ] 5.4 Unit test: disambiguation — "quota exceeded try again" → RateLimit, "quota exceeded" alone → Billing
- [ ] 5.5 Unit test: unknown errors default to retryable Unknown
