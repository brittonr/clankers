# clanker-router

Model router and auth gateway for LLM providers.

Routes completion requests across Anthropic, OpenAI, Google/Gemini, DeepSeek,
Groq, Mistral, OpenRouter, Together, Fireworks, Perplexity, xAI, HuggingFace,
and arbitrary OpenAI-compatible endpoints. Handles credential management,
retry with backoff, circuit breaking, response caching, and usage tracking.

## Features

- **Multi-provider routing** with configurable fallback chains
- **Circuit breaker** per provider/model (Closed → Open → HalfOpen)
- **Retry** with exponential backoff, full jitter, and Retry-After headers
- **Credential pooling** — multiple API keys per provider, round-robin or LRU
- **OAuth PKCE** with automatic token refresh and file-locking
- **Response caching** with SHA-256 keys, TTL, and LRU eviction
- **Usage tracking** per-model token counts and cost
- **OpenAI-compatible proxy** (`/v1/chat/completions`, `/v1/models`)
- **iroh QUIC tunnel** for remote proxy access without port forwarding
- **Standalone binary** with CLI and ratatui TUI

## Library usage

```rust
use clanker_router::{Router, Model};
use clanker_router::provider::CompletionRequest;

// Build router, register providers, send completions
let router = Router::new();
// ...
```

## Binary

```bash
clanker-router auth set-key openai sk-...
clanker-router models
clanker-router ask "hello world"
clanker-router proxy --port 8080
clanker-router daemon start
```

## Feature flags

- `proxy` — axum-based OpenAI-compatible proxy server
- `rpc` — iroh QUIC tunnel for remote proxy access
- `cli` — binary with clap CLI and ratatui TUI (enables `rpc` + `proxy`)

## License

MIT
