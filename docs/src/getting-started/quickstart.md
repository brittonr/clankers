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

## Batch trajectory runs

Run local JSONL prompt batches and export deterministic trajectories for evaluation or review:

```bash
clankers batch run --input prompts.jsonl --output results.jsonl
clankers batch run --input prompts.jsonl --output sharegpt.json --format sharegpt --concurrency 2
clankers batch run --input prompts.jsonl --output eval.jsonl --format eval-jsonl --execution daemon --run-id smoke --resume
```

Each input line is a JSON object with a non-empty `prompt`, optional string `id`, and optional object `metadata`. Batch runs accept local paths only, bound concurrency, preserve result order, and write safe JSONL, ShareGPT, or eval JSONL output. Daemon execution records deterministic per-job session ids and resumes each prompt through normal session persistence; `--resume` reads the sidecar manifest and skips completed job ids. Replay metadata records counts, prompt sizes, session/model handles, redaction status, and optional objective receipts without logging raw prompts or credentials.

The `batch-eval-runner-kit` is the copyable brick for eval-oriented batch reuse: use a local JSONL fixture, explicit run id, eval JSONL export, deterministic resume manifest, objective metadata such as `expected_contains`, and fail-closed local-path validation. The checked drift rail is `scripts/check-batch-eval-runner-kit.rs`, and the focused fixture lives in `src/modes/batch.rs`.

## ACP IDE integration

Expose a clankers session to ACP-compatible editors over foreground stdio:

```bash
clankers acp serve                         # serve a new ACP session
clankers acp serve --session <id>          # resume a known session id
clankers acp serve --new --model <model>   # force a fresh session and optional model
```

The current adapter supports ACP initialization, `session/new` binding, and `session/prompt` prompt acceptance with safe receipts. Prompt receipts record session id, byte count, and prompt hash rather than raw prompt text. Terminal creation, remote workspaces, diffs, arbitrary tool listing/calls, and editor push notifications return explicit unsupported-method errors.

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
