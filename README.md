# clankers

**This project is in heavy development and experimentation. Expect lots of breakages, outdated, generated docs, etc.**

A terminal coding agent in Rust. Inspired by [pi](https://pi.dev), built to be hacked on.

## Build and test

```
cargo build --release
./scripts/test-harness.sh quick        # check + workspace tests
./scripts/test-harness.sh live         # optional live/local-model tests such as aspen2 Qwen
./scripts/test-harness.sh full         # fmt, tests, clippy, repo rails, tigerstyle
./scripts/test-harness.sh vm           # all Linux Nix VM checks
./scripts/test-harness.sh ci           # exact nix flake check gate
```

The harness writes machine-readable results to `target/test-harness/results.json`, a Markdown summary to `target/test-harness/summary.md`, JUnit XML for CI collectors to `target/test-harness/junit.xml`, and per-step logs under `target/test-harness/logs/`. Use `CLANKERS_TEST_DRY_RUN=1` to inspect a tier without running it, or `CLANKERS_TEST_RESULT_DIR=path/to/results` to redirect all harness reports. `nix flake check` includes the credential-free `e2e-fake` check, which runs the deterministic fake-provider e2e tier through the packaged `clankers` binary. The optional live aspen2 check is not part of normal pure evaluation; from the repo root run `CLANKERS_ENABLE_LIVE_CHECKS=1 nix build --impure --no-link .#checks.$(nix eval --raw --impure --expr builtins.currentSystem).live-aspen2-qwen36 --option sandbox false -L`. It expects the sibling rats checkout at `../rats` by default; set `CLANKERS_LIVE_RATS_DIR=/path/to/rats` to override. The test self-skips when the aspen2 endpoint/model is unavailable.

Useful focused tiers:

```
./scripts/test-harness.sh package clankers-provider discovery
./scripts/test-harness.sh e2e                  # default fake-provider deterministic e2e
./scripts/test-harness.sh e2e api              # fake-provider prompt/tool/json checks
./scripts/test-harness.sh e2e fast             # local CLI/config/auth checks
./scripts/test-harness.sh live local-model     # optional live local-model checks, self-skips when unavailable
./scripts/test-harness.sh live aspen2-qwen36   # aspen2 Qwen 3.6 OpenAI-compatible streaming check
./scripts/test-harness.sh vm smoke             # vm-smoke only
./scripts/test-harness.sh vm core              # smoke, remote daemon, session recovery
./scripts/test-harness.sh vm module            # daemon/router/integration NixOS module VMs
./scripts/test-harness.sh vm vm-remote-daemon  # one explicit VM check
```

## Auth

Set an API key directly:

```
export ANTHROPIC_API_KEY=sk-...
```

Or use OAuth:

```
clankers auth login                               # Anthropic default OAuth login
clankers auth login --provider openai-codex       # ChatGPT Plus/Pro Codex subscription login
clankers auth login --provider openai-codex --account work
clankers auth status --all                        # grouped provider status
```

`openai-codex` stays separate from API-key `openai`. Use provider-qualified model IDs such as
`openai-codex/gpt-5.3-codex` for ChatGPT subscription Codex and `openai/gpt-4o` for API-key OpenAI.
Unsupported `openai-codex` plans stay authenticated but unavailable for Codex use, and explicit Codex
requests fail closed instead of falling back to API-key `openai`.

Supports multiple accounts and same-provider credential pools (`--account work`, `clankers auth add anthropic --api-key ... --account backup`, `clankers auth add openrouter --api-key ... --account backup`, `clankers auth switch --provider openai-codex work`). A single 429 is retried before rotation, repeated 429s rotate for 1 hour, and 402 quota errors rotate for 24 hours.

When using Anthropic OAuth, clankers now prepends a Claude Code
billing-header system block and rewrites clankers-specific markers in outbound
request text so Claude subscription billing still works. Disable that
compatibility layer with `CLANKERS_DISABLE_CLAUDE_SUBSCRIPTION_COMPAT=1`. If
Anthropic changes the expected block contents, override it with
`CLANKERS_ANTHROPIC_BILLING_HEADER`.

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

### ACP IDE Integration

Expose a clankers session to ACP-compatible editors over stdio:

```
clankers acp serve                         # serve a new foreground ACP session
clankers acp serve --session <id>          # resume a known clankers session id
clankers acp serve --new --model <model>   # force a new session and optional model
```

The first pass supports initialization and session prompt/update messages over a foreground stdio adapter. Terminal creation, remote workspaces, arbitrary tool listing/calls, and editor push notifications are intentionally returned as unsupported until dedicated follow-up work lands.

### Headless

No TUI required. Pipe prompts in, get results out.

```
clankers -p "explain this codebase"                     # stream text to stdout
clankers -p "explain this codebase" --inline            # styled markdown in scrollback
clankers -p "list all TODOs" --mode json                # JSON lines event stream
clankers -p "refactor auth" -o result.md                # write to file
echo "what is this?" | clankers --stdin                 # pipe input
```

Works in CI, cron jobs, and scripts.

## Providers

`clanker-router` talks to Anthropic through its native API, and to OpenAI, Google, DeepSeek, Groq, Mistral, xAI, OpenRouter, Together, Fireworks, Perplexity, and HuggingFace through an OpenAI-compatible backend. Ollama is auto-detected on localhost. Any OpenAI-compatible local server (LM Studio, vLLM, etc.) works via `--api-base`.

## Router

`clanker-router` is a standalone daemon that sits between the agent and LLM providers. Run it separately or let clankers auto-start it.

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

## Context References

Use `@` references in prompts to inline local context before a turn runs:

- `@path/to/file` includes a text file.
- `@path/to/file:10-20` includes a line range.
- `@path/to/dir/` includes a sorted directory listing.
- `@path/to/image.png` attaches an image when the prompt path supports image blocks.

Local context references work in the TUI, attach, and one-shot prompt modes. URL, git-diff, remote, and session-artifact references are intentionally reported as unsupported in this first pass rather than silently fetched or dropped.

## Branching

Fork conversations to explore alternatives without losing your work. Use `/fork` to try different approaches, `/switch` to navigate between branches, `/branches` to list them, and `/merge` to combine the best parts. See [`docs/tutorials/branching.md`](docs/tutorials/branching.md) for a walkthrough.

## Subagents

Delegate work to sub-instances. `subagent` spawns ephemeral one-shot workers for quick tasks (search, review, analysis) with parallel and chained execution. `delegate_task` spawns persistent named workers for long-running tasks that maintain state across interactions. Both get their own context and tool access.

## Worktree Isolation

Each session can run in its own git worktree, so parallel agents can't step on each other. Includes LLM-powered merge conflict resolution when merging back. Disable with `--no-worktree`.

## Plugins

clankers supports multiple plugin runtimes:

- `kind: "extism"` — WebAssembly plugins loaded via [Extism](https://extism.org)
- `kind: "stdio"` — supervised process plugins that register tools live over a framed stdio protocol

Install a plugin by dropping a plugin directory into `plugins/` or by running `clankers plugin install <path>`.

Extism example:

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

Stdio example:

```json
{
  "name": "clankers-stdio-echo",
  "version": "0.1.0",
  "kind": "stdio",
  "stdio": {
    "command": "./plugin.py",
    "working_dir": "plugin-dir",
    "sandbox": "inherit"
  }
}
```

Reference stdio fixture/example: `examples/plugins/clankers-stdio-echo/`.

Build Extism plugins with `cargo build --target wasm32-unknown-unknown --release`. For stdio plugins, implement the clankers length-prefixed JSON protocol and register tools after `ready`.

See `docs/src/reference/plugins.md` for launch-policy fields, sandbox modes, and migration guidance from Extism manifests to stdio plugins.

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

## Batch Trajectory Runs

Run bounded local prompt batches and export trajectories for evaluation, review, or RL data preparation:

```bash
clankers batch run prompts.jsonl --output results.jsonl
clankers batch run prompts.jsonl --output sharegpt.json --format sharegpt --concurrency 2
```

Input is local JSONL: each line is an object with a non-empty `prompt`, optional string `id`, and optional object `metadata`. Outputs are local JSONL or ShareGPT-style files. The first pass is a foreground CLI workflow only: remote `http://`, `https://`, or `s3://` inputs/outputs are rejected, concurrency is bounded, result ordering is stable, and replay/debug metadata records counts and prompt sizes rather than raw prompts.

## Built-in Tools

Core: `read`, `write`, `edit`, `patch`, `execute_code`, `process`, `bash`, `grep`, `find`, `ls`, `ask`, `commit`, `web`, `nix`

Orchestration: `subagent`, `delegate_task`, `switch_model`, `loop`, `signal_loop_success`

Specialty: `review`, `todo`, `cost`, `schedule`, `browser` (when `browserAutomation.enabled`), `external_memory` (when `externalMemory.enabled`), `checkpoint`, `tool_gateway`, `voice_mode`, `soul_personality`, `image_gen`, `procmon`, `skills_list`, `skill_view`, `validate_tui`

Working-directory checkpoints: `clankers checkpoint create [--label <LABEL>]`, `clankers checkpoint list`, and `clankers checkpoint rollback <CHECKPOINT_ID> --yes` snapshot and restore local git checkout files using `.git/clankers-checkpoints`. Agents can use the Specialty `checkpoint` tool for the same local git-backed create/list/rollback surface. The first pass is local-only: non-git directories, remote checkpoint stores, submodule recursion, and rollback without explicit confirmation return actionable errors. Replay/debug metadata records action/status/backend/repo/checkpoint id/counts and sanitized errors, not raw diffs or file contents.

Tool gateway validation: `clankers gateway status [--json]` reports the first-pass local gateway boundary, and `clankers gateway validate --toolsets <LIST> [--deliver <TARGET>] [--json]` validates local/session delivery and toolset names. Agents can use the Specialty `tool_gateway` tool for the same status/validate surface. Remote/platform delivery, Matrix delivery outside an active bridge, webhooks, cloud storage targets, and credential/header delivery return explicit unsupported errors. Replay/debug metadata records only safe labels.

Voice/STT validation: `clankers voice status [--json]` reports first-pass voice support boundaries, and `clankers voice validate --input <SOURCE> [--reply <text|tts|none>] [--json]` validates a local file or unsupported microphone/remote/Matrix input without recording, reading audio bytes, or contacting an STT provider. Agents can use the Specialty `voice_mode` tool for the same status/validate surface. First-pass support is local policy validation only: microphone capture, remote/cloud audio, provider transcription, automatic spoken reply loops, and Matrix audio outside an active bridge return explicit unsupported errors. Replay/debug metadata records safe input kind/label and reply mode, not raw audio, transcripts, full paths, URLs, credentials, or Matrix payloads.

SOUL/personality validation: `clankers soul status [--json]` reports first-pass SOUL/personality support boundaries, and `clankers soul validate [--soul <PATH|discover>] [--personality <NAME>] [--json]` validates local SOUL file/discovery intent and safe personality preset names without mutating the active system prompt. Agents can use the Specialty `soul_personality` tool for the same status/validate surface. First-pass support is local policy validation only: remote/cloud persona fetching, command hooks, encrypted/secret bundles, raw prompt/personality persistence, and autonomous self-modifying personality changes return explicit unsupported errors. Replay/debug metadata records safe source kind/label, optional preset name, support flag, and sanitized error category/message, not raw SOUL contents, full paths, URLs, headers, commands, credentials, or prompt text.

Matrix: `matrix_send`, `matrix_read`, `matrix_rooms`, `matrix_peers`, `matrix_join`, `matrix_rpc`

Plugins and configured MCP servers add additional tools at runtime. MCP tools are published as Specialty tools using a source-identifying prefix such as `mcp_filesystem_read_file` or a configured `toolPrefix`.

## Architecture

For the prompt/event/provider/session golden path, see [`docs/src/reference/request-lifecycle.md`](docs/src/reference/request-lifecycle.md).

Workspace-local crates under `crates/`:

| Crate | Purpose |
|---|---|
| `clanker-auth` | Generic capability token infrastructure |
| `clanker-message` | Reusable conversation message/content/streaming types |
| `clanker-plugin-sdk` | Extism guest SDK used by plugins |
| `clanker-router` | Multi-provider routing, fallback, caching, OAuth, RPC |
| `clanker-tui-types` | Shared TUI event/action/block/display types |
| `clankers-agent` | Agent loop, prompt execution, tool dispatch |
| `clankers-agent-defs` | Agent definition discovery and loading |
| `clankers-autoresearch` | Automated research workflows |
| `clankers-config` | Settings, paths, keybindings, auth path materialization |
| `clankers-controller` | SessionController (transport-agnostic agent driver) |
| `clankers-core` | Functional-core state/effect contracts |
| `clankers-db` | Embedded database (redb) |
| `clankers-engine` | Shell-independent engine request/event surface |
| `clankers-engine-host` | Runtime host for engine turns and streams |
| `clankers-hooks` | Event hooks (pre-commit, session start, etc.) |
| `clankers-matrix` | Matrix protocol bridge |
| `clankers-model-selection` | Complexity routing and cost tracking |
| `clankers-nix` | Nix evaluation and integration helpers |
| `clankers-plugin` | Plugin discovery, manifests, and host runtime |
| `clankers-procmon` | Process monitor |
| `clankers-prompts` | Prompt template system |
| `clankers-protocol` | Daemon-client wire protocol (frames, events, commands) |
| `clankers-provider` | LLM provider abstraction and compatibility adapters |
| `clankers-session` | JSONL session persistence and context rebuilds |
| `clankers-skills` | Skill discovery and loading |
| `clankers-tool-host` | Shared tool catalog/executor host traits |
| `clankers-tts` | Text-to-speech integration |
| `clankers-tui` | Terminal UI (ratatui) |
| `clankers-ucan` | Clankers-specific capability tokens over `clanker-auth` |
| `clankers-util` | Shared utilities (logging, direnv, parsing, truncation, etc.) |
| `clankers-zellij` | Zellij session sharing |

Extracted first-party dependencies used by the workspace include [`clanker-actor`](https://github.com/brittonr/clanker-actor), [`clanker-loop`](https://github.com/brittonr/clanker-loop), [`clanker-scheduler`](https://github.com/brittonr/clanker-scheduler), and [`graggle`](https://github.com/brittonr/graggle).

## License

[AGPL-3.0-or-later](LICENSE)
