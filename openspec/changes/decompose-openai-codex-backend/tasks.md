## Phase 1: Baseline and Extraction

- [ ] [serial] Inventory openai_codex.rs responsibilities and capture current auth/entitlement/streaming tests as baseline.
- [ ] [depends:baseline] Extract entitlement cache/probing and auth/account helpers with focused negative tests.
- [ ] [depends:baseline] Extract Responses API request construction, retry/error classification, and streaming normalization modules.
- [ ] [depends:baseline] Update router/provider imports and run OpenAI Codex/auth/router targeted tests.
- [ ] [serial] Run cargo fmt, clanker-router nextest filters for openai_codex/openai_compat/auth, cargo check --tests for router/provider, openspec validate, and git diff --check.
