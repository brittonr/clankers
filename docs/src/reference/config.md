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
`thinkingLevel` defaults to `max`; set it to `off` to disable provider reasoning summaries by default.

```json
{
  "model": "openai-codex/gpt-5.5",
  "thinkingLevel": "max",
  "routing": {
    "enabled": true,
    "low_threshold": 20.0,
    "high_threshold": 50.0,
    "budget_soft_limit": 5.0,
    "budget_hard_limit": 10.0
  },
  "modelRoles": {
    "default": { "model": "openai-codex/gpt-5.5" },
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
  },
  "steelEval": {
    "enabled": true
  }
}
```

MCP server entries are merged by name across global/project settings. Stdio servers use `command` and optional `args`; HTTP servers use `url` and optional `headerEnv` mappings whose values are read from environment variables. Clankers only forwards explicitly allowlisted environment variables or header values. MCP tool publication applies `includeTools` before `excludeTools`, skips collisions with existing tools, and prefixes visible tool names with `mcp_<server>_` unless `toolPrefix` is set. Published MCP tool calls honor per-server `timeoutMs`, observe the session cancellation token, fail closed if the live catalog schema drifts from the published schema, and attach safe receipts with server/tool/status/duration/error-class metadata but no raw arguments, headers, tokens, environment values, or paths.

Browser automation is disabled by default. Set `browserAutomation.enabled = true` with either `cdpUrl` for an existing local Chrome/Chromium DevTools endpoint or `browserBinary` to let clankers launch a local browser. The local CDP backend supports stateful `navigate`, `snapshot`/`current_url`, `click`, `type`/`fill`, `screenshot`, `evaluate`, and `close` actions: target discovery and creation use the DevTools HTTP endpoints, while DOM/evaluate/screenshot actions use each target's `webSocketDebuggerUrl`. `allowedOrigins` gates navigation before any backend call, `allowEvaluate` and `allowScreenshots` enforce policy before script/screenshot dispatch, and tool results include replay/debug metadata such as source, action, status, elapsed time, session id, backend, URL/origin, title/target type, and safe error details without raw CDP URLs or credentials.

External memory providers are disabled by default. Set `externalMemory.enabled = true` to publish the Specialty `external_memory` tool after configuration validation. The local provider searches the existing clankers memory database with `search` and reports configuration with `status`. The HTTP provider is supported only when explicitly configured with `provider = "http"`, `endpoint`, and `credentialEnv`; the credential value is loaded from that environment variable at call time, blank/missing credentials fail closed before network contact, `timeoutMs` bounds the request, and `maxResults` bounds both the outbound query limit and returned memories. `name` is a safe label for output/metadata, and `injectIntoPrompt` remains opt-in policy state: explicit tool calls can return remote memory, but automatic prompt injection is still disabled unless later prompt assembly consumes that flag. Tool result metadata is replay/debug safe: it records provider kind/name, action, status, elapsed time, result count, injection policy, and sanitized error details, but never raw queries, result text, headers, tokens, or credential environment values.

Steel eval tool publication defaults to the safe pure profile. Missing `steelEval` config publishes the agent-visible `steel_eval` built-in with no ambient host functions, no session capabilities, zero host-call budget, bounded source/output/step limits, and deterministic redacted receipts. Set `steelEval.enabled = false` to omit the tool explicitly from the built-in catalog.

Steel turn planning defaults to the bundled reviewed `steel.host.plan_turn` profile/script when `steelTurnPlanning` is omitted. Set `steelTurnPlanning.enabled = false` to opt out and keep Rust-native planning with no Steel-authorship receipt. This setting controls only turn planning; it does not grant Steel provider/tool execution, mutation, filesystem, process, network, credential, daemon, TUI, or native-tool authority.

Working-directory checkpoints need no configuration in the first pass. Use `clankers checkpoint create`, `clankers checkpoint list`, and `clankers checkpoint rollback <CHECKPOINT_ID> --yes` in a git checkout, or the Specialty `checkpoint` tool from prompt/TUI/daemon tool paths. The local git backend stores snapshots in `.git/clankers-checkpoints`, restores only clankers-owned checkpoint ids, and rejects non-git directories, remote stores, submodule recursion, and rollback without explicit confirmation. Replay/debug metadata records ids, counts, repo path, status, and sanitized errors; raw diffs and file contents are not persisted.

Tool gateway/platform delivery needs no durable configuration in this slice. Use `clankers gateway status [--json]`, `clankers gateway validate --toolsets <LIST> [--deliver <TARGET>] [--json]`, `clankers gateway deliver --artifact-type <file|media|scheduled-output> [--path <PATH>] [--deliver <TARGET>] [--outbox <PATH>] [--matrix-active] [--json]`, `clankers gateway delivery-status --outbox <PATH> [--json]`, `clankers gateway retry --outbox <PATH> --attempt-id <ID> [--json]`, or the compatibility `clankers gateway deliver-receipt ...` path. The Specialty `tool_gateway` tool exposes the same status/validate/deliver/delivery_status/retry boundary from prompt/TUI/daemon paths. Standalone and daemon tool rebuilds share gateway policy helpers for active toolsets and disabled-tool filtering. Delivery attempts record an adapter-backed local/session or active-Matrix receipt plus safe outbox metadata; Matrix requires an explicit active bridge context and unsupported remote/webhook/cloud targets fail closed. Receipts/outboxes contain action/status/backend/source, artifact kind, target kind, attempt id, basename-only safe paths, redacted platform handles, retryability, and sanitized errors, never raw destinations, tokens, headers, credentials, or full paths.

Voice/STT mode needs no durable configuration in the local-first slice. Use `clankers voice status [--json]`, `clankers voice validate --input <SOURCE> [--reply <text|tts|none>] [--json]`, `clankers voice start --enable [--auto-submit] [--json]`, `clankers voice stop [--json]`, and `clankers voice submit-transcript --transcript <TEXT> [--reply <text|tts|none>] [--auto-submit] [--json]`, or the Specialty `voice_mode` tool from prompt/TUI/daemon paths. Capture never starts in the background by default, raw audio retention is false in the first implementation, cloud STT is disabled by policy, and accepted transcripts are handed to the ordinary session prompt path only after explicit submit/auto-submit policy. Replay/debug metadata records safe input kind/label, capture state, provider-handle status, reply mode, counts, and transcript digests, not raw audio, transcripts, full paths, URLs, credentials, or Matrix payloads.

SOUL/personality prompt assembly needs no durable config file in this slice. Local `SOUL.md` is discovered from `.clankers/SOUL.md`, project-root `SOUL.md`, then `~/.clankers/agent/SOUL.md` and included in the system prompt after SYSTEM/APPEND_SYSTEM and before AGENTS.md/CLAUDE.md. Set `CLANKERS_PERSONALITY_PRESET=<name>` to include `.clankers/personality/<name>.md` or `~/.clankers/agent/personality/<name>.md`; set `CLANKERS_DISABLE_SOUL_PERSONALITY=1` to omit both SOUL and preset sections. Use `clankers soul status [--json]` or `clankers soul validate [--soul <PATH|discover>] [--personality <NAME>] [--json]`, or the Specialty `soul_personality` tool from prompt/TUI/daemon paths, for non-mutating policy checks. Remote/cloud persona sources, command-executed persona hooks, encrypted/secret bundles, raw prompt/personality persistence, and autonomous self-modifying personality changes return explicit unsupported errors. Replay/debug metadata records safe source kind/label, optional preset name, support flag, path hash, byte count, precedence, and sanitized error category/message, not raw SOUL contents, full paths, URLs, headers, commands, credentials, or prompt text.

Self-evolution productionization has no global enable switch; every run remains explicit and disabled-by-default. `clankers self-evolution run` defaults to `--profile dry-run-only`, which records isolated candidate artifacts and mechanical receipts without claiming production evidence. `--profile controlled-dogfood` and `--profile promotion-eligible` require `--corpus-manifest <PATH>` with local JSON fields `version`, `targets`, `cases`, `redaction_policy`, `min_improvement`, and `regression_budget`. Missing or invalid corpus manifests produce `readiness.label=blocked`, and approval/application requires `readiness.label=promotion_eligible` plus the existing human approval and live-apply gates. Receipts record safe readiness labels, reasons, thresholds, case counts, session-observability evidence, hashes, and sanitized limitations; they do not persist raw prompts, transcripts, corpus payloads, credentials, or full session content.

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
