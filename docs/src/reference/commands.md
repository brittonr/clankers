# Slash Commands Reference

Slash commands provide quick access to common operations. Type `/` in the input editor to see autocomplete suggestions.

## General

### `/help`
Show available commands with descriptions.

**Example:**
```
/help
```

### `/clear`
Clear conversation history from display. Does not affect the agent's context window.

**Example:**
```
/clear
```

### `/reset`
Reset conversation and context. Clears history and starts fresh.

**Example:**
```
/reset
```

### `/compact`
Summarize conversation to save tokens. Asks the model to create a compact summary of the conversation so far, replacing the full history to reduce token usage.

**Example:**
```
/compact
```

### `/model <name>`
Switch to a different model.

**Example:**
```
/model claude-opus-4-6
/model gpt-4
```

### `/think [off|low|medium|high|max]`
Set or cycle through extended thinking levels.

**Usage:**
```
/think              # cycle to next level
/think off          # disable thinking
/think low          # light reasoning (~5k tokens)
/think medium       # moderate reasoning (~10k tokens)
/think high         # deep reasoning (~32k tokens)
/think max          # maximum reasoning (~128k tokens)
/think 10000        # set budget directly (maps to nearest level)
```

**Keybinding:** `Ctrl+T` cycles through levels

### `/status`
Show current settings: model, token usage, and session information.

**Example:**
```
/status
```

### `/usage`
Show detailed token usage statistics and estimated cost.

**Example:**
```
/usage
```

### `/undo`
Remove the last conversation turn (user message + assistant response).

**Example:**
```
/undo
```

### `/version`
Show clankers version and build information.

**Example:**
```
/version
```

## Session Management

### `/session [list|resume|delete|purge]`
Manage sessions.

**Usage:**
```
/session                # show current session info
/session list [n]       # list recent sessions (default: 10)
/session resume [id]    # resume a previous session (opens menu if no id)
/session delete <id>    # delete a session
/session purge          # delete all sessions for this directory
```

**Examples:**
```
/session list 20
/session resume sess_abc123
/session delete sess_old456
```

### `/export [filename]`
Export conversation to a file.

**Example:**
```
/export conversation.md
/export
```

## Navigation

### `/cd <path>`
Change working directory.

**Example:**
```
/cd /home/user/project
/cd ../other-project
```

## Tools

### `/shell <command>`
Run a shell command directly without going through the agent.

**Example:**
```
/shell ls -la
/shell git status
```

### `/tools`
List all available tools, including built-in tools and any tools provided by loaded plugins.

**Example:**
```
/tools
```

### `/plugin [name]`
Show loaded plugins. Lists all discovered and loaded plugins with their status.

**Usage:**
```
/plugin             # list all plugins
/plugin wordcount   # show details for a specific plugin
```

## Authentication

### `/login [code|url|--account <name>]`
Start or complete OAuth login flow with Anthropic.

**Usage:**
```
/login                      # generate an auth URL and display it
/login <code#state>         # complete login with code from browser
/login <callback URL>       # complete login with the full callback URL
/login --account <name>     # login to a specific account
```

See also: `/account`

### `/account [switch|login|logout|remove|status|list]`
Manage multiple authenticated accounts.

**Usage:**
```
/account                    # list all accounts & status
/account switch <name>      # switch active account
/account login [name]       # login to an account (default: active)
/account logout [name]      # logout an account
/account remove <name>      # remove an account
/account status [name]      # show account status
/account list               # list all accounts
```

**Examples:**
```
/account switch work
/account logout personal
```

## Collaboration

### `/worker [name] [task]`
Spawn or list swarm workers in Zellij panes.

**Usage:**
```
/worker                     # list active workers
/worker <name>              # spawn an idle worker
/worker <name> <task>       # spawn worker with a task
```

**Example:**
```
/worker builder
/worker tester run all unit tests
```

**Requirements:** Must be running inside a Zellij session (`clankers --zellij` or `clankers --swarm`)

### `/share [--read-only]`
Share the current Zellij session remotely via iroh P2P.

**Usage:**
```
/share              # share read-write
/share --read-only  # share read-only
```

**Requirements:** Must be running inside a Zellij session

### `/subagents [kill|remove|clear]`
List and manage subagents.

**Usage:**
```
/subagents              # list all subagents
/subagents kill <id>    # kill a running subagent
/subagents kill all     # kill all running subagents
/subagents remove <id>  # remove a subagent entry from the panel
/subagents clear        # remove all completed/failed subagents
```

**Examples:**
```
/subagents
/subagents kill sub_abc123
/subagents clear
```

### `/peers [add|remove|probe|discover|allow|deny|server]`
Manage P2P swarm peers.

**Usage:**
```
/peers                          # list all peers (switches to peers panel)
/peers add <node-id> <name>     # add a peer to the registry
/peers remove <name-or-id>      # remove a peer
/peers probe [name-or-id]       # probe a peer (or all peers)
/peers discover                 # scan LAN via mDNS for new peers
/peers allow <node-id>          # add to allowlist
/peers deny <node-id>           # remove from allowlist
/peers server [on|off]          # start/stop embedded RPC server
```

**Examples:**
```
/peers add abc123...xyz worker-1
/peers probe
/peers discover
```

## Planning & Review

### `/plan [on|off]`
Toggle architecture-first plan mode. In plan mode, the agent reads and analyzes the codebase first, proposes an implementation plan, and waits for approval before making any edits.

**Usage:**
```
/plan           # toggle plan mode
/plan on        # enable plan mode
/plan off       # disable plan mode
```

### `/review [base|staged]`
Start an interactive code review of recent changes.

**Usage:**
```
/review             # review changes vs main/master
/review <base>      # review changes vs a specific base ref
/review staged      # review only staged changes
```

**Examples:**
```
/review
/review develop
/review staged
```

### `/role [name] [model]`
Switch or list model roles for different task types.

**Usage:**
```
/role                   # list all role assignments
/role <name>            # switch to a role's model
/role <name> <model>    # set a role's model
/role reset             # clear all role overrides
```

**Roles:** `default`, `smol`, `slow`, `plan`, `commit`, `review`

**Examples:**
```
/role plan
/role commit claude-opus-4-6
/role reset
```

## UI & Layout

### `/todo [add|done|wip|remove|clear]`
Manage todo list in the right-side panel.

**Usage:**
```
/todo                   # list all items
/todo add <text>        # add a new item
/todo done <id|text>    # mark item as done
/todo wip <id|text>     # mark item as in-progress
/todo remove <id>       # remove an item
/todo clear             # remove all completed items
```

**Examples:**
```
/todo add Fix parser tests
/todo done 1
/todo wip refactor auth
```

### `/preview [markdown]`
Preview markdown rendering (debug/test). Injects a fake assistant block with sample markdown content.

**Usage:**
```
/preview                # show default markdown sample
/preview **bold** text  # render the provided markdown
```

### `/layout <preset>|toggle <panel>`
Switch panel layout.

**Usage:**
```
/layout default             # 3-column (todo+files | chat | subagents+peers)
/layout wide                # wide chat with left sidebar
/layout focused             # chat only (no panels)
/layout right               # all panels on the right
/layout toggle <panel>      # show/hide a panel (todo|files|subagents|peers)
```

**Examples:**
```
/layout focused
/layout toggle subagents
```

## Advanced

### `/system [show|set|append|prepend|reset|file]`
View or modify the system prompt.

**Usage:**
```
/system                     # show current system prompt (truncated)
/system show                # show full system prompt
/system set <text>          # replace the system prompt entirely
/system append <text>       # append text to the system prompt
/system prepend <text>      # prepend text to the system prompt
/system reset               # restore the original system prompt
/system file <path>         # load system prompt from a file
```

**Examples:**
```
/system show
/system append Always output code comments
/system reset
```

### `/editor`
Open `$EDITOR` (or `$VISUAL`, falls back to `vi`) to compose a multi-line prompt. Content loads back into the input when you save and quit.

**Keybindings:** `Ctrl+O` (insert mode), `o` (normal mode)

**Example:**
```
/editor
```

### `/memory [add|edit|remove|search|clear]`
Manage cross-session persistent memories.

**Usage:**
```
/memory                         # list all memories
/memory add <text>              # add a global memory
/memory add --project <text>    # add a project-scoped memory
/memory edit <id> <text>        # replace memory text by ID
/memory remove <id>             # remove a memory by ID
/memory search <query>          # search memories by text/tags
/memory clear                   # remove all memories
```

**Examples:**
```
/memory add User prefers tabs over spaces
/memory add --project Use TypeScript for all new files
/memory search coding style
```

## Branching (NEW)

### `/fork [reason]`
Fork conversation to explore alternatives. Creates a new branch from the current message.

**Usage:**
```
/fork                       # fork with auto-generated name
/fork try recursive descent # fork with a descriptive reason
```

**Examples:**
```
/fork
/fork try async version
/fork explore memoization approach
```

### `/rewind <N|message-id|label>`
Jump back to an earlier message in the conversation.

**Usage:**
```
/rewind <N>             # go back N messages
/rewind <message-id>    # jump to specific message
/rewind <label>         # jump to a labeled message
```

**Examples:**
```
/rewind 5
/rewind msg_abc123
/rewind parser-fork-point
```

### `/branches [--verbose]`
List all conversation branches.

**Usage:**
```
/branches               # list all branches
/branches --verbose     # show detailed branch tree
```

### `/switch <branch-name|message-id>`
Switch to a different conversation branch.

**Usage:**
```
/switch <branch-name>   # switch by branch name
/switch <message-id>    # switch to specific message
```

**Examples:**
```
/switch recursive-approach
/switch msg_def456
```

### `/label <name>`
Add a human-readable label to the current message. Labels can be used with `/rewind` and `/switch` for easy navigation.

**Example:**
```
/label working-parser
/label checkpoint-before-refactor
```

### `/compare <block-id-a> <block-id-b>`
Compare two branches side-by-side. Shows differences between approaches taken in different branches.

**Example:**
```
/compare msg_abc123 msg_def456
```

### `/merge <source> <target>`
Merge one branch into another. Combines the conversation history from the source branch into the target branch.

**Example:**
```
/merge recursive-branch main
```

### `/merge-interactive <source> <target>`
Interactively select which messages to merge from the source branch into the target branch.

**Example:**
```
/merge-interactive experimental main
```

### `/cherry-pick <message-id> <target> [--with-children]`
Copy specific message(s) from one branch to another.

**Usage:**
```
/cherry-pick <message-id> <target>                  # copy single message
/cherry-pick <message-id> <target> --with-children  # copy message + descendants
```

**Examples:**
```
/cherry-pick msg_abc123 main
/cherry-pick msg_def456 experimental --with-children
```

## Keyboard Shortcuts for Branching

- `b` (normal mode) / `Ctrl+B` (insert mode) — Toggle branch panel
- `Shift+B` — Open branch switcher (fuzzy picker)
- `Shift+I` / `Ctrl+I` — Toggle message ID display

## Quit

### `/quit`
Exit clankers.

**Example:**
```
/quit
```
