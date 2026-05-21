# Proposal

## Why

Provider selection compatibility has to be explicit. Omitted-provider Anthropic defaults must remain intact, missing or malformed claim handling for chatgpt_account_id must be defined, and provider-scoped status behavior for `status --provider openai-codex` must be reviewable.

## Verification

The scenario-complete verification plan covers proactive refresh, 401 retry, 429 retry, provider-scoped status, and discovery hiding.
