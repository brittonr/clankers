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
