# Design

The design keeps omitted-provider Anthropic defaults, rejects malformed chatgpt_account_id claim material before use, and defines provider-scoped status for explicit provider selection. Retry policy uses 3 retries, 1s/2s/4s backoff, exactly one 401 refresh, and one refresh cycle per request. The verification plan covers proactive refresh, 401 retry, 429 retry, provider-scoped status, and discovery hiding.
