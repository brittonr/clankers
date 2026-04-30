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
  }
}
```

MCP server entries are merged by name across global/project settings. Stdio servers use `command` and optional `args`; HTTP servers use `url` and optional `headerEnv` mappings whose values are read from environment variables. Clankers only forwards explicitly allowlisted environment variables or header values. MCP tool publication applies `includeTools` before `excludeTools`, skips collisions with existing tools, and prefixes visible tool names with `mcp_<server>_` unless `toolPrefix` is set.

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
