# clankers

A terminal coding agent in Rust. Inspired by [pi](https://pi.dev), built to be hacked on.

## Build

```
cargo build --release
```

## Auth

```
export ANTHROPIC_API_KEY=sk-...
```

## Use

```
clankers                        # interactive TUI
clankers -p "fix the tests"     # one-shot
```

## Plugins

Plugins are WebAssembly modules loaded via [Extism](https://extism.org). Drop a `plugin.json` + `.wasm` file into `plugins/` or install with `clankers plugin install <path>`.

A plugin declares tools the agent can call:

```json
{
  "name": "clankers-wordcount",
  "version": "0.1.0",
  "wasm": "clankers_wordcount.wasm",
  "kind": "extism",
  "tools": ["wordcount"],
  "tool_definitions": [
    {
      "name": "wordcount",
      "description": "Count words, lines, and characters in text",
      "handler": "handle_tool_call",
      "input_schema": {
        "type": "object",
        "properties": {
          "text": { "type": "string" }
        },
        "required": ["text"]
      }
    }
  ]
}
```

The Rust side is a single Extism guest function:

```rust
use extism_pdk::*;

#[plugin_fn]
pub fn handle_tool_call(input: String) -> FnResult<String> {
    let call: ToolCallInput = serde_json::from_str(&input)?;
    // do work, return JSON result
}
```

Build with `cargo build --target wasm32-unknown-unknown --release`. See `examples/plugins/` for a complete walkthrough and `plugins/` for shipped plugins (hash, text-stats, etc).

## Headless

No TUI required. Pipe prompts in, get results out.

```
clankers -p "explain this codebase"                     # stream text to stdout
clankers -p "list all TODOs" --mode json                # JSON lines event stream
clankers -p "refactor auth" --output result.md          # write to file
echo "what is this?" | clankers --stdin                 # pipe input
```

Headless mode works in CI, cron jobs, scripts — anywhere without a terminal.

## Router

`clankers-router` is a standalone daemon that sits between the agent and LLM providers. Run it separately or let clankers auto-start it.

- **Multi-provider** — Anthropic, OpenAI, Google, DeepSeek, Groq, Mistral, xAI, OpenRouter, Together, Fireworks, Perplexity, plus any local server (Ollama, LM Studio, vLLM)
- **Fallback chains** — automatic failover when a provider is rate-limited or down
- **Circuit breaker** — per-provider health tracking with exponential backoff
- **Response cache** — SHA-256 keyed with TTL and LRU eviction
- **OpenAI-compatible proxy** — exposes the router as an OpenAI API endpoint, so Cursor, aider, Continue, etc. can use your credentials and routing
- **P2P tunnel** — QUIC-based remote access via [iroh](https://iroh.computer), no port forwarding needed

## Skills

Skills are reusable prompt snippets that teach the agent domain-specific knowledge. Put them in `.clankers/skills/` or install with `clankers skill install`.

## License

[AGPL-3.0-or-later](LICENSE)
