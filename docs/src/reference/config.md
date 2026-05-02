# Configuration

## Config paths

Clankers reads configuration from two locations, merged with project-local taking precedence:

| Scope | Path |
|-------|------|
| Global | `~/.clankers/agent/` |
| Project | `.clankers/` (in repo root) |
| Fallback | `~/.pi/agent/` (pi compatibility) |

## settings.json

Main configuration file. Place in `~/.clankers/settings.json` or `.clankers/settings.json`.

```json
{
  "model": "claude-sonnet-4-5",
  "routing": {
    "enabled": true,
    "low_threshold": 20.0,
    "high_threshold": 50.0,
    "budget_soft_limit": 5.0,
    "budget_hard_limit": 10.0
  },
  "modelRoles": {
    "default": { "model": "claude-sonnet-4-5" },
    "smol": { "model": "claude-haiku-4" },
    "slow": { "model": "claude-opus-4" }
  },
  "costTracking": {
    "soft_limit": 5.0,
    "hard_limit": 10.0,
    "warning_interval": 1.0
  },
  "mcp": {
    "servers": {
      "filesystem": {
        "enabled": true,
        "transport": "stdio",
        "command": "npx",
        "args": ["-y", "@modelcontextprotocol/server-filesystem", "/tmp"],
        "envAllowlist": ["MCP_TOKEN"],
        "includeTools": ["read_file"],
        "excludeTools": [],
        "toolPrefix": "fs",
        "timeoutMs": 30000
      },
      "search": {
        "enabled": false,
        "transport": "http",
        "url": "https://mcp.example.test/rpc",
        "headerEnv": { "Authorization": "MCP_AUTH_HEADER" }
      }
    }
  },
  "browserAutomation": {
    "enabled": true,
    "backend": "cdp",
    "cdpUrl": "http://127.0.0.1:9222",
    "browserBinary": null,
    "userDataDir": ".clankers/browser-profile",
    "headless": true,
    "allowEvaluate": false,
    "allowScreenshots": true,
    "timeoutMs": 30000,
    "allowedOrigins": ["http://localhost:*", "https://example.test"]
  },
  "externalMemory": {
    "enabled": true,
    "provider": "local",
    "name": "project-memory",
    "maxResults": 8,
    "injectIntoPrompt": false
  }
}
```

MCP server entries are merged by name across global/project settings. Stdio servers use `command` and optional `args`; HTTP servers use `url` and optional `headerEnv` mappings whose values are read from environment variables. Clankers only forwards explicitly allowlisted environment variables or header values. MCP tool publication applies `includeTools` before `excludeTools`, skips collisions with existing tools, and prefixes visible tool names with `mcp_<server>_` unless `toolPrefix` is set.

Browser automation is disabled by default. Set `browserAutomation.enabled = true` with either `cdpUrl` for an existing local Chrome/Chromium DevTools endpoint or `browserBinary` to let clankers launch a local browser. The first backend is CDP HTTP: it supports `navigate`, `snapshot`/`current_url`, and `close`; selector clicks, fills, screenshots, and JavaScript evaluation require a later CDP WebSocket command backend and return explicit unsupported-action errors in this slice. `allowedOrigins` gates navigation before any backend call, `allowEvaluate` and `allowScreenshots` enforce policy, and tool results include replay/debug metadata such as source, action, status, elapsed time, session id, backend, URL/origin, and safe error details.

External memory providers are disabled by default. Set `externalMemory.enabled = true` to publish the Specialty `external_memory` tool. The first pass supports the local provider, which searches the existing clankers memory database with `search` and reports configuration with `status`; HTTP providers validate their endpoint/credential settings but return an explicit unsupported-provider error before network contact. `maxResults` bounds returned memories, `name` is a safe label for output/metadata, and `injectIntoPrompt` is stored for future prompt-injection support but does not yet add provider context automatically. Tool result metadata is replay/debug safe: it records provider kind/name, action, status, elapsed time, result count, and sanitized error details, but never raw queries, result text, headers, tokens, or credential environment values.

Working-directory checkpoints need no configuration in the first pass. Use `clankers checkpoint create`, `clankers checkpoint list`, and `clankers checkpoint rollback <CHECKPOINT_ID> --yes` in a git checkout, or the Specialty `checkpoint` tool from prompt/TUI/daemon tool paths. The local git backend stores snapshots in `.git/clankers-checkpoints`, restores only clankers-owned checkpoint ids, and rejects non-git directories, remote stores, submodule recursion, and rollback without explicit confirmation. Replay/debug metadata records ids, counts, repo path, status, and sanitized errors; raw diffs and file contents are not persisted.

Tool gateway/platform delivery needs no configuration in the first pass. Use `clankers gateway status [--json]` or `clankers gateway validate --toolsets <LIST> [--deliver <TARGET>] [--json]`, or the Specialty `tool_gateway` tool from prompt/TUI/daemon paths, to validate normalized toolsets and local/session delivery policy. The first pass supports local/session validation and Matrix only when an active Matrix bridge context is explicitly present. Remote/platform delivery, webhooks, cloud storage, credential/header delivery, and Matrix outside an active bridge return explicit unsupported errors with safe metadata.

Voice/STT mode needs no configuration in the first pass. Use `clankers voice status [--json]` or `clankers voice validate --input <SOURCE> [--reply <text|tts|none>] [--json]`, or the Specialty `voice_mode` tool from prompt/TUI/daemon paths, to validate local file input and reply-mode policy without recording, reading audio bytes, or contacting STT providers. The first pass supports local file-policy validation only. Microphone capture, provider transcription, remote/cloud audio, automatic spoken reply loops, and Matrix audio outside an active bridge return explicit unsupported errors. Replay/debug metadata records safe input kind/label and reply mode, not raw audio, transcripts, full paths, URLs, credentials, or Matrix payloads.

SOUL/personality mode needs no configuration in the first pass. Use `clankers soul status [--json]` or `clankers soul validate [--soul <PATH|discover>] [--personality <NAME>] [--json]`, or the Specialty `soul_personality` tool from prompt/TUI/daemon paths, to validate local SOUL file/discovery intent and safe personality preset names without mutating prompt assembly. The first pass supports local policy validation only. Remote/cloud persona sources, command-executed persona hooks, encrypted/secret bundles, raw prompt/personality persistence, and autonomous self-modifying personality changes return explicit unsupported errors. Replay/debug metadata records safe source kind/label, optional preset name, support flag, and sanitized error category/message, not raw SOUL contents, full paths, URLs, headers, commands, credentials, or prompt text.

See [Multi-Model Routing](./multi-model.md) for the full routing configuration reference.

## pricing.json

Custom model pricing in `~/.clankers/pricing.json`:

```json
{
  "claude-opus-4": {
    "input_per_mtok": 15.0,
    "output_per_mtok": 75.0,
    "display_name": "Claude Opus 4"
  }
}
```

## Agent definitions

Named agent configurations in `~/.clankers/agent/agents/` or `.clankers/agents/`:

```bash
clankers --agent reviewer
clankers --agent researcher --agent-scope project
```

## Skills

Reusable prompt snippets in `~/.clankers/agent/skills/<name>/SKILL.md` or `.clankers/skills/<name>/SKILL.md`.

## CLI flags

| Flag | Description |
|------|-------------|
| `--model <name>` | Override model |
| `--budget <amount>` | Hard budget limit (USD) |
| `--max-cost <amount>` | Hard budget limit (alias) |
| `--enable-routing` | Enable complexity-based routing |
| `--agent <name>` | Use a named agent definition |
| `--no-worktree` | Disable git worktree isolation |
| `--continue` | Resume last session |
| `--resume <id>` | Resume specific session |
| `-p <prompt>` | One-shot prompt (no TUI) |
| `--mode json` | JSON lines output |
| `--mode inline` | Styled markdown in scrollback |
| `--inline` | Shorthand for `--mode inline` |
| `-o <file>` | Write output to file |
| `--stdin` | Read prompt from stdin |
| `--zellij` | Run inside Zellij |
| `--swarm` | Enable swarm mode |
