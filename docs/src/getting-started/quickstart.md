# Quick Start

## Interactive TUI

```bash
clankers
```

Type a prompt in insert mode (`i`), press Enter to send. The agent reads files, runs commands, and edits code. Press `q` in normal mode to quit.

## One-shot prompts

```bash
clankers -p "fix the tests"
clankers -p "explain this codebase"
clankers -p "explain this codebase" --inline   # styled markdown in scrollback
clankers -p "list all TODOs" --mode json
echo "what is this?" | clankers --stdin
```

## Daemon mode

Run sessions in the background and attach from any terminal:

```bash
clankers daemon start -d        # start background daemon
clankers attach --new           # create + attach to a session
clankers ps                     # list active sessions
clankers attach                 # reattach (interactive picker)
clankers daemon kill <id>       # kill a session
clankers daemon stop            # stop daemon
```

## ACP IDE integration

Expose a clankers session to ACP-compatible editors over foreground stdio:

```bash
clankers acp serve                         # serve a new ACP session
clankers acp serve --session <id>          # resume a known session id
clankers acp serve --new --model <model>   # force a fresh session and optional model
```

The first pass supports ACP initialization and session prompt/update request handling. Terminal creation, remote workspaces, arbitrary tool listing/calls, and editor push notifications return explicit unsupported-method errors.

## Sessions

Conversations persist as JSONL. Resume where you left off:

```bash
clankers --continue             # resume last session
clankers --resume <id>          # resume a specific one
clankers session list
```

## Slash commands

Type `/` in the input to see all available commands. Common ones:

| Command | What it does |
|---------|-------------|
| `/help` | List commands |
| `/model <name>` | Switch model |
| `/think [level]` | Set thinking depth |
| `/compact` | Summarize to save tokens |
| `/fork [reason]` | Branch the conversation |
| `/usage` | Show token usage and cost |
| `/session list` | List recent sessions |

See [Commands Reference](../reference/commands.md) for the full list.
