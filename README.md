# clankers

A terminal coding agent in Rust. Inspired by [pi](https://pi.dev), built to be hacked on.

## Build

```
cargo build --release
cargo nextest run              # run tests
cargo clippy -- -D warnings    # lint
```

## Auth

Set an API key directly:

```
export ANTHROPIC_API_KEY=sk-...
```

Or use OAuth:

```
clankers auth login              # interactive provider selection
clankers auth login openai       # specific provider
clankers auth status             # check credentials
```

Supports multiple accounts (`--account work`, `clankers auth switch personal`).

## Use

```
clankers                        # interactive TUI
clankers -p "fix the tests"     # one-shot prompt
```

### Daemon Mode

Run agent sessions as background processes and attach from any terminal:

```
clankers daemon start -d        # start background daemon
clankers attach                 # attach to a session (interactive picker)
clankers attach --new           # create and attach to a new session
clankers attach --auto-daemon   # auto-start daemon if needed
clankers ps                     # list active sessions
clankers daemon kill <id>       # kill a session
clankers daemon stop            # stop the daemon
```

### Headless

No TUI required. Pipe prompts in, get results out.

```
clankers -p "explain this codebase"                     # stream text to stdout
clankers -p "list all TODOs" --mode json                # JSON lines event stream
clankers -p "refactor auth" -o result.md                # write to file
echo "what is this?" | clankers --stdin                 # pipe input
```

Works in CI, cron jobs, and scripts.

## Providers

`clankers-router` talks to Anthropic through its native API, and to OpenAI, Google, DeepSeek, Groq, Mistral, xAI, OpenRouter, Together, Fireworks, Perplexity, and HuggingFace through an OpenAI-compatible backend. Ollama is auto-detected on localhost. Any OpenAI-compatible local server (LM Studio, vLLM, etc.) works via `--api-base`.

## Router

`clankers-router` is a standalone daemon that sits between the agent and LLM providers. Run it separately or let clankers auto-start it.

It routes across all configured providers with automatic failover when one is rate-limited or down. Per-provider/model health state with exponential backoff keeps requests away from unhealthy endpoints. Responses are cached by SHA-256 request hash with configurable TTL.

The router exposes an OpenAI-compatible HTTP proxy, so Cursor, aider, Continue, etc. can use your credentials and routing. An iroh QUIC tunnel makes the same API reachable by node ID from anywhere, no port forwarding needed.

## Multi-Model Routing

Routes tasks to models by complexity. Simple tasks go to fast, cheap models; complex reasoning goes to powerful ones. The agent can switch models mid-conversation and tracks per-model costs with budget enforcement.

```
clankers --max-cost 10.0            # hard budget limit ($10)
clankers --enable-routing           # enable complexity-based routing
```

See [`docs/multi-model.md`](docs/multi-model.md) for configuration and cost tracking details.

## Sessions

Conversations persist as JSONL. Pick up where you left off.

```
clankers --continue                 # resume last session
clankers --resume <id>              # resume a specific session
clankers session list               # list recent sessions
clankers session show <id>          # inspect a session
clankers session export <id>        # export to file
```

## Branching

Fork conversations to explore alternatives without losing your work. Use `/fork` to try different approaches, `/switch` to navigate between branches, `/branches` to list them, and `/merge` to combine the best parts. See [`docs/tutorials/branching.md`](docs/tutorials/branching.md) for a walkthrough.

## Subagents

Delegate work to sub-instances. `subagent` spawns ephemeral one-shot workers for quick tasks (search, review, analysis) with parallel and chained execution. `delegate_task` spawns persistent named workers for long-running tasks that maintain state across interactions. Both get their own context and tool access.

## Worktree Isolation

Each session can run in its own git worktree, so parallel agents can't step on each other. Includes LLM-powered merge conflict resolution when merging back. Disable with `--no-worktree`.

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

Build with `cargo build --target wasm32-unknown-unknown --release`. See `examples/plugins/` for a walkthrough.

Shipped plugins: calendar, email, github, hash, self-validate, text-stats.

## P2P

### RPC

Peer-to-peer agent communication via [iroh](https://iroh.computer) QUIC:

```
clankers rpc id                         # show your node ID
clankers rpc start                      # start RPC server
clankers rpc ping <node-id>             # ping a remote instance
clankers rpc prompt <node-id> "..."     # send a prompt to a remote agent
clankers rpc send-file <node-id> <path> # send a file
clankers rpc peers list                 # list known peers
clankers rpc discover --mdns            # find peers on the LAN
```

### Remote Daemon Access

Attach to a daemon running on another machine:

```
clankers attach --remote <node-id>      # attach to remote daemon via iroh QUIC
```

### Session Sharing

Share a live Zellij terminal session over the network:

```
clankers share                          # get a node ID + key
clankers join <node-id> <key>           # join from another machine
```

## Matrix Bridge

Connect clankers instances over Matrix rooms for multi-agent coordination. Instances exchange structured messages (`m.clankers.*` types) over encrypted Matrix channels. Enable with `clankers daemon start --matrix`.

## Skills

Skills are reusable prompt snippets that teach the agent domain-specific knowledge. Place them in `~/.clankers/agent/skills/<name>/SKILL.md` (global) or `.clankers/skills/<name>/SKILL.md` (project).

## Agent Definitions

Named agent configurations with custom model, system prompt, and tool access. Place them in `~/.clankers/agent/agents/` or `.clankers/agents/`.

```
clankers --agent reviewer               # use a named agent definition
clankers --agent researcher --agent-scope project
```

## Capability Tokens

UCAN-based authorization tokens for scoping access to daemon sessions:

```
clankers token create --read-only       # read-only token
clankers token create --tools "read,grep,bash" --expire 24h
clankers token create --root            # full access
clankers token list                     # list issued tokens
clankers token revoke <hash>            # revoke a token
```

## Built-in Tools

Core: `read`, `write`, `edit`, `bash`, `grep`, `find`, `ls`, `ask`, `commit`, `web`, `nix`

Orchestration: `subagent`, `delegate_task`, `switch_model`, `loop`, `signal_loop_success`

Specialty: `review`, `todo`, `cost`, `schedule`, `image_gen`, `procmon`, `validate_tui`

Matrix: `matrix_send`, `matrix_read`, `matrix_rooms`, `matrix_peers`, `matrix_join`, `matrix_rpc`

Plugins add additional tools at runtime.

## Architecture

~30 workspace crates under `crates/`:

| Crate | Purpose |
|---|---|
| `clankers-agent` | Agent loop, system prompt, tool dispatch |
| `clankers-actor` | Erlang-style actor system (ProcessRegistry, signals, links) |
| `clankers-agent-defs` | Agent definition discovery and loading |
| `clankers-auth` | OAuth and credential management |
| `clankers-config` | Settings, paths, keybindings |
| `clankers-controller` | SessionController (transport-agnostic agent driver) |
| `clankers-db` | Embedded database (redb) |
| `clankers-hooks` | Event hooks (pre-commit, session start, etc.) |
| `clankers-loop` | Loop/retry engine |
| `clankers-matrix` | Matrix protocol bridge |
| `clankers-merge` | LLM-powered merge conflict resolution |
| `clankers-message` | Message types and serialization |
| `clankers-model-selection` | Complexity routing and cost tracking |
| `clankers-plugin` | WASM plugin host (Extism) |
| `clankers-plugin-sdk` | Plugin development SDK |
| `clankers-procmon` | Process monitor |
| `clankers-prompts` | Prompt template system |
| `clankers-protocol` | Daemon-client wire protocol (frames, events, commands) |
| `clankers-provider` | LLM provider abstraction |
| `clankers-router` | Multi-provider routing, fallback, caching, proxy |
| `clankers-scheduler` | Task scheduling |
| `clankers-session` | JSONL session persistence |
| `clankers-skills` | Skill discovery and loading |
| `clankers-specs` | OpenSpec integration |
| `clankers-tui` | Terminal UI (ratatui) |
| `clankers-tui-types` | Shared TUI type definitions |
| `clankers-util` | Shared utilities (logging, direnv, etc.) |
| `clankers-zellij` | Zellij session sharing |

## License

[AGPL-3.0-or-later](LICENSE)
