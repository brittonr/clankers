# Bot Commands

## Purpose

Give users control over the agent session from within the Matrix room.
Currently the daemon skips all `/`-prefixed messages.  Replace that with
a command dispatch system using `!` prefix (Matrix convention for bot
commands, avoids collision with Matrix client slash commands).

## Requirements

### Command prefix

Bot commands MUST use the `!` prefix.  Messages starting with `!` that
match a known command are dispatched; unknown `!` commands are passed
through to the agent as normal prompts.

### Required commands

The daemon MUST implement these commands:

| Command | Behavior |
|---------|----------|
| `!restart` | Kill the current session, start fresh on next message |
| `!status` | Reply with model, session turn count, uptime, token usage |
| `!skills` | List loaded skills |
| `!compact` | Trigger context compaction on the session |
| `!model <name>` | Switch the session's model |
| `!help` | List available commands |

### Command responses

Command responses MUST be sent as replies to the command message (Matrix
reply threading) so they don't pollute the main conversation flow.

GIVEN a user sends `!status`
WHEN the daemon processes the command
THEN a reply is sent with session info
AND the command is NOT forwarded to the agent

### Unknown commands passthrough

GIVEN a user sends `!foobar hello`
WHEN `foobar` does not match any known command
THEN the full message `!foobar hello` is forwarded to the agent as a prompt

### Slash commands ignored

Messages starting with `/` SHOULD continue to be silently ignored (they are
Matrix client commands, not intended for the bot).
